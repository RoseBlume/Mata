use mata_gpu_intrinsics::{Backend,Capability,CapabilitySet,Emitter,Error,Expr,FunctionBuilder,
    Intrinsic,Scalar,Stage,Type,ALL_INTRINSICS,INTRINSIC_COUNT};
fn emitter(backend:Backend,stage:Stage)->Emitter{Emitter::new(backend,stage,CapabilitySet::CORE)}
#[test]
fn catalog_is_complete_consistent_and_unique(){
    assert_eq!(ALL_INTRINSICS.len(), INTRINSIC_COUNT);
    assert!(INTRINSIC_COUNT>=250);
    let mut seen=std::collections::HashSet::new();
    for (id,op) in ALL_INTRINSICS.iter().enumerate(){
        let info=op.info();assert_eq!(info.op,*op);
        assert_eq!(*op as usize,id);
        assert!(seen.insert(info.name));
        assert_eq!(Intrinsic::from_name(info.name),Some(*op));
        assert!(info.arity.max>=info.arity.min);
    }
    assert_eq!(Intrinsic::from_name("not_an_intrinsic"),None);
}
#[test]
fn wgsl_and_glsl_and_hlsl_generate_math(){
    for backend in [Backend::Wgsl,Backend::Glsl,Backend::Hlsl]{
        let e=emitter(backend,Stage::Vertex);
        let x=Expr::variable("x",Type::F32).unwrap();
        assert_eq!(e.call(Intrinsic::Sin,&[x.clone()]).unwrap().code(),"sin(x)");
        assert_eq!(e.call(Intrinsic::Add,&[x.clone(),Expr::float(2.0).unwrap()]).unwrap().code(),"(x + 2.0)");
        assert_eq!(e.call(Intrinsic::Dot,&[Expr::variable("a",Type::VEC3).unwrap(),Expr::variable("b",Type::VEC3).unwrap()]).unwrap().ty(),Type::F32);
    }
}
#[test]
fn stage_capabilities_and_arity_are_enforced(){
    let x=Expr::variable("x",Type::F32).unwrap();
    let e=emitter(Backend::Wgsl,Stage::Vertex);
    assert!(matches!(e.call(Intrinsic::Dpdx,&[x.clone()]),Err(Error::FeatureDisabled(Capability::Derivatives))));
    let e=Emitter::new(Backend::Wgsl,Stage::Vertex,CapabilitySet::CORE.with(Capability::Derivatives));
    assert!(matches!(e.call(Intrinsic::Dpdx,&[x.clone()]),Err(Error::WrongStage{..})));
    let e=Emitter::new(Backend::Wgsl,Stage::Fragment,CapabilitySet::CORE.with(Capability::Derivatives));
    assert_eq!(e.call(Intrinsic::Dpdx,&[x.clone()]).unwrap().code(),"dpdx(x)");
    assert!(matches!(e.call(Intrinsic::Dot,&[x.clone()]),Err(Error::WrongArity{..})));
    assert!(matches!(e.call(Intrinsic::Dot,&[x.clone(),x]),Err(Error::WrongTypes{..})));
}
#[test]
fn gpu_resource_methods_need_correct_types_and_options(){
    let e=Emitter::new(Backend::Wgsl,Stage::Fragment,CapabilitySet::CORE.with(Capability::Textures));
    let t=Expr::variable("tex",Type::Texture2D).unwrap();
    let s=Expr::variable("sampling",Type::Sampler).unwrap();
    let coords=Expr::variable("uv",Type::VEC2).unwrap();
    let result=e.call_typed(Intrinsic::TextureSample,&[t.clone(),s,coords],Type::VEC4).unwrap();
    assert_eq!(result.code(),"textureSample(tex, sampling, uv)");
    assert_eq!(result.ty(),Type::VEC4);
    assert!(matches!(e.call(Intrinsic::TextureSample,&[t.clone()]),Err(Error::WrongArity{..})));
    assert!(matches!(e.call_typed(Intrinsic::TextureStore,&[t.clone(),Expr::int(0),Expr::int(0)],Type::Void),Err(Error::FeatureDisabled(Capability::StorageTextures))));
    let e=Emitter::new(Backend::Wgsl,Stage::Compute,CapabilitySet::ALL);
    assert!(matches!(e.call_typed(Intrinsic::TextureStore,&[t,Expr::int(0),Expr::int(0)],Type::Void),Err(Error::WrongTypes{..})));
}
#[test]
fn backend_specific_rewrites_and_unsupported_calls(){
    let x=Expr::variable("a",Type::VEC3).unwrap();let y=Expr::variable("b",Type::VEC3).unwrap();
    let glsl=emitter(Backend::Glsl,Stage::Fragment);
    assert_eq!(glsl.call(Intrinsic::Less,&[x,y]).unwrap().code(),"lessThan(a, b)");
    let f=Expr::variable("f",Type::F32).unwrap();
    assert_eq!(glsl.call(Intrinsic::Modulo,&[f.clone(),Expr::float(2.0).unwrap()]).unwrap().code(),"mod(f, 2.0)");
    let wgsl=emitter(Backend::Wgsl,Stage::Fragment);
    assert!(!wgsl.supports(Intrinsic::Inverse));
    assert!(matches!(wgsl.call(Intrinsic::Inverse,&[Expr::variable("m",Type::Matrix(4,4)).unwrap()]),Err(Error::Unsupported{..})));
    let m=Expr::variable("m",Type::Matrix(2,3)).unwrap();
    assert_eq!(wgsl.call(Intrinsic::Transpose,&[m]).unwrap().ty(),Type::Matrix(3,2));
}
#[test]
fn function_builder_and_discard(){
    let e=emitter(Backend::Wgsl,Stage::Fragment);
    let mut fun=FunctionBuilder::new(e,"shade",Type::F32).unwrap();
    let a=fun.parameter("a",Type::F32).unwrap();
    let sin=e.call(Intrinsic::Sin,&[a]).unwrap();
    let s=fun.let_value("v",&sin).unwrap();
    assert!(matches!(fun.let_value("v",&s),Err(Error::DuplicateParameter)));
    fun.return_expr(&s).unwrap();
    assert_eq!(fun.build().unwrap(),"fn shade(a: f32) -> f32 {\n    let v: f32 = sin(a);\n    return v;\n}\n");
    let mut frag=FunctionBuilder::new(e,"drop_fragment",Type::Void).unwrap();frag.discard().unwrap();
    assert!(frag.build().unwrap().contains("discard;"));
    let v=Expr::variable("a",Type::Atomic(Scalar::U32)).unwrap();assert_eq!(v.ty(),Type::Atomic(Scalar::U32));
}
#[cfg(feature="std")]
#[test]
fn cpu_references_and_edge_cases(){
    use mata_gpu_intrinsics::cpu::*;
    let a=Float4::new(1.0,0.0,0.0,0.0);
    let b=Float4::new(0.0,1.0,0.0,0.0);
    assert_eq!(cross3(a,b),Float4::new(0.0,0.0,1.0,0.0));
    assert_eq!(a.dot(b),0.0);
    assert_eq!(pack4x8_unorm(Float4::new(1.0,0.0,0.0,1.0)),0xff00_00ff);
    assert_eq!(extract_bits(u32::MAX,0,32),u32::MAX);
    assert_eq!(insert_bits(0,3,31,2),1u32<<31);
    assert_eq!(rotate_left(0x8000_0000,1),1);
    assert_eq!(mul_mat4_vec4([a,b,Float4::new(0.0,0.0,1.0,0.0),Float4::new(0.0,0.0,0.0,1.0)],Float4::new(2.0,3.0,4.0,1.0)),Float4::new(2.0,3.0,4.0,1.0));
}
#[test]
fn named_intrinsic_wrappers_and_barriers_work_without_gpu(){
    use mata_gpu_intrinsics::ops;
    let e=Emitter::new(Backend::Wgsl,Stage::Compute,CapabilitySet::CORE.with(Capability::Workgroup));
    let barrier=ops::workgroup_barrier(&e).unwrap();
    assert_eq!(barrier.ty(),Type::Void);
    assert_eq!(barrier.code(),"workgroupBarrier()");
    let x=Expr::float(0.5).unwrap();
    assert_eq!(ops::sin(&e,&[x]).unwrap().code(),"sin(0.5)");
    let atom=Expr::from_source_unchecked("&counter",Type::Atomic(Scalar::U32));
    let e=Emitter::new(Backend::Wgsl,Stage::Compute,CapabilitySet::CORE.with(Capability::Atomics));
    let result=ops::atomic_add(&e,&[atom,Expr::uint(1)],Type::U32).unwrap();
    assert_eq!(result.code(),"atomicAdd(&counter, 1u)");
}
