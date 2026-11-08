//! Run with `cargo run --example function_table` on any computer (no GPU needed).
use mata_gpu_intrinsics::{
    Backend, CapabilitySet, Expr, FunctionTable, Intrinsic, Stage, Type,
};

fn main() -> Result<(),Box<dyn std::error::Error>> {
    for backend in [Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip,
        Backend::Wgsl,Backend::Glsl,Backend::Hlsl] {
        let table=FunctionTable::new(backend,Stage::Compute,CapabilitySet::CORE);
        let x=Expr::variable("x",Type::F32)?;
        let sin=table.get(Intrinsic::Sin).expect("sin should exist");
        let y=sin.call(&[x])?;
        println!("{backend:?}: {} ({} operations enabled)",y.code(),table.available_count());
    }
    let nvidia=FunctionTable::nvidia(CapabilitySet::CORE);
    let operation=nvidia.get_named("sin").expect("sin");
    println!("NVIDIA sin: {}",operation.call(&[Expr::float(0.5)?])?.code());
    Ok(())
}
