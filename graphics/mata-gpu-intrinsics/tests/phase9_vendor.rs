use mata_gpu_intrinsics::vendor::{self, Target,Op,VendorError,CudaClusterConfig};
#[test]
fn cuda_architectural_requirements(){
    assert!(vendor::check(Op::CudaClusterSync,&Target::cuda(8,9)).is_err());
    assert!(vendor::check(Op::CudaClusterSync,&Target::cuda(9,0)).is_ok());
    assert!(vendor::check(Op::CudaWmmaF16,&Target::cuda(6,1)).is_err());
    assert!(vendor::check(Op::CudaShuffleXorU32,&Target::cuda(7,5)).is_ok());
}
#[test]
fn hip_low_precision_requires_cdna4(){
    let old=Target::hip("gfx942").unwrap();let new=Target::hip("gfx950").unwrap();
    assert!(vendor::check(Op::HipFp4E2M1,&old).is_err());
    assert!(vendor::check(Op::HipFp4E2M1,&new).is_ok());
    assert!(vendor::check(Op::HipFp6E3M2,&new).is_ok());
}
#[test]
fn only_checked_vendor_snippets_emitted(){
    let result=vendor::emit_device_helpers(&Target::cuda(8,6), &[Op::CudaLaneId,Op::CudaActiveMask,Op::CudaActiveMask]).unwrap();
    assert!(result.contains("asm volatile"));
    assert!(result.contains("__activemask"));
    assert_eq!(result.matches("__activemask").count(),1);
    assert!(matches!(vendor::emit_device_helpers(&Target::cuda(9,0),&[Op::CudaTmaBulkCopy]),Err(VendorError::MissingCompilerFeature(_))));
    assert!(vendor::emit_device_helpers(&Target::cuda(8,0),&[Op::CudaWmmaF16]).unwrap().contains("mma_sync"));
    let hip=vendor::emit_device_helpers(&Target::hip("gfx950").unwrap(),&[Op::HipFp6E2M3]).unwrap();
    assert!(hip.contains("<hip/hip_fp6.h>"));
    assert!(hip.contains("__hip_cvt_float_to_fp6"));
    assert!(vendor::emit_device_helpers(&Target::cuda(9,0),&[Op::HipWaveBarrier]).is_err());
}
#[test]
fn cluster_config_validates_dimensions(){
    let ok=CudaClusterConfig{grid_blocks:[8,1,1],block_threads:[256,1,1],cluster_blocks:[2,1,1]};
    assert!(ok.validate(&Target::cuda(9,0)).is_ok());
    assert!(ok.validate(&Target::cuda(8,0)).is_err());
    assert!(CudaClusterConfig{grid_blocks:[7,1,1],..ok}.validate(&Target::cuda(9,0)).is_err());
}

#[test]
fn fp4_fp6_roundtrips_and_packing(){
    use vendor::MiniFloat;
    for mode in [MiniFloat::E2M1,MiniFloat::E2M3,MiniFloat::E3M2]{
        for code in 0u8..(1u8<<mode.bits()){
            let value=mode.decode(code).unwrap();
            assert_eq!(mode.encode(value).unwrap(),code);
        }
        let input=(0u8..(1u8<<mode.bits())).collect::<Vec<_>>();
        let packed=vendor::pack_codes(mode,&input).unwrap();
        assert_eq!(vendor::unpack_codes(mode,&packed,input.len()).unwrap(),input);
    }
    assert_eq!(MiniFloat::E2M1.encode(6.4).unwrap(),7);
    assert!(MiniFloat::E2M3.encode(f32::NAN).is_err());
}
