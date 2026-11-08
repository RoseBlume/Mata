//! CPU-only OpenCL C source generation; no Intel GPU required to run this example.
use mata_gpu_intrinsics::{intel::KernelBuilder,CapabilitySet,Expr,Intrinsic,Type};

fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut kernel=KernelBuilder::new("scale_and_bias",CapabilitySet::CORE)?;
    let input=kernel.read_only_buffer("input",Type::F32)?;
    let output=kernel.read_write_buffer("output",Type::F32)?;
    let count=kernel.value("count",Type::U32)?;
    let scale=kernel.value("scale",Type::F32)?;
    let idx=kernel.index_1d("i")?;
    kernel.return_if_out_of_bounds(&idx,&count)?;
    let value=input.load(&idx)?;
    let adjusted=kernel.emitter().call(Intrinsic::Multiply,&[value,scale])?;
    let biased=kernel.emitter().call(Intrinsic::Add,&[adjusted,Expr::float(0.25)?])?;
    kernel.store(&output,&idx,&biased)?;
    println!("{}",kernel.build()?);
    Ok(())
}
