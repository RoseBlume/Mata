use mata_gpu_intrinsics::resources::*;
#[test] fn image_dimensions_and_pitch(){
    let d=ImageDesc::new_2d(13,7,ImageFormat::Rgba8Unorm,ImageAccess::ReadWrite).unwrap();
    assert_eq!(d.row_bytes().unwrap(),52);
    assert_eq!(d.slice_bytes().unwrap(),364);
    assert_eq!(d.byte_len().unwrap(),364);
    assert!(d.region([12,6,0],[1,1,1]).is_ok());
    assert!(d.region([12,6,0],[2,1,1]).is_err());
    assert!(d.region([0,0,0],[1,1,0]).is_err());
}
#[test] fn image_overflow_rejected(){
    assert!(ImageDesc::new_2d(usize::MAX,10,ImageFormat::Rgba32Float,ImageAccess::ReadOnly).is_err());
    assert!(SamplerDesc{normalized_coordinates:false,address:AddressMode::Repeat,filter:FilterMode::Nearest}.validate().is_err());
}
#[test] fn array_and_3d_images(){
    let a=ImageDesc{dimension:ImageDimension::D2Array,extent:[16,16,1],array_layers:3,format:ImageFormat::R8Unorm,access:ImageAccess::ReadOnly};
    assert_eq!(a.byte_len().unwrap(),16*16*3);
    assert!(a.region([0,0,2],[16,16,1]).is_ok());
    assert!(a.region([0,0,3],[16,16,1]).is_err());
}
