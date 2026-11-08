use mata_gpu_intrinsics::{Backend, ir::*};
fn ptr(element:Number,access:Access)->DataType{DataType::Pointer(Box::new(DataType::Scalar(element)),AddressSpace::Global,access)}
fn v(name:&str,ty:DataType)->Value{variable(name,ty).unwrap()}
fn make(op:ComputeOp,number:Number)->Program{
    let buf=ptr(number,Access::ReadOnly);let out=ptr(number,Access::WriteOnly);
    let idx=global_id(0).unwrap();
    let x=load(v("src",buf.clone()),idx.clone()).unwrap();
    let result=compute(op,vec![x]).unwrap();
    let mut program=Program::default();
    program.add_kernel(Kernel{name:"math_kernel".into(),args:vec![Parameter{name:"src".into(),ty:buf},Parameter{name:"dst".into(),ty:out.clone()}],
        body:vec![Statement::Store(v("dst",out),idx,result)],workgroup:[64,1,1]}).unwrap();
    program
}
#[test] fn float_math_compiles_to_native_calls(){
    let p=make(ComputeOp::Log1p,Number::F32);
    assert!(p.emit(Backend::IntelOpenCl).unwrap().contains("log1p("));
    assert!(p.emit(Backend::NvidiaCuda).unwrap().contains("log1pf("));
    assert!(p.emit(Backend::AmdHip).unwrap().contains("log1pf("));
    for op in [ComputeOp::Cbrt,ComputeOp::Expm1,ComputeOp::Erf,ComputeOp::Erfc,
        ComputeOp::Atan,ComputeOp::Sinh,ComputeOp::Log10,ComputeOp::Asinh] {
        for backend in [Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip]{assert!(make(op,Number::F32).emit(backend).is_ok(),"{op:?} on {backend:?}")}
    }
}
#[test] fn integer_operations_have_real_capability_boundaries(){
    let p=make(ComputeOp::Popcount,Number::U32);
    assert!(p.emit(Backend::IntelOpenCl).unwrap().contains("popcount("));
    assert!(p.emit(Backend::NvidiaCuda).is_err());
    assert!(p.emit(Backend::AmdHip).is_err());
    assert!(compute(ComputeOp::AbsDiff,vec![v("a",DataType::Scalar(Number::I32)),v("b",DataType::Scalar(Number::I32))]).is_err());
}
#[test] fn arity_and_type_mismatches(){
    let x=v("x",DataType::Scalar(Number::F32));
    assert!(compute(ComputeOp::Atan2,vec![x.clone()]).is_err());
    assert!(compute(ComputeOp::Log1p,vec![x.clone(),x.clone()]).is_err());
    assert!(compute(ComputeOp::Rotate,vec![x.clone(),x.clone()]).is_err());
}
#[test] fn paired_sincos_emits_a_typed_tuple(){
    let input=variable("angle",DataType::Scalar(Number::F32)).unwrap();
    let (ty,expr)={let x=compute(ComputeOp::SinCos,vec![input]).unwrap();(x.ty(),x)};
    assert_eq!(ty,DataType::Tuple(vec![DataType::Scalar(Number::F32);2]));
    let mut prog=Program::default();
    prog.add_function(Function{name:"two_angles".into(),args:vec![Parameter{name:"angle".into(),ty:DataType::Scalar(Number::F32)}],result:ty,
        body:vec![Statement::Return(Some(expr))]}).unwrap();
    for backend in [Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip] {
        let src=prog.emit(backend).unwrap();
        assert!(src.contains("sin"));assert!(src.contains("cos"));assert!(src.contains("_MataTuple"));
    }
}
#[test] fn cuda_hip_bf16_storage_is_explicit(){
    let bf=DataType::Scalar(Number::Bf16);
    let mut p=Program::default();
    p.add_kernel(Kernel{name:"copy_bf16".into(),args:vec![Parameter{name:"x".into(),ty:ptr(Number::Bf16,Access::ReadOnly)},Parameter{name:"y".into(),ty:ptr(Number::Bf16,Access::WriteOnly)}],
        body:vec![Statement::Store(v("y",ptr(Number::Bf16,Access::WriteOnly)),Value::UInt(0,DataType::Scalar(Number::U32)),load(v("x",ptr(Number::Bf16,Access::ReadOnly)),Value::UInt(0,DataType::Scalar(Number::U32))).unwrap())],
        workgroup:[1,1,1]}).unwrap();
    let cu=p.emit(Backend::NvidiaCuda).unwrap();assert!(cu.contains("<cuda_bf16.h>"));assert!(cu.contains("__nv_bfloat16"));
    let hip=p.emit(Backend::AmdHip).unwrap();assert!(hip.contains("<hip/hip_bf16.h>"));assert!(hip.contains("hip_bfloat16"));
    assert!(p.emit(Backend::IntelOpenCl).is_err());
    assert_eq!(bf,DataType::Scalar(Number::Bf16));
}
