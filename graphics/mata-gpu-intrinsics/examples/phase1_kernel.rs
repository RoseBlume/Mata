//! Prints a Phase 1 compute kernel for one target language (no GPU required).
use mata_gpu_intrinsics::{Backend,ir::*};
fn main(){
    let arg=std::env::args().nth(1).unwrap_or_else(||"cl".into());
    let backend=match arg.as_str(){
        "wgsl"=>Backend::Wgsl,"glsl"=>Backend::Glsl,"hlsl"=>Backend::Hlsl,
        "cl"=>Backend::IntelOpenCl,"cuda"=>Backend::NvidiaCuda,"hip"=>Backend::AmdHip,
        _=>{eprintln!("use wgsl|glsl|hlsl|cl|cuda|hip");std::process::exit(2)}
    };
    let f=DataType::Scalar(Number::F32);
    let input=DataType::Pointer(Box::new(f.clone()),AddressSpace::Global,Access::ReadOnly);
    let output=DataType::Pointer(Box::new(f),AddressSpace::Global,Access::ReadWrite);
    let i=global_id(0).unwrap();
    let x=variable("x",input.clone()).unwrap();
    let y=variable("y",output.clone()).unwrap();
    let sum=binary(Binary::Add,load(x,i.clone()).unwrap(),Value::FloatBits(1.0f32.to_bits())).unwrap();
    let k=Kernel{name:"compute".into(),args:vec![Parameter{name:"x".into(),ty:input},Parameter{name:"y".into(),ty:output}],body:vec![Statement::Store(y,i,sum)],workgroup:[64,1,1]};
    let mut p=Program::default();p.add_kernel(k).unwrap();
    match p.emit(backend){Ok(src)=>print!("{}",src),Err(e)=>{eprintln!("{:?}",e);std::process::exit(1)}}
}
