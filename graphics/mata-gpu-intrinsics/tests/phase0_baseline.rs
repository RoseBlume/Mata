//! GPU-free Phase 0 regression and compile-only API checks.
use mata_gpu_intrinsics::{Backend, CapabilitySet, Emitter, Expr, Intrinsic, Stage, Type, ALL_INTRINSICS, INTRINSIC_COUNT};

const BASELINE_NAMES: &str = include_str!("../audit/catalog_names.txt");

#[test]
fn frozen_catalog_order_and_identity() {
    let frozen: Vec<&str> = BASELINE_NAMES.lines().collect();
    assert_eq!(frozen.len(),322,"Phase 0 catalog must contain 322 entries");
    assert_eq!(INTRINSIC_COUNT,322,"Update the baseline intentionally if adding intrinsics");
    for (i,(op,name)) in ALL_INTRINSICS.iter().zip(frozen.iter()).enumerate() {
        assert_eq!(*op as usize,i,"stable enum index changed");
        assert_eq!(format!("{:?}",op),*name,"catalog reordered or renamed at index {i}");
        assert_eq!(op.info().op,*op);
        assert_eq!(Intrinsic::from_name(op.info().name),Some(*op));
    }
}

#[test]
fn no_gpu_emitter_compile_smoke_all_targets() {
    // These verify public API usability and simple source lowering, not native compilation.
    let x=Expr::variable("x",Type::F32).unwrap();
    for backend in [Backend::Wgsl,Backend::Glsl,Backend::Hlsl,
                    Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip] {
        let emit=Emitter::new(backend,Stage::Compute,CapabilitySet::ALL);
        assert!(emit.supports(Intrinsic::Sin),"missing baseline mapping: {:?}",backend);
        let s=emit.call(Intrinsic::Sin,&[x.clone()]).expect("baseline sine source");
        assert_eq!(s.ty(),Type::F32);
        assert!(!s.code().is_empty());
    }
}
