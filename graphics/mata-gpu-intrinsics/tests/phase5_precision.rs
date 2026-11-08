use mata_gpu_intrinsics::{precision::{F16Bits,Bf16Bits,Rounding,ulp_distance_f32,ulp_distance_f64,Complex},ir::{compute,ComputeOp,DataType,Number,variable,IrError},Backend};
#[test] fn fp16_exact_reference_patterns(){
    for &(input,bits) in &[(0.0,0x0000),(1.0,0x3c00),(-2.0,0xc000),(65504.0,0x7bff)] {
        assert_eq!(F16Bits::from_f32(input).bits(),bits);
        assert_eq!(F16Bits(bits).to_f32(),input);
    }
    assert_eq!(F16Bits(1).to_f32().to_bits(),(2.0f32).powi(-24).to_bits());
    assert_eq!(F16Bits(0x8000).to_f32().to_bits(),(-0.0f32).to_bits());
    assert_eq!(F16Bits::from_f32(f32::INFINITY).bits(),0x7c00);
    assert_eq!(F16Bits::convert(1e9,Rounding::TowardZero).bits(),0x7bff);
    assert!(F16Bits::from_f32(f32::NAN).to_f32().is_nan());
    assert_eq!(F16Bits::convert(f32::from_bits(0x3f801000),Rounding::NearestEven).bits(),0x3c00);
    assert_eq!(F16Bits::convert(f32::from_bits(0x3f801000),Rounding::TowardZero).bits(),0x3c00);
    assert_eq!(F16Bits::from_f32(f32::from_bits(0x3f803000)).bits(),0x3c02);
}
#[test] fn bfloat16_ties_nan_inf_and_subnormals(){
    assert_eq!(Bf16Bits::from_f32(1.0).bits(),0x3f80);
    assert_eq!(Bf16Bits::from_f32(-0.0).bits(),0x8000);
    assert_eq!(Bf16Bits::from_f32(f32::INFINITY).bits(),0x7f80);
    assert_eq!(Bf16Bits::from_f32(f32::from_bits(0x3f808000)).bits(),0x3f80);
    assert!(Bf16Bits::from_f32(f32::NAN).to_f32().is_nan());
}
#[test] fn ulp_oracles(){
    assert_eq!(ulp_distance_f32(1.0,1.0),Some(0));
    assert_eq!(ulp_distance_f64(1.0,1.0),Some(0));
    assert_eq!(ulp_distance_f32(-0.0,0.0),Some(0));
    assert_eq!(ulp_distance_f32(f32::NAN,1.0),None);
    assert_eq!(ulp_distance_f32(1.0,f32::from_bits(1.0f32.to_bits()+1)),Some(1));
}
#[test] fn complex_oracles(){
    let z=Complex{re:3.0f32,im:4.0f32};
    assert_eq!(z.abs(),5.0);
    assert_eq!(z.conj().im,-4.0);
    assert!((z.mul(z).re+7.0).abs()<1e-5);
}
#[test] fn math_emitter_presence_and_negative_case(){
    let x=variable("x",DataType::Scalar(Number::F32)).unwrap();
    let y=variable("y",DataType::Scalar(Number::F32)).unwrap();
    let a=compute(ComputeOp::Log1p,vec![x.clone()]).unwrap();
    assert_eq!(a.ty(),DataType::Scalar(Number::F32));
    assert!(compute(ComputeOp::Hypot,vec![x.clone(),y.clone()]).is_ok());
    assert!(matches!(compute(ComputeOp::Atan2,vec![x.clone()]),Err(IrError::TypeMismatch)));
    let _=Backend::IntelOpenCl;
}
