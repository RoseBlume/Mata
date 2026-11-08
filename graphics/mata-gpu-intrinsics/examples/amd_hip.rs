//! Generate a HIP C++ kernel with 64-bit wave ballots, without an AMD GPU.
use mata_gpu_intrinsics::{amd::KernelBuilder,Capability,CapabilitySet,Expr,Intrinsic,Type};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let caps=CapabilitySet::CORE.with(Capability::AmdWave);
    let mut kernel=KernelBuilder::new("wave_ballot",caps)?;
    let output=kernel.read_write_buffer("out_masks",Type::U64)?;
    let limit=kernel.value("n",Type::U32)?;
    let index=kernel.index_1d("idx")?;
    // No wave barrier after a divergent early return.
    kernel.return_if_out_of_bounds(&index,&limit)?;
    let lane=kernel.emitter().call_typed(Intrinsic::AmdLaneId,&[],Type::U32)?;
    let predicate=kernel.emitter().call(Intrinsic::Less,&[lane,Expr::uint(16)])?;
    let mask=kernel.emitter().call_typed(Intrinsic::AmdBallot,&[predicate],Type::U64)?;
    kernel.store(&output,&index,&mask)?;
    print!("{}",kernel.build()?);
    Ok(())
}
