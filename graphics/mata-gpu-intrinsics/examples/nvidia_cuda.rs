//! Run on any machine: `cargo run --example nvidia_cuda`
//! Produces CUDA C++ device source only. It does not execute the kernel.
use mata_gpu_intrinsics::{nvidia::KernelBuilder,CapabilitySet,Expr,Intrinsic,Type};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut k=KernelBuilder::new("scale_by_two",CapabilitySet::CORE)?;
    let input=k.read_only_buffer("input",Type::F32)?;
    let output=k.read_write_buffer("output",Type::F32)?;
    let n=k.value("count",Type::U32)?;
    let i=k.index_1d("idx")?;
    k.return_if_out_of_bounds(&i,&n)?;
    let val=input.load(&i)?;
    let result=k.emitter().call(Intrinsic::Multiply,&[val,Expr::float(2.0)?])?;
    k.store(&output,&i,&result)?;
    let source=k.build()?;
    println!("{}",source);
    Ok(())
}
