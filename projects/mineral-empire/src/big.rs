//! Unbounded-range decimal scientific arithmetic, without external number crates.
//!
//! A Big number uses a 53-bit-precision signed mantissa and an arbitrary-sized
//! decimal exponent. Range is constrained only by available memory, not f64 or
//! a fixed-width exponent. This is NOT arbitrary-precision mantissa arithmetic:
//! adding very different magnitudes rounds away the smaller one.
use std::{cmp::Ordering, fmt, str::FromStr};

const RADIX:u64=1_000_000_000;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Exp { negative:bool, digits:Vec<u32> } // little-endian base 1e9
impl Exp {
    fn from_i64(x:i64)->Self {let mut v=x.unsigned_abs();let mut digits=vec![];while v!=0 {digits.push((v%RADIX) as u32);v/=RADIX;}Self{negative:x<0&&!digits.is_empty(),digits}}
    fn is_zero(&self)->bool {self.digits.is_empty()}
    fn normalize(&mut self){while self.digits.last()==Some(&0){self.digits.pop();}if self.is_zero(){self.negative=false;}}
    fn cmp_abs(&self,rhs:&Self)->Ordering {self.digits.len().cmp(&rhs.digits.len()).then_with(||self.digits.iter().rev().cmp(rhs.digits.iter().rev()))}
    fn cmp_signed(&self,rhs:&Self)->Ordering {if self.negative!=rhs.negative{return if self.negative{Ordering::Less}else{Ordering::Greater}}let cmp=self.cmp_abs(rhs);if self.negative{cmp.reverse()}else{cmp}}
    fn sum_abs(a:&Self,b:&Self)->Vec<u32>{let mut out=vec![];let mut carry=0u64;for i in 0..a.digits.len().max(b.digits.len()){let v=a.digits.get(i).copied().unwrap_or(0) as u64+b.digits.get(i).copied().unwrap_or(0) as u64+carry;out.push((v%RADIX) as u32);carry=v/RADIX;}if carry>0{out.push(carry as u32)}out}
    fn sub_abs(a:&Self,b:&Self)->Vec<u32>{// precondition abs(a)>=abs(b)
        let mut out=vec![];let mut borrow=0i64;for i in 0..a.digits.len(){let mut v=a.digits[i] as i64-b.digits.get(i).copied().unwrap_or(0) as i64-borrow;if v<0{v+=RADIX as i64;borrow=1}else{borrow=0}out.push(v as u32)}out
    }
    fn add(&self,rhs:&Self)->Self{let (negative,digits)=if self.negative==rhs.negative{(self.negative,Self::sum_abs(self,rhs))}else{match self.cmp_abs(rhs){Ordering::Greater=>(self.negative,Self::sub_abs(self,rhs)),Ordering::Less=>(rhs.negative,Self::sub_abs(rhs,self)),Ordering::Equal=>(false,vec![])}};let mut out=Self{negative,digits};out.normalize();out}
    fn sub(&self,rhs:&Self)->Self{let mut n=rhs.clone();if !n.is_zero(){n.negative=!n.negative};self.add(&n)}
    fn add_i64(&self,rhs:i64)->Self{self.add(&Self::from_i64(rhs))}
    fn mul_small(&self,n:u32)->Self{if n==0||self.is_zero(){return Self::default();}let mut carry=0u64;let mut digits=vec![];for &v in &self.digits{let w=v as u64*n as u64+carry;digits.push((w%RADIX) as u32);carry=w/RADIX;}while carry>0{digits.push((carry%RADIX) as u32);carry/=RADIX;}Self{negative:self.negative,digits}}
    fn div_small(&self,n:u32)->(Self,i64){assert!(n>0);let mut digits=vec![0;self.digits.len()];let mut rem=0u64;for i in (0..self.digits.len()).rev(){let a=rem*RADIX+self.digits[i] as u64;digits[i]=(a/n as u64) as u32;rem=a%n as u64;}let mut out=Self{negative:self.negative,digits};out.normalize();(out,if self.negative{-(rem as i64)}else{rem as i64})}
    fn small_diff(&self,rhs:&Self,limit:u64)->Option<i64>{let delta=self.sub(rhs);let mut result=0u64;for &v in delta.digits.iter().rev(){result=result.checked_mul(RADIX)?.checked_add(v as u64)?;if result>limit{return None}}Some(if delta.negative{-(result as i64)}else{result as i64})}
    fn parse(s:&str)->Result<Self,String>{let t=s.trim();if t.is_empty(){return Err("empty exponent".into())}let (negative,digits_str)=if let Some(x)=t.strip_prefix('-'){(true,x)}else{(false,t.strip_prefix('+').unwrap_or(t))};if digits_str.is_empty()||!digits_str.bytes().all(|b|b.is_ascii_digit()){return Err("invalid decimal exponent".into())}if digits_str.len()>16_384{return Err("exponent is too long for a save".into())}let mut digits=Vec::new();for chunk in digits_str.as_bytes().rchunks(9){let s=std::str::from_utf8(chunk).map_err(|e|e.to_string())?;digits.push(s.parse::<u32>().map_err(|e|e.to_string())?);}let mut out=Self{negative,digits};out.normalize();Ok(out)}
}
impl fmt::Display for Exp {fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{if self.is_zero(){return write!(f,"0")}if self.negative{write!(f,"-")?}let mut chunks=self.digits.iter().rev();write!(f,"{}",chunks.next().unwrap())?;for d in chunks{write!(f,"{d:09}")?;}Ok(())}}

#[derive(Clone,Debug)]
pub struct Big { mantissa:f64, exponent:Exp }
impl Default for Big {fn default()->Self{Self::zero()}}
impl Big {
    pub fn zero()->Self{Self{mantissa:0.,exponent:Exp::default()}}
    pub fn one()->Self{Self::from(1.)}
    pub fn is_zero(&self)->bool{self.mantissa==0.}
    pub fn is_positive(&self)->bool{self.mantissa>0.}
    pub fn from_parts(mantissa:f64,exponent:i64)->Self{Self::make(mantissa,Exp::from_i64(exponent))}
    fn make(mut mantissa:f64,mut exponent:Exp)->Self{
        if mantissa==0. || !mantissa.is_finite(){return Self::zero()}
        let shift=mantissa.abs().log10().floor() as i64;
        if shift!=0 {let applied=shift.clamp(-300,300);mantissa/=10f64.powi(applied as i32);exponent=exponent.add_i64(applied);}
        // Correction for inaccuracies near 1 / 10 boundary.
        while mantissa.abs()>=10. {mantissa/=10.;exponent=exponent.add_i64(1);}
        while mantissa.abs()<1. {mantissa*=10.;exponent=exponent.add_i64(-1);}
        Self{mantissa,exponent}
    }
    pub fn add(&self,other:&Self)->Self{
        if self.is_zero(){return other.clone()}if other.is_zero(){return self.clone()}
        let cmp=self.exponent.cmp_signed(&other.exponent);
        let (large,small)=if cmp==Ordering::Less{(other,self)}else{(self,other)};
        match large.exponent.small_diff(&small.exponent,22){Some(delta)=>Self::make(large.mantissa+small.mantissa*10f64.powi(-(delta as i32)),large.exponent.clone()),None=>large.clone()}
    }
    pub fn neg(&self)->Self{let mut n=self.clone();n.mantissa=-n.mantissa;n}
    pub fn sub(&self,other:&Self)->Self{self.add(&other.neg())}
    pub fn mul(&self,other:&Self)->Self{if self.is_zero()||other.is_zero(){return Self::zero()}Self::make(self.mantissa*other.mantissa,self.exponent.add(&other.exponent))}
    pub fn div(&self,other:&Self)->Self{assert!(!other.is_zero(),"division by zero");if self.is_zero(){return Self::zero()}Self::make(self.mantissa/other.mantissa,self.exponent.sub(&other.exponent))}
    pub fn mul_f64(&self,k:f64)->Self{if !k.is_finite()||k==0.{return Self::zero()}Self::make(self.mantissa*k,self.exponent.clone())}
    pub fn pow_u64(&self,mut power:u64)->Self {let mut base=self.clone();let mut acc=Self::one();while power>0 {if power&1!=0{acc=acc.mul(&base)}power>>=1;if power!=0{base=base.mul(&base)}}acc}
    /// Positive-base fractional power represented as a ratio; huge exponents
    /// are divided as arbitrary-size integers, never cast to f64.
    pub fn pow_ratio(&self,numerator:u32,denominator:u32)->Self{
        if self.is_zero(){return Self::zero()}assert!(self.is_positive() && denominator>0);
        let (whole,remain)=self.exponent.mul_small(numerator).div_small(denominator);
        let extra=10f64.powf((self.mantissa.log10()*numerator as f64+remain as f64)/denominator as f64);
        Self::make(extra,whole)
    }
    /// Approximate base-10 logarithm with the same arbitrary-range exponent
    /// support as Big. The result's mantissa still has f64 precision, but
    /// even an exponent with thousands of digits is represented safely.
    /// Useful for logarithmic softcaps, not exact logarithm arithmetic.
    pub fn log10_approx(&self)->Self {
        assert!(self.is_positive(), "log10 is undefined for non-positive Big values");
        let digits=self.exponent.to_string();
        let (negative,abs)=match digits.strip_prefix('-') {
            Some(part)=>(true,part), None=>(false,digits.as_str()),
        };
        let take=abs.len().min(15);
        // A small head prevents an overflow when the exponent's decimal
        // representation contains more digits than f64 can parse.
        let head=abs[..take].parse::<f64>().unwrap_or(0.);
        let magnitude=Self::from_parts(head,(abs.len()-take) as i64);
        let whole=if negative {magnitude.neg()} else {magnitude};
        whole.add(&Self::from(self.mantissa.log10()))
    }
    pub fn floor(&self)->Self {
        if self.is_zero(){return Self::zero()}
        if self.exponent.cmp_signed(&Exp::from_i64(15))==Ordering::Greater{return self.clone()}
        let Some(e)=self.exponent.small_diff(&Exp::default(),308) else{return Self::zero()};
        let v=self.mantissa*10f64.powi(e as i32);
        Self::from(v.floor())
    }
    pub fn clamp_nonnegative(self)->Self{if self.mantissa<0.{Self::zero()}else{self}}
    pub fn to_f64(&self)->f64{if self.is_zero(){return 0.}match self.exponent.small_diff(&Exp::default(),307){Some(e) if e>=-307=>self.mantissa*10f64.powi(e as i32),Some(e) if e< -307=>0.,_=>if self.exponent.negative{0.}else{self.mantissa.signum()*f64::INFINITY}}}
    pub fn exponent_string(&self)->String{self.exponent.to_string()}
    pub fn display(&self)->String {
        if self.is_zero(){return "0".into()}
        if let Some(exp)=self.exponent.small_diff(&Exp::default(),34){
            if exp < 3 && exp>=-3 {let f=self.to_f64();return if f.abs()>=10. {format!("{f:.0}")}else{format!("{f:.2}")}}
            if exp>=3&&exp<34 {
                const SUFFIX:[&str;12]=["","K","M","B","T","Qa","Qi","Sx","Sp","Oc","No","Dc"];
                let group=(exp/3) as usize;let offset=exp%3;
                if group<SUFFIX.len(){return format!("{:.2}{}",self.mantissa*10f64.powi(offset as i32),SUFFIX[group])}
            }
        }
        format!("{:.3}e{}",self.mantissa,self.exponent)
    }
    /// Full range-safe serialization (17 significant mantissa digits).
    pub fn storage(&self)->String{format!("{:.16}e{}",self.mantissa,self.exponent)}
}
impl From<f64> for Big {fn from(x:f64)->Self{if !x.is_finite()||x==0.{return Self::zero()}let e=(x.abs().log10().floor() as i64).clamp(-307,307);Self::make(x/10f64.powi(e as i32),Exp::from_i64(e))}}
impl From<u64> for Big {fn from(x:u64)->Self{Self::from(x as f64)}}
impl From<&Big> for Big {fn from(x:&Big)->Self{x.clone()}}
impl FromStr for Big {type Err=String;fn from_str(s:&str)->Result<Self,Self::Err>{let (m,e)=s.trim().split_once('e').or_else(||s.trim().split_once('E')).ok_or("Big numbers require a scientific exponent: 1.23e456")?;let mantissa=m.parse::<f64>().map_err(|x|x.to_string())?;if !mantissa.is_finite(){return Err("non-finite mantissa".into())}Ok(Self::make(mantissa,Exp::parse(e)?))}}
impl PartialEq for Big{fn eq(&self,other:&Self)->bool{self.partial_cmp(other)==Some(Ordering::Equal)}}
impl PartialOrd for Big {fn partial_cmp(&self,other:&Self)->Option<Ordering>{if self.is_zero()||other.is_zero(){return self.mantissa.partial_cmp(&other.mantissa)}if self.mantissa.signum()!=other.mantissa.signum(){return self.mantissa.partial_cmp(&other.mantissa)}let v=self.exponent.cmp_signed(&other.exponent).then_with(||self.mantissa.abs().partial_cmp(&other.mantissa.abs()).unwrap_or(Ordering::Equal));Some(if self.mantissa<0.{v.reverse()}else{v})}}
impl fmt::Display for Big {fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{write!(f,"{}",self.display())}}
#[cfg(test)]mod tests{
    use super::*;
    fn n(s:&str)->Big{s.parse().unwrap()}
    #[test]fn beyond_f64(){let a=n("4.25e900000000000000000000000000000000000");let b=a.mul(&n("2e900000000000000000000000000000000000"));assert!(b> a);let c=b.div(&a);assert!(c>n("1e800000000000000000000000000000000000"));assert_eq!(a.add(&a).div(&Big::from(2.)).storage(),a.storage());}
    #[test]fn arithmetic(){let a=n("3e500");let b=n("2e500");assert_eq!(a.add(&b).display(),"5.000e500");assert_eq!(a.sub(&b).display(),"1.000e500");let c=a.mul(&b);assert!(c>n("5e999"));assert_eq!(c.div(&b),a);}
    #[test]fn signed_and_tiny_numbers(){
        let x=n("-7e450");let y=n("2e450");
        assert_eq!(x.add(&y).display(),"-5.000e450");
        assert_eq!(x.sub(&y).display(),"-9.000e450");
        assert_eq!(x.div(&y).display(),"-3.50");
        let tiny=n("1e-500");
        assert_eq!(tiny.mul(&n("1e1000")).display(),"1.000e500");
    }
    #[test]fn logarithm_works_beyond_f64(){
        let ordinary=n("1e165").log10_approx();
        assert_eq!(ordinary,Big::from(165.));
        let huge=n("2e9999999999999999999999999999999999999");
        let answer=huge.log10_approx();
        assert!(answer>n("1e35"));
        assert!(answer<n("1e38"));
    }
    #[test]fn exponent_arithmetic(){let a=Exp::parse("999999999999999999999999999999999").unwrap();assert_eq!(a.add_i64(1).to_string(),"1000000000000000000000000000000000");assert_eq!(a.sub(&a).to_string(),"0");let (q,r)=a.div_small(10);assert_eq!(q.to_string(),"99999999999999999999999999999999");assert_eq!(r,9);}
}
