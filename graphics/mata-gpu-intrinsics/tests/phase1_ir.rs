use mata_gpu_intrinsics::{Backend,ir::*};
fn create_axpy() -> Program {
    let f32_ty=DataType::Scalar(Number::F32);
    let buffer_ty=DataType::Pointer(Box::new(f32_ty.clone()),AddressSpace::Global,Access::ReadWrite);
    let in_ty=DataType::Pointer(Box::new(f32_ty.clone()),AddressSpace::Global,Access::ReadOnly);
    let x=variable("x",in_ty.clone()).unwrap();
    let out=variable("out",buffer_ty.clone()).unwrap();
    let i=global_id(0).unwrap();
    let data=load(x,i.clone()).unwrap();
    let result=binary(Binary::Add,data,Value::FloatBits(1f32.to_bits())).unwrap();
    let k=Kernel {name:"increment".into(),args:vec![
        Parameter{name:"x".into(),ty:in_ty},Parameter{name:"out".into(),ty:buffer_ty}],
        body:vec![Statement::Store(out,i,result)],workgroup:[64,1,1]};
    let mut p=Program::default();p.add_kernel(k).unwrap();p
}
#[test]
fn generates_opencl_cuda_hip() {
    let p=create_axpy();
    let cl=p.emit(Backend::IntelOpenCl).unwrap();
    assert!(cl.contains("__kernel void increment"));
    assert!(cl.contains("get_global_id(0)"));
    assert!(cl.contains("const __global float* x"));
    for backend in [Backend::NvidiaCuda,Backend::AmdHip]{
        let src=p.emit(backend).unwrap();
        assert!(src.contains("__global__ void increment"));
        assert!(src.contains("blockIdx.x * blockDim.x + threadIdx.x"));
    }
}
#[test]
fn shader_bindings_are_emitted() {
    let p=create_axpy();
    let wgsl=p.emit(Backend::Wgsl).unwrap();
    assert!(wgsl.contains("@group(0) @binding(0)"));
    assert!(wgsl.contains("@compute @workgroup_size(64, 1, 1)"));
    let glsl=p.emit(Backend::Glsl).unwrap();
    assert!(glsl.contains("layout(std430, binding = 0)"));
    assert!(glsl.contains("gl_GlobalInvocationID.x"));
    let hlsl=p.emit(Backend::Hlsl).unwrap();
    assert!(hlsl.contains("StructuredBuffer<float> x"));
    assert!(hlsl.contains("RWStructuredBuffer<float> out"));
}
#[test]
fn types_and_record_validation() {
    assert!(DataType::Vector(Number::I16,16).validate().is_ok());
    assert!(DataType::Vector(Number::F32,5).validate().is_err());
    let mut p=Program::default();
    p.add_record(Record{name:"Particle".into(),fields:vec![Field{name:"mass".into(),ty:DataType::Scalar(Number::F32)}]}).unwrap();
    assert!(p.add_record(Record{name:"Particle".into(),fields:vec![]}).is_err());
    assert!(p.emit(Backend::NvidiaCuda).unwrap().contains("typedef struct Particle"));
}
#[test]
fn structured_branch_loop_function() {
    let i=variable("i",DataType::Scalar(Number::I32)).unwrap();
    let end=Value::Int(3,DataType::Scalar(Number::I32));
    let cond=binary(Binary::Lt,i,end.clone()).unwrap();
    let mut p=Program::default();
    p.add_function(Function{name:"helper".into(),args:vec![],result:DataType::Void,
        body:vec![Statement::ForRange{index:"i".into(),start:Value::Int(0,DataType::Scalar(Number::I32)),end,
        body:vec![Statement::If(cond,vec![Statement::Barrier],vec![])]},Statement::Return(None)]}).unwrap();
    let src=p.emit(Backend::IntelOpenCl).unwrap();
    assert!(src.contains("for (int i = 0; i < 3; i++)"));
    assert!(src.contains("barrier(CLK_LOCAL_MEM_FENCE)"));
}
