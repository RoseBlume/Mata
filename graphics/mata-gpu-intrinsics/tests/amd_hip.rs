//! Pure host-Rust HIP source generation checks; no AMD GPU or ROCm SDK needed.
use mata_gpu_intrinsics::{amd::KernelBuilder, Backend, Capability, CapabilitySet,
    Emitter, Error, Expr, Intrinsic, Stage, Type, ops};

fn gpu(caps:CapabilitySet)->Emitter{Emitter::new(Backend::AmdHip,Stage::Compute,caps)}
#[test]
fn hip_scalar_and_types(){
    let e=gpu(CapabilitySet::CORE);
    assert_eq!(Type::U64.source(Backend::AmdHip),"unsigned long long");
    assert_eq!(Type::U32.source(Backend::AmdHip),"unsigned int");
    assert_eq!(Expr::uint64(u64::MAX).code(),"18446744073709551615ULL");
    let x=Expr::variable("x",Type::F32).unwrap();
    assert_eq!(e.call(Intrinsic::Sin,&[x.clone()]).unwrap().code(),"sinf(x)");
    assert_eq!(e.call(Intrinsic::Multiply,&[x.clone(),x]).unwrap().code(),"(x * x)");
    assert_eq!(e.call_typed(Intrinsic::AmdFastSin,&[Expr::float(1.0).unwrap()],Type::F32).unwrap().code(),"__sinf(1.0)");
    assert_eq!(e.call_typed(Intrinsic::AmdBytePerm,&[Expr::uint(1),Expr::uint(2),Expr::uint(3)],Type::U32).unwrap().code(),"__byte_perm(1u, 2u, 3u)");
    assert!(matches!(e.call_typed(Intrinsic::TextureSample,&[],Type::VEC4),Err(Error::Unsupported{..})));
    assert!(!e.supports(Intrinsic::NvidiaBallotSync));
}
#[test]
fn stage_and_wave_capability_gates(){
    let e=Emitter::new(Backend::AmdHip,Stage::Fragment,CapabilitySet::ALL);
    assert!(!e.supports(Intrinsic::Sin));
    assert!(matches!(e.call(Intrinsic::Sin,&[Expr::float(1.0).unwrap()]),Err(Error::WrongStage{..})));
    let core=gpu(CapabilitySet::CORE);
    assert!(matches!(core.call_typed(Intrinsic::AmdWaveSize,&[],Type::U32),Err(Error::FeatureDisabled(Capability::AmdWave))));
    let wave=gpu(CapabilitySet::CORE.with(Capability::AmdWave));
    assert_eq!(wave.call_typed(Intrinsic::AmdWaveSize,&[],Type::U32).unwrap().code(),"((unsigned int)warpSize)");
    assert_eq!(wave.call_typed(Intrinsic::AmdLaneId,&[],Type::U32).unwrap().code(),"((unsigned int)__lane_id())");
    assert!(matches!(wave.call_typed(Intrinsic::AmdReadLane,&[Expr::uint(12),Expr::uint(1)],Type::U32),Err(Error::FeatureDisabled(Capability::AmdGcn))));
    let gcn_only=gpu(CapabilitySet::CORE.with(Capability::AmdGcn));
    assert!(matches!(gcn_only.call_typed(Intrinsic::AmdReadFirstLane,&[Expr::uint(0)],Type::U32),Err(Error::FeatureDisabled(Capability::AmdWave))));
}
#[test]
fn wave64_ballot_and_lane_ops(){
    let e=gpu(CapabilitySet::CORE.with(Capability::AmdWave).with(Capability::AmdGcn));
    let ballot=ops::amd_ballot(&e,&[Expr::boolean(true)],Type::U64).unwrap();
    assert_eq!(ballot.ty(),Type::U64);
    assert_eq!(ballot.code(),"((unsigned long long)__ballot(true))");
    assert_eq!(ops::amd_active_mask(&e,&[],Type::U64).unwrap().code(),"((unsigned long long)__activemask())");
    assert!(matches!(ops::amd_ballot(&e,&[Expr::boolean(true)],Type::U32),Err(Error::WrongTypes{..})));
    assert!(matches!(ops::amd_shuffle(&e,&[Expr::uint(7),Expr::float(1.0).unwrap()],Type::U32),Err(Error::WrongTypes{..})));
    assert_eq!(ops::amd_shuffle_xor(&e,&[Expr::uint(7),Expr::uint(3)],Type::U32).unwrap().code(),"__shfl_xor(7u, (int)(3u), warpSize)");
    assert_eq!(ops::amd_read_first_lane(&e,&[Expr::uint(123)],Type::U32).unwrap().code(),"__builtin_amdgcn_readfirstlane(123u)");
    assert_eq!(ops::amd_ds_bpermute(&e,&[Expr::uint(9),Expr::uint(4)],Type::U32).unwrap().code(),"((unsigned int)__builtin_amdgcn_ds_bpermute(((int)(4u)) * 4, (int)(9u)))");
    assert!(matches!(ops::amd_ds_permute(&e,&[Expr::uint(2),Expr::float(1.0).unwrap()],Type::U32),Err(Error::WrongTypes{..})));
    assert_eq!(ops::amd_popcount64(&e,&[Expr::uint64(u64::MAX)],Type::U32).unwrap().code(),"__popcll(18446744073709551615ULL)");
}
#[test]
fn hip_kernel_builder_buffers_and_atomics(){
    let mut k=KernelBuilder::new("transform",CapabilitySet::CORE.with(Capability::Atomics)).unwrap();
    let input=k.read_only_buffer("input",Type::F32).unwrap();
    let output=k.read_write_buffer("output",Type::F32).unwrap();
    let count=k.value("count",Type::U32).unwrap();
    let ix=k.index_1d("i").unwrap();
    k.return_if_out_of_bounds(&ix,&count).unwrap();
    let x=input.load(&ix).unwrap();
    let out=k.emitter().call(Intrinsic::Multiply,&[x,Expr::float(2.0).unwrap()]).unwrap();
    k.store(&output,&ix,&out).unwrap();
    let shader=k.build().unwrap();
    assert!(shader.contains("#include <hip/hip_runtime.h>"));
    assert!(shader.contains("extern \"C\" __global__ void transform(const float* __restrict__ input, float* __restrict__ output, unsigned int count)"));
    assert!(shader.contains("output[i] = (input[i] * 2.0);"));
    let mut k=KernelBuilder::new("atomic",CapabilitySet::CORE.with(Capability::Atomics)).unwrap();
    let b=k.read_write_buffer("counter",Type::U32).unwrap();
    let ptr=b.atomic_address_at(&Expr::uint(0)).unwrap();
    let v=k.emitter().call_typed(Intrinsic::AtomicAdd,&[ptr,Expr::uint(1)],Type::U32).unwrap();
    assert_eq!(v.code(),"atomicAdd((counter + 0u), 1u)");
    k.statement(&v);
    assert!(k.build().unwrap().contains("atomicAdd((counter + 0u), 1u);"));
}
#[test]
fn invalid_kernel_calls(){
    assert!(KernelBuilder::new("not valid",CapabilitySet::CORE).is_err());
    let mut k=KernelBuilder::new("kernel",CapabilitySet::CORE).unwrap();
    let input=k.read_only_buffer("src",Type::F32).unwrap();
    assert!(k.store(&input,&Expr::uint(0),&Expr::float(2.0).unwrap()).is_err());
    assert!(k.read_write_buffer("src",Type::U32).is_err());
    assert!(input.atomic_address_at(&Expr::uint(0)).is_err());
    assert!(KernelBuilder::new("invalid_caps",CapabilitySet::CORE.with(Capability::AmdGcn)).unwrap().build().is_err());
}
