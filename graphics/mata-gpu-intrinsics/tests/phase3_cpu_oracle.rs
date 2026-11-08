//! CPU reference outcomes only. These do NOT execute or validate emitted GPU code.
//! Use these as known inputs/outputs when a GPU runtime becomes available in Phase 4.
#[test]
fn saxpy_reference_including_fused_rounding(){
    let x=[1.0f32,-2.0,3.5,0.0];
    let y=[-1.0f32,2.5,0.0,4.0];
    let actual=x.iter().zip(y).map(|(&x,y)|x.mul_add(2.0,y)).collect::<Vec<_>>();
    assert_eq!(actual,vec![1.0,-1.5,7.0,4.0]);
}
#[test]
fn reduction_reference_handles_partial_last_group(){
    let input=(0..70).map(|x|x as f32).collect::<Vec<_>>();
    let groups=input.chunks(64).map(|part|part.iter().copied().sum::<f32>()).collect::<Vec<_>>();
    assert_eq!(groups,vec![2016.0,399.0]);
}
#[test]
fn stencil_reference_preserves_unwritten_boundaries(){
    let src=[2.0f32,4.0,8.0,16.0,32.0];
    let mut dst=[-123.0f32;5];
    for idx in 1..src.len()-1 {dst[idx]=0.25*src[idx-1]+0.5*src[idx]+0.25*src[idx+1]}
    assert_eq!(dst,[-123.0,4.5,9.0,18.0,-123.0]);
}
#[test]
fn histogram_reference_ignores_out_of_range_bins(){
    let data=[0usize,2,2,3,9,1,2,0];
    let mut hist=[0u32;4];
    for &bin in &data{if let Some(dest)=hist.get_mut(bin){*dest+=1}}
    assert_eq!(hist,[2,1,3,1]);
}
