//! GPU integration point: produce shader-source functions and a real OpenCL
//! compute kernel using Mata GPU Intrinsics. The existing `mata-3d` backend is
//! a CPU rasterizer. These programs are not executed until a GPU renderer
//! compiles and dispatches them on a supported graphics/compute API.
use mata_gpu_intrinsics::{
    Backend,CapabilitySet,Emitter,Expr,FunctionBuilder,Intrinsic,Stage,Type,
    intel::KernelBuilder,
};
use std::{fs,path::Path};

pub fn export_to(folder:&Path)->Result<(),String>{
    fs::create_dir_all(folder).map_err(|e|e.to_string())?;
    for (backend,name) in [(Backend::Wgsl,"lod.wgsl"),(Backend::Glsl,"lod.glsl"),
                           (Backend::Hlsl,"lod.hlsl")] {
        let source=radius_function(backend).map_err(|e|e.to_string())?;
        fs::write(folder.join(name),source).map_err(|e|e.to_string())?;
    }
    let opencl=projected_radius_opencl().map_err(|e|e.to_string())?;
    fs::write(folder.join("projected_radius.cl"),opencl).map_err(|e|e.to_string())?;
    Ok(())
}

/// Shader-side projection math mirrors `SphereLods::at`. This is an exported
/// helper function, not a complete GPU pipeline. Applications need a vertex
/// shader/fragment shader/compute dispatch around it.
fn radius_function(backend:Backend)->Result<String,mata_gpu_intrinsics::Error>{
    let e=Emitter::new(backend,Stage::Compute,CapabilitySet::CORE);
    let mut function=FunctionBuilder::new(e,"mata_projected_radius",Type::F32)?;
    let radius=function.parameter("radius",Type::F32)?;
    let depth=function.parameter("depth",Type::F32)?;
    let viewport_height=function.parameter("viewport_height",Type::F32)?;
    let guarded_depth=e.call(Intrinsic::Max,&[depth,Expr::float(0.001)?])?;
    let numerator=e.call(Intrinsic::Multiply,&[radius,viewport_height])?;
    let numerator=e.call(Intrinsic::Multiply,&[numerator,Expr::float(0.88)?])?;
    let projected=e.call(Intrinsic::Divide,&[numerator,guarded_depth])?;
    function.return_expr(&projected)?;
    function.build()
}

/// One work item per atom, reading projected depth and radius to generate a
/// GPU-resident array of projected radii for subsequent LOD selection.
fn projected_radius_opencl()->Result<String,mata_gpu_intrinsics::Error>{
    let mut kernel=KernelBuilder::new("mata_projected_radius_kernel",CapabilitySet::CORE)?;
    let radii=kernel.read_only_buffer("radii",Type::F32)?;
    let depths=kernel.read_only_buffer("depths",Type::F32)?;
    let output=kernel.read_write_buffer("pixels",Type::F32)?;
    let height=kernel.value("viewport_height",Type::F32)?;
    let count=kernel.value("atom_count",Type::U32)?;
    let idx=kernel.index_1d("i")?;
    kernel.return_if_out_of_bounds(&idx,&count)?;
    let denominator=kernel.emitter().call(Intrinsic::Max,
        &[depths.load(&idx)?,Expr::float(0.001)?])?;
    let numerator=kernel.emitter().call(Intrinsic::Multiply,&[radii.load(&idx)?,height])?;
    let numerator=kernel.emitter().call(Intrinsic::Multiply,&[numerator,Expr::float(0.88)?])?;
    let result=kernel.emitter().call(Intrinsic::Divide,&[numerator,denominator])?;
    kernel.store(&output,&idx,&result)?;
    kernel.build()
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn shader_generation_has_expected_kernel_code(){
        for backend in [Backend::Wgsl,Backend::Glsl,Backend::Hlsl] {
            let s=radius_function(backend).expect("backend codegen");
            assert!(s.contains("mata_projected_radius"));
        }
        let cl=projected_radius_opencl().expect("opencl codegen");
        assert!(cl.contains("mata_projected_radius_kernel"));
    }
}
