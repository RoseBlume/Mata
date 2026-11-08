use mata_gpu_intrinsics::{ir::*,Backend};
fn f32_ty()->DataType{DataType::Scalar(Number::F32)}
#[test]
fn record_layout_and_array_stride(){
    let mut p=Program::default();
    p.add_record(Record{name:"Particle".into(),fields:vec![Field{name:"id".into(),ty:DataType::Scalar(Number::U32)},Field{name:"location".into(),ty:DataType::Vector(Number::F32,3)},Field{name:"mass".into(),ty:f32_ty()}]}).unwrap();
    let layout=p.record_layout("Particle").unwrap();
    assert_eq!(layout.fields[1].offset,16);
    assert_eq!(layout.fields[2].offset,28);
    assert_eq!(layout.layout.size,32);
    assert_eq!(p.storage_layout(&DataType::Array(Box::new(DataType::Vector(Number::F32,3)),4)).unwrap().stride,16);
}
#[test]
fn shader_scalar_uniforms(){
    let mut p=Program::default();
    p.add_kernel(Kernel{name:"dispatch".into(),args:vec![Parameter{name:"scale".into(),ty:f32_ty()}],body:vec![],workgroup:[8,1,1]}).unwrap();
    assert!(p.emit(Backend::Wgsl).unwrap().contains("var<uniform>"));
    assert!(p.emit(Backend::Glsl).unwrap().contains("uniform float scale"));
    assert!(p.emit(Backend::Hlsl).unwrap().contains("cbuffer"));
}
#[test]
fn wrong_types_and_unbound_names_rejected(){
    let mut p=Program::default();
    p.add_kernel(Kernel{name:"bad".into(),args:vec![],body:vec![Statement::Expr(variable("missing",f32_ty()).unwrap())],workgroup:[32,1,1]}).unwrap();
    assert_eq!(p.validate(),Err(IrError::UnknownSymbol));
    let mut p=Program::default();
    p.add_kernel(Kernel{name:"bad".into(),args:vec![],body:vec![Statement::Return(Some(Value::FloatBits(0)))],workgroup:[32,1,1]}).unwrap();
    assert_eq!(p.validate(),Err(IrError::InvalidControlFlow));
}
#[test]
fn recursive_record_layout_is_rejected(){
    let mut p=Program::default();
    p.add_record(Record{name:"Rec".into(),fields:vec![Field{name:"self_field".into(),ty:DataType::Record("Rec".into())}]}).unwrap();
    assert_eq!(p.validate(),Err(IrError::InvalidLayout));
}
