//! Bit-exact, no-dependency floating storage conversions and numerical test oracles.
//! These types describe storage; arithmetic should promote to f32 unless a target
//! explicitly supports the corresponding precision and rounding mode.
use core::fmt;

#[repr(transparent)]
#[derive(Clone,Copy,Default,PartialEq,Eq,Hash)]
pub struct F16Bits(pub u16);
#[repr(transparent)]
#[derive(Clone,Copy,Default,PartialEq,Eq,Hash)]
pub struct Bf16Bits(pub u16);
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Rounding { NearestEven, TowardZero }

impl fmt::Debug for F16Bits {fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{write!(f,"F16Bits({:#06x})",self.0)}}
impl fmt::Debug for Bf16Bits {fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{write!(f,"Bf16Bits({:#06x})",self.0)}}
impl F16Bits {
    pub const fn bits(self)->u16{self.0}
    pub fn from_f32(x:f32)->Self{Self::convert(x,Rounding::NearestEven)}
    pub fn convert(x:f32,round:Rounding)->Self{
        let bits=x.to_bits(); let sign=((bits>>16)&0x8000) as u16;
        let exponent=((bits>>23)&0xff) as i32;let significand=bits&0x007f_ffff;
        if exponent==255 {
            if significand==0{return Self(sign|0x7c00)}
            let payload=((significand>>13) as u16)|0x0200;
            return Self(sign|0x7c00|payload);
        }
        if exponent==0 && significand==0{return Self(sign)}
        let mut e=exponent-127+15;
        if e>=31{return Self(sign|if round==Rounding::TowardZero {0x7bff}else{0x7c00})}
        if e<=0 {
            if e< -10{return Self(sign)}
            let mantissa=if exponent==0 {significand}else{significand|0x800000};
            let shift=(14-e) as u32;
            let mut result=mantissa>>shift;
            if round==Rounding::NearestEven {
                let remainder=mantissa&((1u32<<shift)-1);
                let half=1u32<<(shift-1);
                if remainder>half || (remainder==half && result&1==1){result+=1}
            }
            return Self(sign|(result as u16));
        }
        let mut mantissa=significand>>13;
        if round==Rounding::NearestEven {
            let remainder=significand&0x1fff;
            if remainder>0x1000 || (remainder==0x1000 && mantissa&1!=0){mantissa+=1}
        }
        if mantissa==0x400 {mantissa=0;e+=1;}
        if e>=31{return Self(sign|if round==Rounding::TowardZero {0x7bff}else{0x7c00})}
        Self(sign|((e as u16)<<10)|(mantissa as u16))
    }
    pub fn to_f32(self)->f32 {
        let s=((self.0&0x8000) as u32)<<16;
        let e=(self.0>>10)&0x1f;
        let m=(self.0&0x03ff) as u32;
        let raw=if e==0 {
            if m==0{s}else{
                let leading=m.leading_zeros()-21;
                let norm=m<<leading;
                let exp=(127-14-(leading as i32)) as u32;
                s|(exp<<23)|((norm&0x3ff)<<13)
            }
        } else if e==31{s|0x7f80_0000|(m<<13)}
        else{s|(((e as u32)+112)<<23)|(m<<13)};
        f32::from_bits(raw)
    }
}
impl Bf16Bits {
    pub const fn bits(self)->u16{self.0}
    pub fn from_f32(x:f32)->Self{Self::convert(x,Rounding::NearestEven)}
    pub fn convert(x:f32,round:Rounding)->Self{
        let raw=x.to_bits();
        if x.is_nan(){return Self(((raw>>16) as u16)|0x0040)}
        let bias=if round==Rounding::NearestEven {0x7fff+((raw>>16)&1)}else{0};
        Self((raw.wrapping_add(bias)>>16) as u16)
    }
    pub fn to_f32(self)->f32{f32::from_bits((self.0 as u32)<<16)}
}
/// Compare finite floating results in units-in-last-place, with monotonic
/// mapping across negative and positive finite values.
pub fn ulp_distance_f32(a:f32,b:f32)->Option<u32>{
    if a.is_nan() || b.is_nan(){return None}
    if a==b{return Some(0)}
    fn ordered(x:f32)->u32{let n=x.to_bits();if n&0x8000_0000!=0{!n}else{n^0x8000_0000}}
    Some(ordered(a).abs_diff(ordered(b)))
}
pub fn ulp_distance_f64(a:f64,b:f64)->Option<u64>{
    if a.is_nan() || b.is_nan(){return None}
    if a==b{return Some(0)}
    fn ordered(x:f64)->u64{let n=x.to_bits();if n&0x8000_0000_0000_0000!=0{!n}else{n^0x8000_0000_0000_0000}}
    Some(ordered(a).abs_diff(ordered(b)))
}
/// Portable complex scalar storage and algebra; transcendental branch cuts
/// are deliberately kept outside this arithmetic-only oracle.
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Complex<T>{pub re:T,pub im:T}
#[cfg(feature="std")]
impl Complex<f32>{
    pub fn conj(self)->Self{Self{re:self.re,im:-self.im}}
    pub fn abs(self)->f32{self.re.hypot(self.im)}
    pub fn mul(self,b:Self)->Self{Self{re:self.re.mul_add(b.re,-self.im*b.im),im:self.re.mul_add(b.im,self.im*b.re)}}
}
#[cfg(feature="std")]
impl Complex<f64>{
    pub fn conj(self)->Self{Self{re:self.re,im:-self.im}}
    pub fn abs(self)->f64{self.re.hypot(self.im)}
    pub fn mul(self,b:Self)->Self{Self{re:self.re.mul_add(b.re,-self.im*b.im),im:self.re.mul_add(b.im,self.im*b.re)}}
}
