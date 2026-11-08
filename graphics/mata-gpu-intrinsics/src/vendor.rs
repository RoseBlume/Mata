//! Opt-in Phase 9 extensions. No PTX/GCN instructions are ever accepted as
//! unchecked user-supplied strings. A verified target architecture is required.
//! Hardware declarations do not imply that the running machine has a GPU.
use alloc::{format, string::String, vec::Vec};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Cuda { sm_major: u8, sm_minor: u8 },
    Hip { gfx: String },
}
impl Target {
    pub const fn cuda(major:u8, minor:u8)->Self {Self::Cuda{sm_major:major,sm_minor:minor}}
    pub fn hip(name:&str)->Result<Self,VendorError>{
        if !(name.len()>=6 && name.len()<=12 && name.starts_with("gfx") &&
            name[3..].bytes().all(|b| b.is_ascii_alphanumeric())) {
            return Err(VendorError::InvalidArchitecture);
        }
        Ok(Self::Hip{gfx:name.into()})
    }
    pub fn cuda_sm(&self)->Option<u16>{match self{Self::Cuda{sm_major,sm_minor}=>Some((*sm_major as u16)*10+*sm_minor as u16),_=>None}}
    pub fn is_cdna4(&self)->bool {matches!(self, Self::Hip{gfx} if gfx == "gfx950")}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Op {
    CudaLaneId, CudaActiveMask, CudaShuffleXorU32, CudaBallot,
    CudaClusterRank, CudaClusterSync,
    CudaWmmaF16, CudaTmaBulkCopy,
    HipReadFirstLaneU32, HipWaveBarrier, HipBallot,
    HipFp4E2M1, HipFp6E2M3, HipFp6E3M2,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum VendorError {
    InvalidArchitecture,
    UnsupportedTarget { operation:Op, target:Target },
    MinimumArchitecture { operation:Op, required:&'static str },
    MissingCompilerFeature(&'static str),
    InvalidMiniFloatCode,
    InvalidLaunch(&'static str),
}
impl core::fmt::Display for VendorError {
    fn fmt(&self,f:&mut core::fmt::Formatter<'_>)->core::fmt::Result{write!(f,"{:?}",self)}
}
#[cfg(feature="std")]
impl std::error::Error for VendorError {}

/// Explicit list of available snippets. The generated code is an independent
/// device-function translation unit to concatenate before an IR-emitted kernel.
/// TMA needs descriptor/barrier coordination and is withheld until its native
/// launch and synchronization contract can be validated end to end.
pub fn check(op:Op, target:&Target)->Result<(),VendorError>{
    use Op::*;
    let is_cuda=matches!(op,CudaLaneId|CudaActiveMask|CudaShuffleXorU32|CudaBallot|CudaClusterRank|CudaClusterSync|CudaWmmaF16|CudaTmaBulkCopy);
    if is_cuda {
        let sm=target.cuda_sm().ok_or_else(||VendorError::UnsupportedTarget{operation:op,target:target.clone()})?;
        let min=match op {CudaClusterRank|CudaClusterSync|CudaTmaBulkCopy=>90,CudaWmmaF16=>70,_=>30};
        if sm<min{return Err(VendorError::MinimumArchitecture{operation:op,required:if min==90{"sm_90"}else if min==70{"sm_70"}else{"sm_30"}})}
    }else {
        if !matches!(target,Target::Hip{..}){return Err(VendorError::UnsupportedTarget{operation:op,target:target.clone()})}
        if matches!(op,HipFp4E2M1|HipFp6E2M3|HipFp6E3M2) && !target.is_cdna4(){
            return Err(VendorError::MinimumArchitecture{operation:op,required:"CDNA4 gfx95* (per supplied HIP reference)"});
        }
    }
    if op==Op::CudaTmaBulkCopy {
        return Err(VendorError::MissingCompilerFeature("native tensor map descriptor and mbarrier coordination are not yet implemented"));
    }
    Ok(())
}
/// Include the declarations after the original generated includes and before
/// kernel entries. Requires a compiler matching the specified architecture.
pub fn emit_device_helpers(target:&Target,ops:&[Op])->Result<String,VendorError>{
    let mut result=String::new();
    for &op in ops {check(op,target)?;}
    if ops.iter().any(|x|matches!(x,Op::CudaClusterRank|Op::CudaClusterSync)) {
        result.push_str("#include <cooperative_groups.h>\n");
    }
    if ops.iter().any(|x|matches!(x,Op::CudaWmmaF16)) {
        result.push_str("#include <mma.h>\n#include <cuda_fp16.h>\n");
    }
    if ops.iter().any(|x|matches!(x,Op::HipFp4E2M1)) {result.push_str("#include <hip/hip_fp4.h>\n");}
    if ops.iter().any(|x|matches!(x,Op::HipFp6E2M3|Op::HipFp6E3M2)) {result.push_str("#include <hip/hip_fp6.h>\n");}
    for &op in ops {
        use Op::*;
        let body=match op {
            CudaLaneId => "__device__ __forceinline__ unsigned _mata_cuda_lane_id() { unsigned r; asm volatile(\"mov.u32 %0, %laneid;\" : \"=r\"(r)); return r; }\n",
            CudaActiveMask => "__device__ __forceinline__ unsigned _mata_cuda_active_mask() { return __activemask(); }\n",
            CudaShuffleXorU32 => "__device__ __forceinline__ unsigned _mata_cuda_shuffle_xor(unsigned mask, unsigned v, int lane_delta) { return __shfl_xor_sync(mask, v, lane_delta); }\n",
            CudaBallot => "__device__ __forceinline__ unsigned _mata_cuda_ballot(unsigned mask, int predicate) { return __ballot_sync(mask, predicate); }\n",
            CudaClusterRank => "__device__ __forceinline__ unsigned _mata_cuda_cluster_rank() { return cooperative_groups::this_cluster().block_rank(); }\n",
            CudaClusterSync => "__device__ __forceinline__ void _mata_cuda_cluster_sync() { cooperative_groups::this_cluster().sync(); }\n",
            HipReadFirstLaneU32 => "__device__ __forceinline__ unsigned _mata_hip_read_first_lane(unsigned v) { return __builtin_amdgcn_readfirstlane(v); }\n",
            HipWaveBarrier => "__device__ __forceinline__ void _mata_hip_wave_barrier() { __builtin_amdgcn_wave_barrier(); }\n",
            HipBallot => "__device__ __forceinline__ unsigned long long _mata_hip_ballot(int predicate) { return __ballot(predicate); }\n",
            HipFp4E2M1 => "__device__ __forceinline__ __hip_fp4_storage_t _mata_hip_to_fp4(float v) { return __hip_cvt_float_to_fp4(v, __HIP_E2M1, __HIP_SATFINITE); }\n",
            HipFp6E2M3 => "__device__ __forceinline__ __hip_fp6_storage_t _mata_hip_to_fp6_e2m3(float v) { return __hip_cvt_float_to_fp6(v, __HIP_E2M3, __HIP_SATFINITE); }\n",
            HipFp6E3M2 => "__device__ __forceinline__ __hip_fp6_storage_t _mata_hip_to_fp6_e3m2(float v) { return __hip_cvt_float_to_fp6(v, __HIP_E3M2, __HIP_SATFINITE); }\n",
            CudaWmmaF16 => "__device__ __forceinline__ void _mata_cuda_wmma_f16_16x16x16(const half* a, const half* b, float* c, int lda, int ldb, int ldc) {\n    using namespace nvcuda;\n    wmma::fragment<wmma::matrix_a, 16, 16, 16, half, wmma::row_major> af;\n    wmma::fragment<wmma::matrix_b, 16, 16, 16, half, wmma::row_major> bf;\n    wmma::fragment<wmma::accumulator, 16, 16, 16, float> cf;\n    wmma::fill_fragment(cf, 0.0f);\n    wmma::load_matrix_sync(af, a, lda);\n    wmma::load_matrix_sync(bf, b, ldb);\n    wmma::mma_sync(cf, af, bf, cf);\n    wmma::store_matrix_sync(c, cf, ldc, wmma::mem_row_major);\n}\n",
            CudaTmaBulkCopy => return Err(VendorError::MissingCompilerFeature("TMA descriptor, mbarrier, and transaction ordering need native CUDA implementation")),
        };
        if !result.contains(body){result.push_str(body);}
    }
    Ok(result)
}
/// CUDA cluster launches must divide the grid into whole clusters. SM90+
/// is required; native cuLaunchKernelEx / cudaLaunchKernelEx is not implied.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct CudaClusterConfig {pub grid_blocks:[u32;3],pub block_threads:[u32;3],pub cluster_blocks:[u32;3]}
impl CudaClusterConfig {
    pub fn validate(self,target:&Target)->Result<(),VendorError>{
        check(Op::CudaClusterSync,target)?;
        let mut threads=1u64;
        let mut blocks=1u64;
        for i in 0..3 {
            let (grid,local,cluster)=(self.grid_blocks[i],self.block_threads[i],self.cluster_blocks[i]);
            if grid==0||local==0||cluster==0||grid%cluster!=0{return Err(VendorError::InvalidLaunch("grid, block, cluster dimensions must be positive and clusters must divide the grid"))}
            threads*=local as u64;blocks*=cluster as u64;
        }
        if threads>1024||blocks>8 {return Err(VendorError::InvalidLaunch("over 1024 threads per block or over 8 blocks in portable cluster baseline"))}
        Ok(())
    }
}
/// Feature manifest for architecture-aware feature selection.
pub fn supported_ops(target:&Target)->Vec<Op>{
    use Op::*;
    [CudaLaneId,CudaActiveMask,CudaShuffleXorU32,CudaBallot,CudaClusterRank,CudaClusterSync,CudaWmmaF16,CudaTmaBulkCopy,
        HipReadFirstLaneU32,HipWaveBarrier,HipBallot,HipFp4E2M1,HipFp6E2M3,HipFp6E3M2]
        .iter().copied().filter(|&op|check(op,target).is_ok()).collect()
}
/// Host-side static compiler flags guidance, not an executed compiler command.
pub fn target_compile_option(target:&Target)->String {
    match target{Target::Cuda{sm_major,sm_minor}=>format!("--gpu-architecture=compute_{}{}",sm_major,sm_minor),Target::Hip{gfx}=>format!("--offload-arch={}",gfx)}
}

/// Decoded finite value for the unscaled OCP/ROCm E2M1, E2M3, E3M2
/// storage formats. These are unscaled value conversions, not MX block formats.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum MiniFloat { E2M1, E2M3, E3M2 }
impl MiniFloat {
    pub const fn bits(self)->u8 {match self{Self::E2M1=>4,Self::E2M3|Self::E3M2=>6}}
    const fn parts(self)->(u8,u8,i32){match self{Self::E2M1=>(2,1,1),Self::E2M3=>(2,3,1),Self::E3M2=>(3,2,3)}}
    pub fn decode(self, bits:u8)->Result<f32,VendorError>{
        if bits >= (1<<self.bits()){return Err(VendorError::InvalidMiniFloatCode)}
        let (e,m,bias)=self.parts();let sign=1u8<<(e+m);
        let exponent=((bits>>m)&((1<<e)-1)) as i32;
        let fraction=(bits&((1<<m)-1)) as f32/(1u32<<m) as f32;
        fn power_of_two(exponent:i32)->f32 {
            if exponent>=0 {(1u32<<(exponent as u32)) as f32}
            else {1.0 / (1u32<<((-exponent) as u32)) as f32}
        }
        let abs=if exponent==0{fraction*power_of_two(1-bias)}
                else{(1.0+fraction)*power_of_two(exponent-bias)};
        Ok(if bits&sign!=0 {-abs}else{abs})
    }
    /// Nearest representable finite code, ties to even (least-significant
    /// storage bit); out-of-range finite inputs saturate to finite maximum.
    /// NaN and infinity are rejected instead of guessing HIP's saturation policy.
    pub fn encode(self, input:f32)->Result<u8,VendorError>{
        if !input.is_finite(){return Err(VendorError::InvalidLaunch("mini-float input must be finite"))}
        let neg=input.is_sign_negative();let sign_bit=1u8<<(self.bits()-1);
        let magnitude=input.abs();let mut best=0u8;let mut distance=f32::INFINITY;
        for code in 0..sign_bit {
            let decoded=self.decode(code)?;
            let diff=(decoded-magnitude).abs();
            if diff<distance || diff==distance && (code&1)==0 && (best&1)!=0 {
                best=code;distance=diff;
            }
        }
        Ok(if neg {best|sign_bit}else{best})
    }
}
/// Little-endian low-bit-first packed storage, not a C/C++ native ABI guarantee.
pub fn pack_codes(kind:MiniFloat,items:&[u8])->Result<Vec<u8>,VendorError>{
    let width=kind.bits() as usize;
    let total=items.len().checked_mul(width).ok_or(VendorError::InvalidLaunch("packed code count overflow"))?;
    let mut bytes=alloc::vec![0u8;(total+7)/8];
    for (i,&code) in items.iter().enumerate(){
        if code >= (1<<width){return Err(VendorError::InvalidLaunch("mini-float code exceeds its storage width"))}
        for bit in 0..width {if (code>>bit)&1!=0 {let position=i*width+bit;bytes[position/8]|=1u8<<(position%8)}}
    }
    Ok(bytes)
}
pub fn unpack_codes(kind:MiniFloat,bytes:&[u8],count:usize)->Result<Vec<u8>,VendorError>{
    let width=kind.bits() as usize;
    let bits=count.checked_mul(width).ok_or(VendorError::InvalidLaunch("packed code count overflow"))?;
    if bits>bytes.len().saturating_mul(8){return Err(VendorError::InvalidLaunch("packed input is too short"))}
    let mut output=Vec::with_capacity(count);
    for i in 0..count {let mut code=0u8;for bit in 0..width {
        let offset=i*width+bit;code|=((bytes[offset/8]>>(offset%8))&1)<<bit;
    }output.push(code)}
    Ok(output)
}
