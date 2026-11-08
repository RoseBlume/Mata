//! Show GPU-independent shader generation from the intrinsic dispatch table.
//! Run `cargo run -p mata-3d --example generate_hlsl` (no GPU required).
use mata_gpu_intrinsics::{CapabilitySet, FunctionTable, Stage};
use mata_3d::shader_dispatch::D3d11Shaders;

fn main() -> Result<(), String> {
    let table = FunctionTable::hlsl(Stage::Fragment, CapabilitySet::CORE);
    let shader = D3d11Shaders::from_table(&table)?;
    println!("HLSL functions: {} available", table.available_count());
    println!("--- D3D11 scene geometry ---\n{}", shader.geometry);
    println!("--- D3D11 scene/HUD composite ---\n{}", shader.composite);
    Ok(())
}
