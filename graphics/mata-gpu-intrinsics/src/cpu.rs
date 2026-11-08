//! Deterministic CPU reference operations for testing GPU shader expressions.
//!
//! These scalar implementations are intentionally independent of CPU SIMD and GPU
//! capabilities. They are not substitutes for a GPU shader compiler or runtime.
use core::ops::{Add, Sub, Mul, Div};

/// Four packed `f32` lanes, with explicitly portable scalar behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Float4(pub [f32; 4]);
impl Float4 {
    /// Construct four lanes from individual components.
    pub const fn new(x:f32,y:f32,z:f32,w:f32)->Self{Self([x,y,z,w])}
    /// Broadcast one scalar to each lane.
    pub const fn splat(x:f32)->Self{Self([x;4])}
    /// Transform each lane independently.
    pub fn map(self,mut f:impl FnMut(f32)->f32)->Self{Self(self.0.map(|x|f(x)))}
    /// Combine two vectors lane-by-lane.
    pub fn zip_map(self,b:Self,mut f:impl FnMut(f32,f32)->f32)->Self{
        Self(core::array::from_fn(|i|f(self.0[i],b.0[i])))
    }
    /// Sum the four pairwise products.
    pub fn dot(self,b:Self)->f32{self.0[0]*b.0[0]+self.0[1]*b.0[1]+self.0[2]*b.0[2]+self.0[3]*b.0[3]}
    /// Return the Euclidean norm of four components.
    pub fn length(self)->f32{self.dot(self).sqrt()}
    /// Divide by length, or return zero for a zero-length input.
    pub fn normalize_or_zero(self)->Self{let n=self.length();if n>0.0{self.map(|v|v/n)}else{Self::splat(0.0)}}
    /// Clamp every lane between corresponding minimum and maximum lanes.
    pub fn clamp(self,min:Self,max:Self)->Self{Self(core::array::from_fn(|i|self.0[i].clamp(min.0[i],max.0[i])))}
}
impl Add for Float4{type Output=Self;fn add(self,b:Self)->Self{self.zip_map(b,|a,b|a+b)}}
impl Sub for Float4{type Output=Self;fn sub(self,b:Self)->Self{self.zip_map(b,|a,b|a-b)}}
impl Mul for Float4{type Output=Self;fn mul(self,b:Self)->Self{self.zip_map(b,|a,b|a*b)}}
impl Div for Float4{type Output=Self;fn div(self,b:Self)->Self{self.zip_map(b,|a,b|a/b)}}

/// Return the absolute values of all lanes.
pub fn abs4(x:Float4)->Float4{x.map(f32::abs)}
/// Return the floor of every lane.
pub fn floor4(x:Float4)->Float4{x.map(f32::floor)}
/// Return the ceiling of every lane.
pub fn ceil4(x:Float4)->Float4{x.map(f32::ceil)}
/// Truncate every lane toward zero.
pub fn trunc4(x:Float4)->Float4{x.map(f32::trunc)}
/// Compute sine on each lane.
pub fn sin4(x:Float4)->Float4{x.map(f32::sin)}
/// Compute cosine on each lane.
pub fn cos4(x:Float4)->Float4{x.map(f32::cos)}
/// Compute tangent on each lane.
pub fn tan4(x:Float4)->Float4{x.map(f32::tan)}
/// Compute base-e exponential on each lane.
pub fn exp4(x:Float4)->Float4{x.map(f32::exp)}
/// Compute the natural logarithm on each lane.
pub fn log4(x:Float4)->Float4{x.map(f32::ln)}
/// Compute square root on each lane.
pub fn sqrt4(x:Float4)->Float4{x.map(f32::sqrt)}
/// Compute reciprocal square root on each lane.
pub fn rsqrt4(x:Float4)->Float4{x.map(|v|1.0/v.sqrt())}
/// Return the fractional part, `x - floor(x)`.
pub fn fract4(x:Float4)->Float4{x.map(|v|v-v.floor())}
/// Interpolate lanes using `a * (1-t) + b * t`.
pub fn mix4(a:Float4,b:Float4,t:Float4)->Float4{
    Float4(core::array::from_fn(|i|a.0[i]*(1.0-t.0[i])+b.0[i]*t.0[i]))
}
/// Clamp four values to the inclusive interval [0, 1].
pub fn saturate4(x:Float4)->Float4{x.clamp(Float4::splat(0.0),Float4::splat(1.0))}
/// Apply the smoothstep polynomial in each lane; equal edges are rejected.
pub fn smoothstep4(edge0:Float4,edge1:Float4,x:Float4)->Option<Float4>{
    if (0..4).any(|i|edge0.0[i]>=edge1.0[i]){return None}
    let t=saturate4((x-edge0)/(edge1-edge0));Some(t*t*(Float4::splat(3.0)-t*Float4::splat(2.0)))
}
/// Return the cross product of the XYZ components and zero in W.
pub fn cross3(a:Float4,b:Float4)->Float4{Float4::new(
    a.0[1]*b.0[2]-a.0[2]*b.0[1],
    a.0[2]*b.0[0]-a.0[0]*b.0[2],
    a.0[0]*b.0[1]-a.0[1]*b.0[0],0.0)}
/// Reflect incident vector `i` around a unit normal `n`.
pub fn reflect4(i:Float4,n:Float4)->Float4{i-n*Float4::splat(2.0*i.dot(n))}
/// Refract incident vector using refraction ratio `eta`; zero on total internal reflection.
pub fn refract4(i:Float4,n:Float4,eta:f32)->Float4{
    let d=n.dot(i);let k=1.0-eta*eta*(1.0-d*d);
    if k<0.0{Float4::splat(0.0)}else{i*Float4::splat(eta)-n*Float4::splat(eta*d+k.sqrt())}
}
/// Saturating conversion of an `f32` to an 8-bit unsigned normalized value.
pub fn pack_unorm8(x:f32)->u8{(x.clamp(0.0,1.0)*255.0).round() as u8}
/// Interpret an unsigned normalized byte in the range [0,1].
pub fn unpack_unorm8(x:u8)->f32{x as f32/255.0}
/// Pack four normalized floats into a 32-bit word, lowest byte first.
pub fn pack4x8_unorm(x:Float4)->u32{
    (pack_unorm8(x.0[0]) as u32)|((pack_unorm8(x.0[1]) as u32)<<8)
        |((pack_unorm8(x.0[2]) as u32)<<16)|((pack_unorm8(x.0[3]) as u32)<<24)
}
/// Unpack a 32-bit word as four unsigned-normalized float channels.
pub fn unpack4x8_unorm(x:u32)->Float4{Float4(core::array::from_fn(|i|unpack_unorm8((x>>(i*8)) as u8)))}
/// Reverse all 32 bits of an unsigned integer.
pub fn reverse_bits(x:u32)->u32{x.reverse_bits()}
/// Count one-bits in an unsigned integer.
pub fn count_one_bits(x:u32)->u32{x.count_ones()}
/// Count leading zero bits in an unsigned integer.
pub fn count_leading_zeros(x:u32)->u32{x.leading_zeros()}
/// Count trailing zero bits in an unsigned integer.
pub fn count_trailing_zeros(x:u32)->u32{x.trailing_zeros()}
/// Circularly rotate unsigned bits left; shifts are implicitly masked to 0..31.
pub fn rotate_left(x:u32,n:u32)->u32{x.rotate_left(n)}
/// Circularly rotate unsigned bits right; shifts are implicitly masked to 0..31.
pub fn rotate_right(x:u32,n:u32)->u32{x.rotate_right(n)}
/// Extract up to 32 consecutive bits from an unsigned word.
/// Out-of-range bits are returned as zeros.
pub fn extract_bits(x:u32,offset:u32,count:u32)->u32{
    if offset>=32{return 0}
    let len=count.min(32-offset);
    if len==0 {0} else if len==32 {x} else {(x>>offset)&((1u32<<len)-1)}
}
/// Insert low `count` bits of `insert` into `base` beginning at `offset`.
/// Bits outside the 32-bit word are ignored.
pub fn insert_bits(base:u32,insert:u32,offset:u32,count:u32)->u32{
    if offset>=32{return base}
    let len=count.min(32-offset);
    if len==0{return base}
    let mask=if len==32{u32::MAX}else{(1u32<<len)-1};
    let shift_mask=mask<<offset;
    (base&!shift_mask)|((insert&mask)<<offset)
}
/// Transpose an array of four four-component vectors.
pub fn transpose4(columns:[Float4;4])->[Float4;4]{
    core::array::from_fn(|row|Float4(core::array::from_fn(|col|columns[col].0[row])))
}
/// Matrix-vector product, interpreting the matrix as four column vectors.
pub fn mul_mat4_vec4(columns:[Float4;4],v:Float4)->Float4{
    let mut out=Float4::splat(0.0);
    for i in 0..4 {out=out+columns[i]*Float4::splat(v.0[i]);}
    out
}
