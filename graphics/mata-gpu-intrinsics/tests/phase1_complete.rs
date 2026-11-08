use mata_gpu_intrinsics::{ir::*,Backend};
fn f32t()->DataType{DataType::Scalar(Number::F32)}
fn i32t()->DataType{DataType::Scalar(Number::I32)}
fn u32t()->DataType{DataType::Scalar(Number::U32)}
fn lit(v:f32)->Value{Value::FloatBits(v.to_bits())}
fn p(name:&str,ty:DataType)->Parameter{Parameter{name:name.into(),ty}}
fn kernel(name:&str,args:Vec<Parameter>,body:Vec<Statement>)->Kernel{
    Kernel{name:name.into(),args,body,workgroup:[8,2,1]}
}
fn program_kernel(k:Kernel)->Program {let mut pr=Program::default();pr.add_kernel(k).unwrap();pr}

#[test]
fn all_backends_emit_full_compute_entrypoint(){
    let inp=DataType::Pointer(Box::new(f32t()),AddressSpace::Global,Access::ReadOnly);
    let out=DataType::Pointer(Box::new(f32t()),AddressSpace::Global,Access::ReadWrite);
    let idx=global_id(0).unwrap();
    let input=variable("x",inp.clone()).unwrap();
    let output=variable("y",out.clone()).unwrap();
    let add=binary(Binary::Add,load(input,idx.clone()).unwrap(),lit(2.0)).unwrap();
    let prog=program_kernel(kernel("compute",vec![p("x",inp),p("y",out)],vec![Statement::Store(output,idx,add)]));
    prog.validate().unwrap();
    for b in [Backend::Wgsl,Backend::Glsl,Backend::Hlsl,Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip]{
        let src=prog.emit(b).unwrap();
        assert!(src.contains("compute") || b==Backend::Glsl);
        assert!(src.contains("x")&&src.contains("y"));
        assert_eq!(src,prog.emit(b).unwrap());
    }
    assert!(prog.emit(Backend::Glsl).unwrap().starts_with("#version 450\n"));
    assert!(prog.emit(Backend::Wgsl).unwrap().contains("@workgroup_size(8, 2, 1)"));
}
#[test]
fn indexing_is_portably_u32(){
    assert_eq!(global_id(1).unwrap().ty(),u32t());
    assert!(global_id(3).is_err());
    let k=kernel("idx",vec![],vec![Statement::Let("id".into(),u32t(),global_id(0).unwrap())]);
    let pr=program_kernel(k);
    assert!(pr.emit(Backend::IntelOpenCl).unwrap().contains("((uint)get_global_id(0))"));
    assert!(pr.emit(Backend::Wgsl).unwrap().contains("_global_id.x"));
}
#[test]
fn explicit_scalar_casts(){
    let cast=Value::Cast(Box::new(Value::Int(3,i32t())),f32t());
    let pr=program_kernel(kernel("cast",vec![],vec![Statement::Let("v".into(),f32t(),cast)]));
    assert!(pr.emit(Backend::Wgsl).unwrap().contains("f32(3)"));
    assert!(pr.emit(Backend::IntelOpenCl).unwrap().contains("((float)(3))"));
}
#[test]
fn std430_and_uniform_and_hlsl_layouts_distinct(){
    let mut pr=Program::default();
    pr.add_record(Record{name:"Particle".into(),fields:vec![
        Field{name:"v".into(),ty:DataType::Vector(Number::F32,3)},
        Field{name:"weight".into(),ty:f32t()},
        Field{name:"values".into(),ty:DataType::Array(Box::new(f32t()),2)}]}).unwrap();
    let st=pr.record_layout("Particle").unwrap();
    assert_eq!(st.fields[1].offset,12);
    assert_eq!(st.fields[2].offset,16);
    assert_eq!(st.layout.size,32);
    let ub=pr.record_layout_with("Particle",LayoutMode::Uniform140).unwrap();
    assert_eq!(ub.fields[2].offset,16);
    assert_eq!(ub.layout.size,48);
    let cb=pr.record_layout_with("Particle",LayoutMode::HlslConstant).unwrap();
    assert_eq!(cb.fields[1].offset,12);
    assert_eq!(cb.fields[2].offset,16);
    assert_eq!(cb.layout.size,48);
    let wg=pr.record_layout_with("Particle",LayoutMode::WgslUniform).unwrap();
    assert_eq!(wg.fields[2].offset,16);
}
#[test]
fn platform_vector_abi_is_explicit(){
    let pr=Program::default();
    let v3=DataType::Vector(Number::F32,3);
    assert_eq!(pr.layout_with(&v3,LayoutMode::Storage430).unwrap().size,12);
    assert_eq!(pr.layout_with(&v3,LayoutMode::OpenClNatural).unwrap().size,16);
    assert_eq!(pr.layout_with(&v3,LayoutMode::CudaNative).unwrap().align,4);
    assert_eq!(pr.layout_with(&v3,LayoutMode::HlslStructured).unwrap().align,4);
    let v8=DataType::Vector(Number::F32,8);
    assert_eq!(pr.layout_with(&v8,LayoutMode::OpenClNatural).unwrap().size,32);
    assert!(pr.storage_layout(&v8).is_err());
}
#[test]
fn bad_fields_and_unknown_calls_cannot_escape_validation(){
    let mut pr=Program::default();
    pr.add_record(Record{name:"A".into(),fields:vec![Field{name:"mass".into(),ty:f32t()}]}).unwrap();
    pr.add_kernel(kernel("bad",vec![p("a",DataType::Record("A".into()))],vec![
        Statement::Expr(Value::Field(Box::new(variable("a",DataType::Record("A".into())).unwrap()),"unknown".into(),f32t()))
    ])).unwrap();
    assert_eq!(pr.validate(),Err(IrError::UnknownField));
    let pr=program_kernel(kernel("bad",vec![],vec![Statement::Expr(Value::Call("no_function".into(),vec![],f32t()))]));
    assert_eq!(pr.validate(),Err(IrError::UnknownFunction));
}
#[test]
fn incorrect_function_arguments_are_rejected(){
    let mut pr=Program::default();
    pr.add_function(Function{name:"double_it".into(),args:vec![p("v",f32t())],result:f32t(),body:vec![
        Statement::Return(Some(variable("v",f32t()).unwrap()))]}).unwrap();
    pr.add_kernel(kernel("caller",vec![],vec![Statement::Let("x".into(),f32t(),
        Value::Call("double_it".into(),vec![Value::Int(7,i32t())],f32t()))])).unwrap();
    assert_eq!(pr.validate(),Err(IrError::TypeMismatch));
}
#[test]
fn recursion_and_missing_nonvoid_return_are_rejected(){
    let mut pr=Program::default();
    pr.add_function(Function{name:"f".into(),args:vec![],result:f32t(),body:vec![]}).unwrap();
    assert_eq!(pr.validate(),Err(IrError::InvalidControlFlow));
    pr.functions[0].body=vec![Statement::Return(Some(Value::Call("f".into(),vec![],f32t())))];
    assert_eq!(pr.validate(),Err(IrError::InvalidControlFlow));
}
#[test]
fn tuples_are_emitted_and_type_checked(){
    let tt=DataType::Tuple(vec![f32t(),i32t()]);
    let mut pr=Program::default();
    pr.add_function(Function{name:"pair".into(),args:vec![],result:tt.clone(),body:vec![
        Statement::Return(Some(Value::Tuple(vec![lit(1.0),Value::Int(2,i32t())])))
    ]}).unwrap();
    pr.add_kernel(kernel("consumer",vec![],vec![
        Statement::Let("result".into(),tt.clone(),Value::Call("pair".into(),vec![],tt.clone())),
        Statement::Let("first".into(),f32t(),Value::TupleAt(Box::new(variable("result",tt).unwrap()),0,f32t()))
    ])).unwrap();
    pr.validate().unwrap();
    for backend in [Backend::Wgsl,Backend::Glsl,Backend::Hlsl,Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip] {
        let code=pr.emit(backend).unwrap();assert!(code.contains("_MataTuple"));assert!(code.contains("value0"));
    }
}
#[test]
fn record_and_array_constructors_validate(){
    let mut pr=Program::default();
    pr.add_record(Record{name:"Cell".into(),fields:vec![Field{name:"mass".into(),ty:f32t()}]}).unwrap();
    let arr=DataType::Array(Box::new(i32t()),3);
    pr.add_kernel(kernel("construct",vec![],vec![
        Statement::Let("cell".into(),DataType::Record("Cell".into()),Value::RecordInit("Cell".into(),vec![("mass".into(),lit(2.0))])),
        Statement::Let("items".into(),arr.clone(),Value::ArrayInit(vec![Value::Int(1,i32t()),Value::Int(2,i32t()),Value::Int(3,i32t())],arr)),
    ])).unwrap();
    pr.validate().unwrap();
    assert!(pr.emit(Backend::IntelOpenCl).unwrap().contains("int items[3] = {1, 2, 3}"));
    assert!(pr.emit(Backend::Wgsl).unwrap().contains("array<i32, 3>"));
}
#[test]
fn record_dependencies_are_emitted_in_order(){
    let mut pr=Program::default();
    pr.add_record(Record{name:"Outer".into(),fields:vec![Field{name:"inner".into(),ty:DataType::Record("Inner".into())}]}).unwrap();
    pr.add_record(Record{name:"Inner".into(),fields:vec![Field{name:"v".into(),ty:f32t()}]}).unwrap();
    let source=pr.emit(Backend::Glsl).unwrap();
    assert!(source.find("struct Inner").unwrap()<source.find("struct Outer").unwrap());
}
#[test]
fn conditional_return_paths_are_enforced(){
    let mut pr=Program::default();
    pr.add_function(Function{name:"select".into(),args:vec![p("cond",DataType::Scalar(Number::Bool))],result:f32t(),body:vec![
        Statement::If(variable("cond",DataType::Scalar(Number::Bool)).unwrap(),vec![Statement::Return(Some(lit(1.0)))],vec![])
    ]}).unwrap();
    assert_eq!(pr.validate(),Err(IrError::InvalidControlFlow));
    pr.functions[0].body.push(Statement::Return(Some(lit(2.0))));
    assert!(pr.validate().is_ok());
}
#[test]
fn multiple_shader_kernels_must_be_selected(){
    let mut pr=Program::default();
    pr.add_kernel(kernel("one",vec![],vec![])).unwrap();
    pr.add_kernel(kernel("two",vec![],vec![])).unwrap();
    assert!(matches!(pr.emit(Backend::Glsl),Err(IrError::Unsupported{..})));
    assert!(pr.emit_kernel("two",Backend::Glsl).unwrap().contains("void main()"));
    assert!(pr.emit(Backend::NvidiaCuda).unwrap().contains("void two()"));
}
#[test]
fn readonly_writes_and_writeonly_reads_are_rejected(){
    let ro=DataType::Pointer(Box::new(f32t()),AddressSpace::Global,Access::ReadOnly);
    let wo=DataType::Pointer(Box::new(f32t()),AddressSpace::Global,Access::WriteOnly);
    let mut pr=program_kernel(kernel("bad",vec![p("x",ro.clone())],vec![
        Statement::Store(variable("x",ro).unwrap(),global_id(0).unwrap(),lit(1.0))]));
    assert_eq!(pr.validate(),Err(IrError::TypeMismatch));
    pr=program_kernel(kernel("bad2",vec![p("x",wo.clone())],vec![
        Statement::Let("y".into(),f32t(),Value::Load(Box::new(variable("x",wo).unwrap()),Box::new(global_id(0).unwrap()),f32t()))]));
    assert_eq!(pr.validate(),Err(IrError::TypeMismatch));
}
#[test]
fn shader_layout_rejects_bool_buffer_and_local_pointer(){
    let buf=DataType::Pointer(Box::new(DataType::Scalar(Number::Bool)),AddressSpace::Global,Access::ReadWrite);
    let pr=program_kernel(kernel("bad",vec![p("b",buf)],vec![]));
    assert!(pr.emit(Backend::Wgsl).is_err());
    let local=DataType::Pointer(Box::new(f32t()),AddressSpace::Workgroup,Access::ReadWrite);
    let pr=program_kernel(kernel("bad",vec![p("b",local)],vec![]));
    assert!(matches!(pr.emit(Backend::Wgsl),Err(IrError::Unsupported{..})));
}
#[test]
fn uniform_parameter_packing_is_computable(){
    let prog=program_kernel(kernel("uniforms",vec![p("a",f32t()),p("b",DataType::Vector(Number::F32,3)),p("c",f32t())],vec![]));
    let wg=prog.kernel_uniform_layout("uniforms",Backend::Wgsl).unwrap();
    assert_eq!(wg.fields[0].offset,0);
    assert_eq!(wg.fields[1].offset,16);
    assert_eq!(wg.fields[2].offset,28);
    let hlsl=prog.kernel_uniform_layout("uniforms",Backend::Hlsl).unwrap();
    assert_eq!(hlsl.fields[0].offset,0);
    assert_eq!(hlsl.fields[1].offset,16);
    assert_eq!(hlsl.fields[2].offset,28);
}
#[test]
fn u64_literal_and_double_constant_are_supported_where_types_exist(){
    let prog=program_kernel(kernel("wide",vec![],vec![
        Statement::Let("large".into(),DataType::Scalar(Number::U64),Value::UInt(u64::MAX,DataType::Scalar(Number::U64))),
        Statement::Let("precise".into(),DataType::Scalar(Number::F64),Value::F64Bits(1.25f64.to_bits()))
    ]));
    assert!(prog.validate().is_ok());
    assert!(prog.emit(Backend::IntelOpenCl).unwrap().contains("18446744073709551615ULL"));
    assert!(prog.emit(Backend::NvidiaCuda).unwrap().contains("double precise"));
    assert!(prog.emit(Backend::Wgsl).is_err());
}
#[test]
fn local_array_indexing_and_writes_emit(){
    let array=DataType::Array(Box::new(i32t()),3);
    let init=Value::ArrayInit(vec![Value::Int(1,i32t()),Value::Int(2,i32t()),Value::Int(3,i32t())],array.clone());
    let prog=program_kernel(kernel("arrays",vec![],vec![
        Statement::Let("values".into(),array.clone(),init),
        Statement::SetIndex(variable("values",array.clone()).unwrap(),Value::Int(1,i32t()),Value::Int(8,i32t())),
        Statement::Let("element".into(),i32t(),Value::Index(Box::new(variable("values",array).unwrap()),Box::new(Value::Int(1,i32t())),i32t()))
    ]));
    prog.validate().unwrap();
    let cl=prog.emit(Backend::IntelOpenCl).unwrap();
    assert!(cl.contains("values[1] = 8;"));
    assert!(cl.contains("values[1];"));
}
#[test]
fn direct_assignment_to_parameter_is_forbidden_in_portable_ir(){
    let prog=program_kernel(kernel("immutable",vec![p("factor",f32t())],vec![Statement::Assign("factor".into(),lit(2.0))]));
    assert_eq!(prog.validate(),Err(IrError::InvalidControlFlow));
}
