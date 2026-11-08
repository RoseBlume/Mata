use mata_gpu_intrinsics::{Backend,ir::*};
fn ty()->DataType{DataType::Scalar(Number::U32)}
fn ix(v:u32)->Value{Value::UInt(v as u64,ty())}
fn data_ptr(space:AddressSpace)->DataType{DataType::Pointer(Box::new(ty()),space,Access::ReadWrite)}
fn data(name:&str,space:AddressSpace)->Value{variable(name,data_ptr(space)).unwrap()}
fn base(body:Vec<Statement>)->Program{
    let mut program=Program::default();
    program.add_kernel(Kernel{name:"compute".into(),args:vec![Parameter{name:"counter".into(),ty:data_ptr(AddressSpace::Global)}],workgroup:[8,1,1],body}).unwrap();
    program
}
fn profile(b:Backend)->TargetProfile{TargetProfile::offline(b)}
#[test]
fn opencl_explicit_atomic_add_and_fence_emit_source(){
    let op=atomic_statement(AtomicOp::Add,data("counter",AddressSpace::Global),ix(0),Some(ix(3)),None,
        Some("previous"),AtomicOrder::AcqRel,AtomicScope::Device,None).unwrap();
    let p=base(vec![op,Statement::Fence(MemoryFenceSpec{order:AtomicOrder::Release,scope:AtomicScope::Device,domain:FenceDomain::Global})]);
    let src=p.emit_for(&profile(Backend::IntelOpenCl)).unwrap();
    assert!(src.contains("atomic_fetch_add_explicit"));
    assert!(src.contains("memory_order_acq_rel"));
    assert!(src.contains("atomic_work_item_fence"));
    assert!(src.contains("atomic_uint"));
    assert!(src.contains("memory_scope_device"));
}
#[test]
fn failed_atomic_orders_do_not_silently_lower(){
    let stmt=atomic_statement(AtomicOp::Add,data("counter",AddressSpace::Global),ix(0),Some(ix(1)),None,
        None,AtomicOrder::AcqRel,AtomicScope::Device,None).unwrap();
    for b in [Backend::NvidiaCuda,Backend::AmdHip]{
        assert_eq!(base(vec![stmt.clone()]).validate_for(&profile(b)).unwrap_err().kind,DiagnosticKind::UnsupportedFeature);
    }
    let wrong=atomic_statement(AtomicOp::Load,data("counter",AddressSpace::Global),ix(0),None,None,
        Some("old"),AtomicOrder::Release,AtomicScope::Device,None).unwrap();
    assert_eq!(base(vec![wrong]).validate_for(&profile(Backend::IntelOpenCl)).unwrap_err().kind,DiagnosticKind::InvalidAtomicOrder);
}
#[test]
fn cuda_hip_legacy_fetch_add_is_restricted_to_relaxed_device_scope(){
    let stmt=atomic_statement(AtomicOp::Add,data("counter",AddressSpace::Global),ix(0),Some(ix(1)),None,
        Some("old"),AtomicOrder::Relaxed,AtomicScope::Device,None).unwrap();
    let program=base(vec![stmt]);
    for b in [Backend::NvidiaCuda,Backend::AmdHip] {
        let text=program.emit_for(&profile(b)).unwrap();
        assert!(text.contains("atomicAdd(&counter[0u], 1u)"));
    }
}
#[test]
fn opencl_cas_returns_old_value_and_uses_failure_order(){
    let stmt=atomic_statement(AtomicOp::CompareExchange,data("counter",AddressSpace::Global),ix(0),Some(ix(15)),Some(ix(3)),
        Some("observed"),AtomicOrder::AcqRel,AtomicScope::Device,Some(AtomicOrder::Acquire)).unwrap();
    let text=base(vec![stmt]).emit_for(&profile(Backend::IntelOpenCl)).unwrap();
    assert!(text.contains("atomic_compare_exchange_strong_explicit"));
    assert!(text.contains("memory_order_acquire"));
    assert!(text.contains("observed = _mata_expected_observed"));
}
#[test]
fn async_copy_requires_feature_uniformity_and_matching_wait(){
    let local=shared("tile",Number::U32).unwrap();
    let mut program=base(vec![
        Statement::Shared{name:"tile".into(),element:Number::U32,len:8},
        Statement::AsyncCopy{destination:local,source:data("counter",AddressSpace::Global),count:ix(8),event:"copy_done".into()},
        Statement::AwaitCopy("copy_done".into()),Statement::Barrier,
    ]);
    let mut target=profile(Backend::IntelOpenCl);
    assert_eq!(program.validate_for(&target).unwrap_err().required_feature,Some(Feature::AsynchronousCopy));
    target.enable(Feature::AsynchronousCopy);
    assert!(program.emit_for(&target).unwrap().contains("async_work_group_copy"));
    program.kernels[0].body.remove(2);
    assert!(program.validate_for(&target).is_err());
}
#[test]
fn fences_are_not_misrepresented_as_group_barriers(){
    let source=base(vec![Statement::Fence(MemoryFenceSpec{order:AtomicOrder::SeqCst,scope:AtomicScope::Workgroup,domain:FenceDomain::Local})])
        .emit_for(&profile(Backend::NvidiaCuda)).unwrap();
    assert!(source.contains("__threadfence_block()"));
    assert!(!source.contains("__syncthreads()"));
}
