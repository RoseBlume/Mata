//! Generate Phase 5 source and 6-related buffer kernels for target compilers.
use mata_gpu_intrinsics::{Backend,ir::*};
use std::{env,fs,path::PathBuf};
fn scalar(n:Number)->DataType{DataType::Scalar(n)}
fn buffer(n:Number,access:Access)->DataType{DataType::Pointer(Box::new(scalar(n)),AddressSpace::Global,access)}
fn unary(name:&str,op:ComputeOp)->Program {
    let src=buffer(Number::F32,Access::ReadOnly);let dst=buffer(Number::F32,Access::WriteOnly);
    let idx=global_id(0).unwrap();let input=load(variable("src",src.clone()).unwrap(),idx.clone()).unwrap();
    let math=compute(op,vec![input]).unwrap();
    let mut p=Program::default();
    p.add_kernel(Kernel{name:name.into(),args:vec![Parameter{name:"src".into(),ty:src},Parameter{name:"dst".into(),ty:dst.clone()}],body:vec![
        Statement::Store(variable("dst",dst).unwrap(),idx,math)],workgroup:[64,1,1]}).unwrap();p
}
fn main(){
    let folder=PathBuf::from(env::args().nth(1).expect("output directory missing"));fs::create_dir_all(&folder).unwrap();
    for (name,program) in [("log1p",unary("phase5_log1p",ComputeOp::Log1p)),("erf",unary("phase5_erf",ComputeOp::Erf))] {
        for (backend,ext) in [(Backend::IntelOpenCl,"cl"),(Backend::NvidiaCuda,"cu"),(Backend::AmdHip,"hip.cpp")] {
            let code=program.emit(backend).unwrap();
            fs::write(folder.join(format!("{name}.{ext}")),code).unwrap();
        }
    }
}
