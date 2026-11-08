//! Pure-Rust source-generation tests. No NVIDIA driver, SDK or CUDA runtime needed.
use mata_gpu_intrinsics::{
    nvidia::KernelBuilder, Backend,Capability,CapabilitySet,Emitter,Error,Expr,Intrinsic,
    Scalar,Stage,Type,ops,
};
fn gpu(caps:CapabilitySet)->Emitter{Emitter::new(Backend::NvidiaCuda,Stage::Compute,caps)}

#[test]
fn math_operator_and_types(){
    let e=gpu(CapabilitySet::CORE);
    assert_eq!(Type::U32.source(Backend::NvidiaCuda),"unsigned int");
    assert_eq!(Type::VEC4.source(Backend::NvidiaCuda),"float4");
    let x=Expr::variable("x",Type::F32).unwrap();
    assert_eq!(e.call(Intrinsic::Sin,&[x.clone()]).unwrap().code(),"sinf(x)");
    assert_eq!(e.call(Intrinsic::Saturate,&[x.clone()]).unwrap().code(),"fminf(fmaxf(x, 0.0f), 1.0f)");
    assert_eq!(e.call(Intrinsic::Fract,&[x.clone()]).unwrap().code(),"(x - floorf(x))");
    let v=e.call(Intrinsic::Add,&[x.clone(),x]).unwrap();
    assert_eq!(v.code(),"(x + x)");
    let a=Expr::variable("a",Type::I32).unwrap();
    let b=Expr::variable("b",Type::I32).unwrap();
    assert_eq!(e.call(Intrinsic::Min,&[a,b]).unwrap().code(),"min(a, b)");
    assert_eq!(e.call(Intrinsic::ReverseBits,&[Expr::uint(7)]).unwrap().code(),"__brev(7u)");
    assert!(matches!(e.call(Intrinsic::Sin,&[Expr::variable("q",Type::VEC4).unwrap()]),Err(Error::WrongTypes{..})));
    assert!(matches!(e.call_typed(Intrinsic::TextureSample,&[],Type::VEC4),Err(Error::Unsupported{..})));
    assert!(!e.supports(Intrinsic::SubgroupShuffle)); // CUDA uses masked warp syntax
    assert!(matches!(e.call(Intrinsic::AtomicLoad,&[]),Err(Error::Unsupported{..})));
    assert_eq!(e.construct(Type::VEC2,&[Expr::float(1.0).unwrap(),Expr::float(2.0).unwrap()]).unwrap().code(),"make_float2(1.0, 2.0)");
}

#[test]
fn cuda_is_compute_only_and_checks_capabilities(){
    let e=Emitter::new(Backend::NvidiaCuda,Stage::Fragment,CapabilitySet::ALL);
    assert!(!e.supports(Intrinsic::Sin));
    assert!(matches!(e.call(Intrinsic::Sin,&[Expr::float(1.0).unwrap()]),Err(Error::WrongStage{..})));
    let core=gpu(CapabilitySet::CORE);
    assert!(matches!(core.call_typed(Intrinsic::NvidiaActiveMask,&[],Type::U32),Err(Error::FeatureDisabled(Capability::NvidiaWarp))));
    let warp=gpu(CapabilitySet::CORE.with(Capability::NvidiaWarp));
    assert_eq!(warp.call_typed(Intrinsic::NvidiaActiveMask,&[],Type::U32).unwrap().code(),"__activemask()");
    assert!(matches!(warp.call_typed(Intrinsic::NvidiaReduceAddSync,&[Expr::uint(u32::MAX),Expr::uint(2)],Type::U32),Err(Error::FeatureDisabled(Capability::NvidiaSm80))));
    let sm80=gpu(CapabilitySet::CORE.with(Capability::NvidiaSm80));
    assert!(matches!(sm80.call_typed(Intrinsic::NvidiaReduceAddSync,&[Expr::uint(u32::MAX),Expr::uint(2)],Type::U32),Err(Error::FeatureDisabled(Capability::NvidiaWarp))));
}

#[test]
fn warp_masks_and_shuffle_types(){
    let e=gpu(CapabilitySet::CORE.with(Capability::NvidiaWarp));
    let mask=Expr::uint(u32::MAX);
    let value=Expr::variable("v",Type::F32).unwrap();
    let lane=Expr::uint(4);
    let shuffled=ops::nvidia_shuffle_xor_sync(&e,&[mask.clone(),value,lane],Type::F32).unwrap();
    assert_eq!(shuffled.code(),"__shfl_xor_sync(4294967295u, v, 4u)");
    let ballot=ops::nvidia_ballot_sync(&e,&[mask.clone(),Expr::boolean(true)],Type::U32).unwrap();
    assert_eq!(ballot.code(),"__ballot_sync(4294967295u, true)");
    assert_eq!(ops::nvidia_sync_warp(&e,&[mask.clone()],Type::Void).unwrap().code(),"__syncwarp(4294967295u)");
    assert!(matches!(ops::nvidia_shuffle_sync(&e,&[mask,Expr::uint(1),Expr::float(2.0).unwrap()],Type::U32),Err(Error::WrongTypes{..})));
}

#[test]
fn cuda_kernel_and_atomics(){
    let mut k=KernelBuilder::new("scale",CapabilitySet::CORE.with(Capability::Atomics)).unwrap();
    let input=k.read_only_buffer("input",Type::F32).unwrap();
    let output=k.read_write_buffer("output",Type::F32).unwrap();
    let count=k.value("count",Type::U32).unwrap();
    let i=k.index_1d("idx").unwrap();
    k.return_if_out_of_bounds(&i,&count).unwrap();
    let x=input.load(&i).unwrap();
    let doubled=k.emitter().call(Intrinsic::Multiply,&[x,Expr::float(2.0).unwrap()]).unwrap();
    k.store(&output,&i,&doubled).unwrap();
    let src=k.build().unwrap();
    assert!(src.contains("extern \"C\" __global__ void scale(const float* __restrict__ input, float* __restrict__ output, unsigned int count)"));
    assert!(src.contains("unsigned int idx = blockIdx.x * blockDim.x + threadIdx.x;"));
    assert!(src.contains("output[idx] = (input[idx] * 2.0);"));

    let mut other=KernelBuilder::new("increment",CapabilitySet::CORE.with(Capability::Atomics)).unwrap();
    let counter=other.read_write_buffer("counter",Type::U32).unwrap();
    let ptr=counter.atomic_address_at(&Expr::uint(0)).unwrap();
    let inc=other.emitter().call_typed(Intrinsic::AtomicAdd,&[ptr,Expr::uint(1)],Type::U32).unwrap();
    assert_eq!(inc.code(),"atomicAdd((counter + 0u), 1u)");
    other.statement(&inc);
    assert!(other.build().unwrap().contains("atomicAdd((counter + 0u), 1u);"));
}

#[test]
fn invalid_kernel_build_usage(){
    assert!(KernelBuilder::new("not valid",CapabilitySet::CORE).is_err());
    let mut k=KernelBuilder::new("ok",CapabilitySet::CORE).unwrap();
    let readonly=k.read_only_buffer("src",Type::I32).unwrap();
    assert!(matches!(k.store(&readonly,&Expr::uint(0),&Expr::int(2)),Err(Error::InvalidType)));
    assert!(matches!(k.read_write_buffer("src",Type::F32),Err(Error::DuplicateParameter)));
    assert!(matches!(readonly.atomic_address_at(&Expr::uint(0)),Err(Error::InvalidType)));
    let bad=KernelBuilder::new("needs_warp",CapabilitySet::CORE.with(Capability::NvidiaSm80)).unwrap();
    assert!(matches!(bad.build(),Err(Error::FeatureDisabled(Capability::NvidiaWarp))));
    let mut k=KernelBuilder::new("ok2",CapabilitySet::CORE).unwrap();
    let out=k.read_write_buffer("out",Type::F32).unwrap();
    assert!(matches!(out.atomic_address_at(&Expr::uint(0)),Err(Error::InvalidType)));
    let ix=Expr::int(0);
    let val=Expr::float(4.0).unwrap();
    k.store(&out,&ix,&val).unwrap();
}

#[test]
fn all_earlier_backends_still_available(){
    assert_eq!(Type::U32.source(Backend::IntelOpenCl),"uint");
    assert_eq!(Type::U32.source(Backend::Wgsl),"u32");
    assert_eq!(Type::U32.source(Backend::Glsl),"uint");
    assert_eq!(Type::U32.source(Backend::Hlsl),"uint");
    assert_eq!(Type::Vector(Scalar::F32,3).source(Backend::IntelOpenCl),"float3");
}
