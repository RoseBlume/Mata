use mata_gpu_intrinsics::{
    backend_pointers, Backend, Capability, CapabilitySet, Emitter, Error, Expr,
    FunctionTable, Intrinsic, Stage, Type, ALL_INTRINSICS, INTRINSIC_COUNT,
};

fn backends() -> [Backend;6] {
    [Backend::Wgsl,Backend::Glsl,Backend::Hlsl,Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip]
}

#[test]
fn every_backend_table_covers_the_complete_catalog() {
    for backend in backends() {
        assert_eq!(backend_pointers(backend).len(),INTRINSIC_COUNT);
        let table=FunctionTable::new(backend,Stage::Compute,CapabilitySet::ALL);
        assert_eq!(table.catalog_count(),INTRINSIC_COUNT);
        assert_eq!(table.backend(),backend);
        assert_eq!(table.stage(),Stage::Compute);
        assert_eq!(table.capabilities(),CapabilitySet::ALL);
        assert_eq!(table.entries().count(),INTRINSIC_COUNT);
        let available=ALL_INTRINSICS.iter().filter(|&&op|table.emitter().supports(op)).count();
        assert_eq!(table.available_count(),available);
        assert!(table.available_count()>0, "{backend:?}");
        for (index,entry) in table.entries().enumerate(){
            assert_eq!(entry.intrinsic,ALL_INTRINSICS[index]);
            assert_eq!(entry.info.op,entry.intrinsic);
            assert_eq!(entry.function.is_some(),table.emitter().supports(entry.intrinsic));
            assert_eq!(table.contains(entry.intrinsic),entry.function.is_some());
            assert_eq!(table.pointer(entry.intrinsic).is_some(),entry.function.is_some());
        }
    }
}

#[test]
fn operation_pointers_really_switch_shader_dialects() {
    for (backend,expected) in [
        (Backend::Wgsl,"sin(x)"),(Backend::Glsl,"sin(x)"),
        (Backend::Hlsl,"sin(x)"),(Backend::IntelOpenCl,"sin(x)"),
        (Backend::NvidiaCuda,"sinf(x)"),(Backend::AmdHip,"sinf(x)"),
    ] {
        let table=FunctionTable::new(backend,Stage::Compute,CapabilitySet::CORE);
        let f=table.get(Intrinsic::Sin).expect("sin is universally supported");
        assert_eq!(f.intrinsic(),Intrinsic::Sin);
        let x=Expr::variable("x",Type::F32).unwrap();
        assert_eq!(f.call(&[x.clone()]).unwrap().code(),expected);
        assert_eq!(table.call(Intrinsic::Sin,&[x.clone()]).unwrap().code(),expected);
        assert_eq!(table.call_named("sin",&[x.clone()]).unwrap().code(),expected);
        assert_eq!(table.get_named("sin").unwrap().call(&[x.clone()]).unwrap().code(),expected);
        assert_eq!(table.call_named_typed("sin",&[x],Type::F32).unwrap().code(),expected);
    }
}

#[test]
fn typed_dispatch_uses_identical_emitter_validation() {
    let t=FunctionTable::wgsl(Stage::Fragment,CapabilitySet::CORE.with(Capability::Textures));
    let texture=Expr::variable("tex",Type::Texture2D).unwrap();
    let sampler=Expr::variable("samp",Type::Sampler).unwrap();
    let coords=Expr::variable("uv",Type::VEC2).unwrap();
    let args=[texture,sampler,coords];
    let sample=t.get(Intrinsic::TextureSample).unwrap();
    assert!(matches!(sample.call(&args),Err(Error::ExplicitReturnTypeRequired(_))));
    let by_table=sample.call_typed(&args,Type::VEC4).unwrap();
    let by_emitter=t.emitter().call_typed(Intrinsic::TextureSample,&args,Type::VEC4).unwrap();
    assert_eq!(by_table,by_emitter);
    assert_eq!(t.call_typed(Intrinsic::TextureSample,&args,Type::VEC4).unwrap(),by_emitter);
}

#[test]
fn missing_entries_are_not_callable_but_errors_remain_descriptive() {
    let table=FunctionTable::wgsl(Stage::Vertex,CapabilitySet::CORE);
    assert!(table.get(Intrinsic::Dpdx).is_none());
    assert!(matches!(table.call(Intrinsic::Dpdx,&[Expr::float(1.0).unwrap()]),Err(Error::FeatureDisabled(Capability::Derivatives))));
    let enabled=FunctionTable::wgsl(Stage::Vertex,CapabilitySet::CORE.with(Capability::Derivatives));
    assert!(enabled.get(Intrinsic::Dpdx).is_none());
    assert!(matches!(enabled.call(Intrinsic::Dpdx,&[Expr::float(1.0).unwrap()]),Err(Error::WrongStage{..})));
    assert!(matches!(table.call_named("operation_that_does_not_exist",&[]),Err(Error::UnknownIntrinsicName)));
    let intel=FunctionTable::new(Backend::IntelOpenCl,Stage::Fragment,CapabilitySet::ALL);
    assert_eq!(intel.available_count(),0);
    assert!(matches!(intel.call(Intrinsic::Sin,&[Expr::float(0.0).unwrap()]),Err(Error::WrongStage{..})));
    assert!(table.get(Intrinsic::AmdReadFirstLane).is_none());
}

#[test]
fn wrong_backend_cannot_be_passed_into_raw_pointer() {
    let w=FunctionTable::wgsl(Stage::Compute,CapabilitySet::CORE);
    let pointer=w.pointer(Intrinsic::Sin).unwrap();
    let alien=Emitter::new(Backend::NvidiaCuda,Stage::Compute,CapabilitySet::CORE);
    assert!(matches!(pointer(&alien,&[Expr::float(0.1).unwrap()],None),
        Err(Error::BackendMismatch{expected:Backend::Wgsl,actual:Backend::NvidiaCuda})));
    let correct=pointer(w.emitter(),&[Expr::float(0.1).unwrap()],None).unwrap();
    assert_eq!(correct.code(),"sin(0.1)");
}

#[test]
fn vendor_specific_entries_respect_requested_capabilities() {
    let intel=FunctionTable::intel(CapabilitySet::CORE);
    let intel_wide=FunctionTable::intel(CapabilitySet::ALL);
    assert!(intel_wide.available_count()>=intel.available_count());
    let nvidia=FunctionTable::nvidia(CapabilitySet::CORE);
    let nvidia_wide=FunctionTable::nvidia(CapabilitySet::ALL);
    assert!(nvidia_wide.available_count()>=nvidia.available_count());
    assert!(!nvidia.contains(Intrinsic::NvidiaShuffleSync));
    assert!(nvidia_wide.contains(Intrinsic::NvidiaShuffleSync));
    let amd=FunctionTable::amd(CapabilitySet::CORE);
    let amd_wide=FunctionTable::amd(CapabilitySet::ALL);
    assert!(amd_wide.available_count()>=amd.available_count());
    assert!(!amd.contains(Intrinsic::AmdReadFirstLane));
    assert!(amd_wide.contains(Intrinsic::AmdReadFirstLane));
    for b in backends() {
        let t=FunctionTable::new(b,Stage::Compute,CapabilitySet::ALL);
        assert!(t.entries().filter(|entry|entry.function.is_some()).count()==t.available_count());
    }
}

#[cfg(feature="std")]
#[test]
fn all_tables_can_be_shared_across_threads() {
    use std::sync::Arc;
    let t=Arc::new(FunctionTable::nvidia(CapabilitySet::CORE));
    let jobs:Vec<_>=(0..4).map(|_|{
        let t=Arc::clone(&t);
        std::thread::spawn(move||{
            for _ in 0..100{
                assert_eq!(t.call(Intrinsic::Sin,&[Expr::float(0.0).unwrap()]).unwrap().code(),"sinf(0.0)");
            }
        })
    }).collect();
    for job in jobs{job.join().unwrap();}
}
