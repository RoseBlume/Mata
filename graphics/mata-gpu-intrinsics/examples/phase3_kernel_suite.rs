//! Generate Phase 3 compute kernels for CUDA C++, HIP C++ and OpenCL C.
//! Usage: cargo run --example phase3_kernel_suite -- /path/to/output
#[path = "../tests/support/phase3_kernels.rs"]
mod cases;
use mata_gpu_intrinsics::{ir::TargetProfile,Backend};
use std::{env,fs,path::PathBuf};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let dest=env::args().nth(1).ok_or("expected output directory as first argument")?;
    let dest=PathBuf::from(dest);fs::create_dir_all(&dest)?;
    for (name,program) in [
        ("saxpy",cases::saxpy()),
        ("reduction",cases::reduction()),
        ("stencil",cases::stencil()),
        ("histogram",cases::histogram()),
    ] {
        for (backend,extension) in [
            (Backend::IntelOpenCl,"cl"),(Backend::NvidiaCuda,"cu"),(Backend::AmdHip,"hip.cpp")
        ] {
            let profile=TargetProfile::offline(backend);
            let source=program.emit_for(&profile).map_err(|e|format!("{name}: {e}"))?;
            let path=dest.join(format!("{name}.{extension}"));
            fs::write(&path,source)?;
            println!("{}",path.display());
        }
    }
    Ok(())
}
