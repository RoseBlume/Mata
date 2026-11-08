use mata_gpu_intrinsics::{Backend,ir::*};

fn scalar(n:Number)->DataType{DataType::Scalar(n)}
fn var(name:&str,t:DataType)->Value{variable(name,t).unwrap()}
fn param(name:&str,ty:DataType)->Parameter{Parameter{name:name.into(),ty}}
fn prog(body:Vec<Statement>)->Program{
    let mut p=Program::default();
    p.add_kernel(Kernel{name:"compute".into(),args:vec![],workgroup:[8,4,1],body}).unwrap();p
}
fn profile(b:Backend)->TargetProfile{TargetProfile::offline(b)}

#[test]
fn empty_portable_kernel_is_valid_for_all_six_backends(){
    let p=prog(vec![Statement::Barrier]);
    for b in [Backend::Wgsl,Backend::Glsl,Backend::Hlsl,Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip]{
        let target=profile(b);
        p.validate_for(&target).unwrap();
        assert!(!p.emit_for(&target).unwrap().is_empty());
    }
}
#[test]
fn enforces_limit_and_axis_dimensions(){
    let p=prog(vec![]);
    let mut cfg=profile(Backend::IntelOpenCl);
    cfg.limits.max_workgroup_size=Some(16);
    assert_eq!(p.validate_for(&cfg).unwrap_err().kind,DiagnosticKind::InvalidWorkgroup);
    cfg.limits.max_workgroup_size=Some(64);
    cfg.limits.max_workgroup_dimensions=[Some(16),Some(2),Some(1)];
    assert_eq!(p.validate_for(&cfg).unwrap_err().kind,DiagnosticKind::InvalidWorkgroup);
    cfg.limits.max_workgroup_dimensions=[Some(16),Some(8),Some(1)];
    assert!(p.validate_for(&cfg).is_ok());
}
#[test]
fn strict_limits_reject_missing_device_queries(){
    let p=prog(vec![]);
    let mut cfg=profile(Backend::Glsl);cfg.strict_limits=true;
    assert_eq!(p.validate_for(&cfg).unwrap_err().kind,DiagnosticKind::UnknownDeviceLimit);
    cfg.limits.max_workgroup_size=Some(1024);
    cfg.limits.max_workgroup_dimensions=[Some(1024),Some(1024),Some(64)];
    assert!(p.validate_for(&cfg).is_ok());
}
#[test]
fn optional_float_features_must_be_asserted_by_caller(){
    let mut p=prog(vec![]);
    p.kernels[0].args.push(param("double_value",scalar(Number::F64)));
    let mut cfg=profile(Backend::NvidiaCuda);
    let error=p.validate_for(&cfg).unwrap_err();
    assert_eq!(error.required_feature,Some(Feature::Fp64));
    assert_eq!(error.kind,DiagnosticKind::UnsupportedFeature);
    cfg.enable(Feature::Fp64);
    assert!(p.emit_for(&cfg).unwrap().contains("double_value"));
    assert_eq!(Feature::Fp64.opencl_feature_macro(),Some("__opencl_c_fp64"));
}
#[test]
fn old_target_dialects_are_rejected(){
    let p=prog(vec![]);
    let mut cfg=profile(Backend::Glsl);cfg.language=LanguageVersion::new(3,3);
    assert_eq!(p.validate_for(&cfg).unwrap_err().kind,DiagnosticKind::InvalidProfile);
    cfg=profile(Backend::IntelOpenCl);cfg.language=LanguageVersion::new(1,0);
    assert_eq!(p.validate_for(&cfg).unwrap_err().kind,DiagnosticKind::InvalidProfile);
}
#[test]
fn divergent_if_barrier_is_rejected_with_actionable_path(){
    let condition=binary(Binary::Eq,global_id(0).unwrap(),Value::UInt(0,scalar(Number::U32))).unwrap();
    let p=prog(vec![Statement::If(condition,vec![Statement::Barrier],vec![])]);
    let d=p.emit_for(&profile(Backend::Wgsl)).unwrap_err();
    assert_eq!(d.kind,DiagnosticKind::DivergentBarrier);
    assert_eq!(d.kernel,Some("compute".into()));
    assert!(d.location.contains("if.true"));
}
#[test]
fn uniform_conditions_may_guard_barriers(){
    let condition=binary(Binary::Eq,Value::Builtin(Builtin::GroupId,0),Value::UInt(0,scalar(Number::U32))).unwrap();
    let p=prog(vec![Statement::If(condition,vec![Statement::Barrier],vec![])]);
    assert!(p.emit_for(&profile(Backend::NvidiaCuda)).is_ok());
}
#[test]
fn barriers_after_varying_early_return_are_rejected(){
    let condition=binary(Binary::Eq,global_id(0).unwrap(),Value::UInt(0,scalar(Number::U32))).unwrap();
    let p=prog(vec![Statement::If(condition,vec![Statement::Return(None)],vec![]),Statement::Barrier]);
    assert_eq!(p.validate_for(&profile(Backend::IntelOpenCl)).unwrap_err().kind,DiagnosticKind::DivergentBarrier);
}
#[test]
fn barrier_inside_unproven_while_is_rejected(){
    let p=prog(vec![Statement::While(Value::Bool(true),vec![Statement::Barrier])]);
    assert_eq!(p.validate_for(&profile(Backend::IntelOpenCl)).unwrap_err().kind,DiagnosticKind::DivergentBarrier);
}
#[test]
fn uniform_for_range_barrier_is_accepted(){
    let p=prog(vec![Statement::ForRange {index:"step".into(),start:Value::Int(0,scalar(Number::I32)),end:Value::Int(4,scalar(Number::I32)),body:vec![Statement::Barrier]}]);
    assert!(p.validate_for(&profile(Backend::IntelOpenCl)).is_ok());
}
#[test]
fn helper_barrier_called_under_divergence_is_rejected(){
    let mut p=prog(vec![]);
    p.add_function(Function{name:"sync_all".into(),args:vec![],result:DataType::Void,body:vec![Statement::Barrier]}).unwrap();
    let cond=binary(Binary::Eq,global_id(0).unwrap(),Value::UInt(0,scalar(Number::U32))).unwrap();
    p.kernels[0].body=vec![Statement::If(cond,vec![Statement::Expr(Value::Call("sync_all".into(),vec![],DataType::Void))],vec![])];
    let error=p.validate_for(&profile(Backend::NvidiaCuda)).unwrap_err();
    assert_eq!(error.kind,DiagnosticKind::DivergentBarrier);
    assert!(error.location.contains("call(sync_all)"));
}
#[test]
fn varying_memory_load_makes_condition_divergent(){
    let ptr=DataType::Pointer(Box::new(scalar(Number::U32)),AddressSpace::Global,Access::ReadOnly);
    let mut p=prog(vec![]);
    p.kernels[0].args=vec![param("values",ptr.clone())];
    let fetched=Value::Load(Box::new(var("values",ptr)),Box::new(global_id(0).unwrap()),scalar(Number::U32));
    let cond=binary(Binary::Eq,fetched,Value::UInt(0,scalar(Number::U32))).unwrap();
    p.kernels[0].body=vec![Statement::If(cond,vec![Statement::Barrier],vec![])];
    assert_eq!(p.validate_for(&profile(Backend::IntelOpenCl)).unwrap_err().kind,DiagnosticKind::DivergentBarrier);
}
#[test]
fn resource_binding_limit_is_validated(){
    let mut p=prog(vec![]);
    let ty=DataType::Pointer(Box::new(scalar(Number::F32)),AddressSpace::Global,Access::ReadOnly);
    p.kernels[0].args=vec![param("a",ty.clone()),param("b",ty)];
    let mut cfg=profile(Backend::Wgsl);
    cfg.limits.max_storage_bindings=Some(1);
    assert_eq!(p.validate_for(&cfg).unwrap_err().kind,DiagnosticKind::InvalidLayout);
    cfg.limits.max_storage_bindings=Some(2);
    assert!(p.validate_for(&cfg).is_ok());
}
#[test]
fn record_layout_errors_are_reported_before_codegen(){
    let mut p=prog(vec![]);
    p.add_record(Record{name:"Flag".into(),fields:vec![Field{name:"yes".into(),ty:scalar(Number::Bool)}]}).unwrap();
    let d=p.emit_for(&profile(Backend::Wgsl)).unwrap_err();
    assert_eq!(d.kind,DiagnosticKind::InvalidLayout);
    assert!(d.location.contains("record.Flag.yes"));
}
#[test]
fn constant_pointer_cannot_be_written(){
    let mut p=prog(vec![]);
    p.kernels[0].args=vec![param("constants",DataType::Pointer(Box::new(scalar(Number::F32)),AddressSpace::Constant,Access::WriteOnly))];
    assert_eq!(p.validate_for(&profile(Backend::IntelOpenCl)).unwrap_err().kind,DiagnosticKind::InvalidAccess);
}
#[test]
fn atomic_order_and_scope_rules_are_checked(){
    let cfg=profile(Backend::IntelOpenCl);
    let spec=AtomicSpec{kind:AtomicKind::Load,order:AtomicOrder::Release,failure:None,scope:AtomicScope::Device,scalar:Number::U32,address_space:AddressSpace::Global,alignment:4};
    assert_eq!(spec.validate(&cfg).unwrap_err().kind,DiagnosticKind::InvalidAtomicOrder);
    let mut good=AtomicSpec{kind:AtomicKind::CompareExchange,order:AtomicOrder::AcqRel,failure:Some(AtomicOrder::Acquire),..spec};
    assert!(good.validate(&cfg).is_ok());
    good.failure=Some(AtomicOrder::SeqCst);
    assert_eq!(good.validate(&cfg).unwrap_err().kind,DiagnosticKind::InvalidAtomicOrder);
    good.failure=Some(AtomicOrder::Relaxed);good.scope=AtomicScope::System;
    assert_eq!(good.validate(&cfg).unwrap_err().required_feature,Some(Feature::SystemScopeAtomics));
    good.scope=AtomicScope::Workgroup;good.address_space=AddressSpace::Workgroup;
    assert!(good.validate(&cfg).is_ok());
    good.alignment=2;
    assert_eq!(good.validate(&cfg).unwrap_err().kind,DiagnosticKind::InvalidAtomicType);
}
#[test]
fn atomic_64bit_is_explicitly_feature_gated(){
    let mut cfg=profile(Backend::NvidiaCuda);
    let spec=AtomicSpec{kind:AtomicKind::ReadModifyWrite,order:AtomicOrder::Relaxed,failure:None,scope:AtomicScope::Device,scalar:Number::U64,address_space:AddressSpace::Global,alignment:8};
    assert_eq!(spec.validate(&cfg).unwrap_err().required_feature,Some(Feature::Atomic64));
    cfg.enable(Feature::Atomic64);
    assert!(spec.validate(&cfg).is_ok());
}
#[test]
fn int64_requires_explicit_device_support_in_strict_path(){
    let mut p=prog(vec![]);
    p.kernels[0].body.push(Statement::Let("count".into(),scalar(Number::I64),Value::Int(10,scalar(Number::I64))));
    let mut cfg=profile(Backend::AmdHip);
    assert_eq!(p.validate_for(&cfg).unwrap_err().required_feature,Some(Feature::Int64));
    cfg.enable(Feature::Int64);
    assert!(p.emit_for(&cfg).is_ok());
}

#[test]
fn fences_reject_incorrect_scope_without_hardware_feature(){
    let mut cfg=profile(Backend::IntelOpenCl);
    let fence=MemoryFenceSpec{order:AtomicOrder::Release,scope:AtomicScope::System,domain:FenceDomain::Global};
    assert_eq!(fence.validate(&cfg).unwrap_err().required_feature,Some(Feature::SystemScopeAtomics));
    cfg.enable(Feature::SystemScopeAtomics);
    assert!(fence.validate(&cfg).is_ok());
    let invalid=MemoryFenceSpec{domain:FenceDomain::Local,..fence};
    assert_eq!(invalid.validate(&cfg).unwrap_err().kind,DiagnosticKind::InvalidAtomicScope);
}

#[test]
fn cuda_constant_address_space_rejected_before_codegen(){
    let mut p=prog(vec![]);
    p.kernels[0].args=vec![param("constant_ptr",DataType::Pointer(Box::new(scalar(Number::F32)),AddressSpace::Constant,Access::ReadOnly))];
    assert_eq!(p.validate_for(&profile(Backend::NvidiaCuda)).unwrap_err().kind,DiagnosticKind::InvalidAddressSpace);
}
#[test]
fn missing_workgroup_feature_cannot_be_silently_assumed(){
    let mut p=prog(vec![]);
    p.kernels[0].args.push(param("v",DataType::Pointer(Box::new(scalar(Number::I32)),AddressSpace::Generic,Access::ReadOnly)));
    assert_eq!(p.validate_for(&profile(Backend::IntelOpenCl)).unwrap_err().required_feature,Some(Feature::GenericAddressSpace));
}
