use mata_gpu_intrinsics::vendor::{self,Op,Target};
fn main(){
    let mode=std::env::args().nth(1).unwrap_or_else(||"cuda".to_string());
    let source=match mode.as_str(){
        "cuda"=>vendor::emit_device_helpers(&Target::cuda(9,0),&[
            Op::CudaLaneId,Op::CudaActiveMask,Op::CudaClusterRank,
            Op::CudaClusterSync,Op::CudaWmmaF16]),
        "hip"=>vendor::emit_device_helpers(&Target::hip("gfx950").unwrap(),&[
            Op::HipReadFirstLaneU32,Op::HipFp4E2M1,Op::HipFp6E2M3]),
        _=>{eprintln!("usage: cargo run --example phase9_vendor -- [cuda|hip]");std::process::exit(2)},
    };
    match source {Ok(src)=>print!("{}",src),Err(e)=>{eprintln!("{}",e);std::process::exit(1)}}
}
