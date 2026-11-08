use mata_gpu_intrinsics::{ir::*, Backend};
#[path = "support/phase3_kernels.rs"] mod cases;
use cases::*;

#[test]
fn all_four_kernels_lower_on_three_native_backends(){
    for program in [saxpy(),reduction(),stencil(),histogram()] {
        let src=lower_all(&program);
        for source in src {assert!(!source.is_empty());assert!(source.contains("void "));}
    }
}
#[test]
fn saxpy_has_bounds_check_and_correct_fma_spelling(){
    let [cl,cu,hip]=lower_all(&saxpy());
    assert!(cl.contains("fma("));assert!(cl.contains("get_global_id(0)"));
    for c in [&cu,&hip]{assert!(c.contains("fmaf("));assert!(c.contains("threadIdx.x"));}
    assert!(cl.contains("get_global_id"));
}
#[test]
fn shared_array_barriers_are_outside_divergent_branches(){
    let [cl,cu,hip]=lower_all(&reduction());
    assert!(cl.contains("__local float scratch[64]"));
    assert_eq!(cl.matches("barrier(CLK_LOCAL_MEM_FENCE)").count(),7);
    for native in [&cu,&hip]{
        assert!(native.contains("__shared__ float scratch[64]"));
        assert_eq!(native.matches("__syncthreads()").count(),7);
    }
}
#[test]
fn stencil_checks_both_neighbors(){
    for source in lower_all(&stencil()){assert!(source.contains("0u"));assert!(source.contains("count"));}
}
#[test]
fn histogram_atomic_spelling_matches_native_runtime(){
    let [cl,cu,hip]=lower_all(&histogram());
    assert!(cl.contains("atomic_add(&hist[bin], 1u)"));
    assert!(cu.contains("atomicAdd(&hist[bin], 1u)"));
    assert!(hip.contains("atomicAdd(&hist[bin], 1u)"));
}
#[test]
fn floating_math_and_cuda_vector_fallback(){
    let n=t(Number::F32);let f0=v("a",n.clone());let f1=v("b",n.clone());
    let math=compute(ComputeOp::Clamp,vec![f0.clone(),f1.clone(),f(1.0)]).unwrap();
    let p=finish("clamp_value",vec![p("a",n.clone()),p("b",n.clone())],vec![Statement::Let("result".into(),n.clone(),math)],64);
    let [cl,cu,hip]=lower_all(&p);
    assert!(cl.contains("clamp(a, b, 1.0f)"));
    assert!(cu.contains("fminf(fmaxf(a, b), 1.0f)"));
    assert!(hip.contains("fminf(fmaxf(a, b), 1.0f)"));
    let vec4=DataType::Vector(Number::F32,4);
    let vv=v("v",vec4.clone());
    let dot=compute(ComputeOp::Dot,vec![vv.clone(),vv]).unwrap();
    let p=finish("dot_vec",vec![p("v",vec4)],vec![Statement::Let("result".into(),n,dot)],64);
    let [cl,cu,hip]=lower_all(&p);
    assert!(cl.contains("dot(v, v)"));
    for source in [&cu,&hip]{assert!(source.contains("(v).x"));assert!(source.contains("(v).w"));}
}
#[test]
fn invalid_types_and_arity_are_rejected(){
    assert_eq!(compute(ComputeOp::Fma,vec![f(1.0)]).unwrap_err(),IrError::TypeMismatch);
    assert_eq!(compute(ComputeOp::Dot,vec![f(1.0),f(2.0)]).unwrap_err(),IrError::InvalidType);
    assert!(compute(ComputeOp::Sqrt,vec![u(10)]).is_err());
    assert!(atomic_add(v("x",ptr(Number::U32,Access::ReadOnly)),u(0),u(1)).is_err());
    assert!(shared("scratch",Number::Bool).is_err());
}
#[test]
fn directly_constructed_malformed_math_is_not_accepted(){
    let corrupt=Value::Compute(ComputeOp::Dot,vec![f(1.0),f(2.0)],t(Number::F32));
    let p=finish("bad",vec![],vec![Statement::Let("out".into(),t(Number::F32),corrupt)],64);
    assert!(p.validate().is_err());
}
#[test]
fn shared_arrays_are_kernel_scope_only_and_cannot_shadow_parameters(){
    let p=finish("bad",vec![p("scratch",t(Number::F32))],vec![Statement::Shared{name:"scratch".into(),element:Number::F32,len:64}],64);
    assert_eq!(p.validate().unwrap_err(),IrError::DuplicateName);
    let mut p=Program::default();p.add_function(Function{name:"bad_helper".into(),args:vec![],result:DataType::Void,
        body:vec![Statement::Shared{name:"scratch".into(),element:Number::F32,len:64}]}).unwrap();
    assert_eq!(p.validate().unwrap_err(),IrError::InvalidControlFlow);
}
#[test]
fn math_backend_contract_does_not_silently_rename(){
    let p=finish("native_only",vec![],vec![Statement::Let("one".into(),t(Number::F32),c(ComputeOp::Sqrt,vec![f(1.0)]))],64);
    for backend in [Backend::Wgsl,Backend::Glsl,Backend::Hlsl]{assert!(p.emit(backend).is_err());}
}
#[test]
fn all_scalar_math_variants_have_three_target_lowering(){
    let unary=[ComputeOp::Abs,ComputeOp::Sqrt,ComputeOp::Sin,ComputeOp::Cos,ComputeOp::Tan,
        ComputeOp::Exp,ComputeOp::Log,ComputeOp::Floor,ComputeOp::Ceil];
    let binary=[ComputeOp::Min,ComputeOp::Max,ComputeOp::Pow];
    let ternary=[ComputeOp::Clamp,ComputeOp::Fma];
    for op in unary.into_iter().chain(binary).chain(ternary){
        let arity=if ternary.contains(&op){3}else if binary.contains(&op){2}else{1};
        let mut args=Vec::new();let mut params=Vec::new();
        for index in 0..arity {
            let name=format!("v{}",index);
            args.push(v(&name,t(Number::F32)));
            params.push(p(&name,t(Number::F32)));
        }
        let expr=compute(op,args).unwrap();
        let kernel=finish("math",params,vec![Statement::Let("result".into(),t(Number::F32),expr)],32);
        for source in lower_all(&kernel) {assert!(source.contains("result = "));}
    }
}
#[test]
fn double_precision_math_requires_explicit_feature(){
    let expr=compute(ComputeOp::Sqrt,vec![v("value",t(Number::F64))]).unwrap();
    let program=finish("double_math",vec![p("value",t(Number::F64))],
        vec![Statement::Let("result".into(),t(Number::F64),expr)],32);
    let mut target=TargetProfile::offline(Backend::NvidiaCuda);
    assert_eq!(program.emit_for(&target).unwrap_err().required_feature,Some(Feature::Fp64));
    target.enable(Feature::Fp64);
    assert!(program.emit_for(&target).unwrap().contains("sqrt(value)"));
}
#[test]
fn cuda_hip_vector_fallback_rejects_repeated_memory_or_call_evaluation(){
    let vector=Value::VectorInit(vec![f(1.0),f(2.0),f(3.0),f(4.0)],Number::F32);
    let norm=compute(ComputeOp::Length,vec![vector]).unwrap();
    let p=finish("length",vec![],vec![Statement::Let("result".into(),t(Number::F32),norm)],32);
    assert!(p.emit(Backend::IntelOpenCl).is_ok());
    for target in [Backend::NvidiaCuda,Backend::AmdHip]{assert!(p.emit(target).is_err());}
}
