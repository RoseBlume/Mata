//! All of these tests run without a GPU, driver or OpenCL library.
use mata_gpu_intrinsics::{
    intel::KernelBuilder,Backend,Capability,CapabilitySet,Emitter,Error,Expr,Intrinsic,Stage,Type,Scalar,ops,
};
fn emitter(caps:CapabilitySet)->Emitter {Emitter::new(Backend::IntelOpenCl,Stage::Compute,caps)}

#[test]
fn intel_opencl_type_names_and_math(){
    assert_eq!(Type::F32.source(Backend::IntelOpenCl),"float");
    assert_eq!(Type::U32.source(Backend::IntelOpenCl),"uint");
    assert_eq!(Type::VEC4.source(Backend::IntelOpenCl),"float4");
    assert_eq!(Type::Texture2D.source(Backend::IntelOpenCl),"image2d_t");
    let e=emitter(CapabilitySet::CORE);
    let x=Expr::variable("x",Type::F32).unwrap();
    assert_eq!(e.call(Intrinsic::Sin,&[x.clone()]).unwrap().code(),"sin(x)");
    assert_eq!(e.call(Intrinsic::Abs,&[x]).unwrap().code(),"fabs(x)");
    let i=Expr::variable("counter",Type::I32).unwrap();
    assert_eq!(e.call(Intrinsic::Abs,&[i]).unwrap().code(),"abs(counter)");
    assert_eq!(e.call(Intrinsic::CountOneBits,&[Expr::uint(0x7)]).unwrap().code(),"popcount(7u)");
    let constructed=e.construct(Type::VEC2,&[Expr::float(1.0).unwrap(),Expr::float(2.0).unwrap()]).unwrap();
    assert_eq!(constructed.code(),"(float2)(1.0, 2.0)");
}
#[test]
fn intel_opencl_requires_compute_and_explicit_extensions(){
    let e=Emitter::new(Backend::IntelOpenCl,Stage::Vertex,CapabilitySet::ALL);
    assert!(!e.supports(Intrinsic::Sin));
    assert!(matches!(e.call(Intrinsic::Sin,&[Expr::float(1.0).unwrap()]),Err(Error::WrongStage{..})));
    let e=emitter(CapabilitySet::CORE);
    assert!(!e.supports(Intrinsic::SubgroupShuffle));
    assert!(matches!(e.call_typed(Intrinsic::SubgroupShuffle,
        &[Expr::uint(1),Expr::uint(0)],Type::U32),Err(Error::FeatureDisabled(Capability::IntelSubgroups))));
    assert!(!e.supports(Intrinsic::TextureSample));
    assert!(!e.supports(Intrinsic::IntelSubGroupBlockRead8));
    assert!(!e.supports(Intrinsic::IntelSubGroupBlockReadUs));
    assert!(!e.supports(Intrinsic::Inverse));
}
#[test]
fn intel_subgroups_shuffle_and_block_io(){
    let caps=CapabilitySet::CORE.with(Capability::IntelSubgroups);
    let e=emitter(caps);
    let lane=Expr::uint(2);
    let data=Expr::variable("value",Type::VEC2).unwrap();
    let shuffled=ops::subgroup_shuffle(&e,&[data.clone(),lane.clone()],Type::VEC2).unwrap();
    assert_eq!(shuffled.code(),"intel_sub_group_shuffle(value, 2u)");
    let shifted=ops::subgroup_shuffle_up(&e,&[data.clone(),data,lane],Type::VEC2).unwrap();
    assert_eq!(shifted.code(),"intel_sub_group_shuffle_up(value, value, 2u)");
    assert!(matches!(ops::subgroup_shuffle(&e,
        &[Expr::float(2.0).unwrap(),Expr::int(0)],Type::F32),Err(Error::WrongTypes{..})));
    let ptr=Expr::from_source_unchecked("values + offset",Type::U32);
    let read=ops::intel_sub_group_block_read4(&e,&[ptr.clone()],Type::Vector(Scalar::U32,4)).unwrap();
    assert_eq!(read.code(),"intel_sub_group_block_read4(values + offset)");
    let vec=Expr::variable("payload",Type::Vector(Scalar::U32,4)).unwrap();
    let store=ops::intel_sub_group_block_write4(&e,&[ptr,vec],Type::Void).unwrap();
    assert_eq!(store.code(),"intel_sub_group_block_write4(values + offset, payload)");
    assert!(matches!(ops::intel_sub_group_block_write(&e,
        &[Expr::variable("p",Type::U32).unwrap(),Expr::float(1.0).unwrap()],Type::Void),Err(Error::WrongTypes{..})));
}
#[test]
fn intel_kernel_generates_cl_source(){
    let mut k=KernelBuilder::new("process",CapabilitySet::CORE).unwrap();
    let input=k.read_only_buffer("source",Type::F32).unwrap();
    let output=k.read_write_buffer("destination",Type::F32).unwrap();
    let count=k.value("count",Type::U32).unwrap();
    let idx=k.index_1d("gid").unwrap();
    k.return_if_out_of_bounds(&idx,&count).unwrap();
    let x=input.load(&idx).unwrap();
    let doubled=k.emitter().call(Intrinsic::Multiply,&[x,Expr::float(2.0).unwrap()]).unwrap();
    k.store(&output,&idx,&doubled).unwrap();
    let code=k.build().unwrap();
    assert!(code.contains("__kernel void process(const __global float* source, __global float* destination, uint count)"));
    assert!(code.contains("destination[gid] = (source[gid] * 2.0);"));
    assert!(code.contains("if (gid >= count) return;"));
    assert!(!code.contains("cl_intel_subgroups"));
}
#[test]
fn intel_kernel_rejects_misuse_and_generates_extension_pragma(){
    let caps=CapabilitySet::CORE.with(Capability::IntelSubgroups);
    let mut k=KernelBuilder::new("test",caps).unwrap();
    let buffer=k.read_only_buffer("a",Type::U32).unwrap();
    assert!(matches!(k.read_write_buffer("a",Type::U32),Err(Error::DuplicateParameter)));
    assert!(matches!(k.store(&buffer,&Expr::uint(0),&Expr::uint(2)),Err(Error::InvalidType)));
    let body=k.emitter().call_typed(Intrinsic::SubgroupShuffle,
        &[Expr::uint(7),Expr::uint(1)],Type::U32).unwrap();
    k.statement(&body);
    assert!(k.build().unwrap().contains("#pragma OPENCL EXTENSION cl_intel_subgroups : enable"));
    let short=CapabilitySet::CORE.with(Capability::IntelSubgroupsShort);
    let k=KernelBuilder::new("short",short).unwrap();
    assert!(matches!(k.build(),Err(Error::FeatureDisabled(Capability::IntelSubgroups))));
}
