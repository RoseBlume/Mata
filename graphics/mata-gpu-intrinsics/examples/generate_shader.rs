//! Run `cargo run --example generate_shader` (GPU and graphics drivers not required).
use mata_gpu_intrinsics::{Backend, CapabilitySet, Emitter, Expr, FunctionBuilder, Intrinsic, Stage, Type};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for backend in [Backend::Wgsl, Backend::Glsl, Backend::Hlsl] {
        let e = Emitter::new(backend, Stage::Fragment, CapabilitySet::CORE);
        let mut function = FunctionBuilder::new(e, "shade", Type::VEC3)?;
        let normal = function.parameter("normal", Type::VEC3)?;
        let light = function.parameter("light", Type::VEC3)?;
        let n = e.call(Intrinsic::Normalize, &[normal])?;
        let l = e.call(Intrinsic::Normalize, &[light])?;
        let dot = e.call(Intrinsic::Dot, &[n, l])?;
        let diffuse = e.call(Intrinsic::Max, &[dot, Expr::float(0.0)?])?;
        let value = e.construct(Type::VEC3, &[diffuse.clone(), diffuse.clone(), diffuse])?;
        function.return_expr(&value)?;
        println!("{:?}:\n{}", backend, function.build()?);
    }
    Ok(())
}
