//! Game-specific shader dispatch policy, shared by all Windows GPU sessions.
//!
//! This module composes a real D3D11 shader using the backend's CPU-side
//! function pointer table. It does **not** discover or execute CUDA/HIP/OpenCL
//! compute kernels, which need separate drivers and runtime dispatch.
use mata_gpu_intrinsics::{CapabilitySet, FunctionTable, Intrinsic, Stage};
use mata_3d::shader_dispatch::D3d11Shaders;

/// Generate the game scene and HUD shaders with verified HLSL function pointers.
/// This works regardless of the host computer's GPU or operating system.
/// The Direct3D runtime compiles the returned HLSL on Windows only.
pub fn make_shader_program() -> Result<D3d11Shaders, String> {
    let table = FunctionTable::hlsl(Stage::Fragment, CapabilitySet::CORE);
    for op in [Intrinsic::Saturate, Intrinsic::Mix] {
        if table.get(op).is_none() {
            return Err(format!("shader dispatch missing required {:?} intrinsic",op));
        }
    }
    D3d11Shaders::from_table(&table)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn game_shader_generation_matches_the_rendering_library() {
        assert_eq!(make_shader_program().unwrap(),D3d11Shaders::new().unwrap());
    }
}
