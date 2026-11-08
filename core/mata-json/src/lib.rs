//! A std-only JSON parser/serializer and manual typed conversion API.
//! Compatible with StudyForge's existing JSON format; no Serde required.
use std::{collections::BTreeMap,fmt};
#[derive(Debug, Clone, PartialEq)]
pub enum Value { Null, Bool(bool), Integer(i64), Unsigned(u64), Float(f64), String(String), Array(Vec<Value>), Object(BTreeMap<String,Value>) }
impl Value {
 pub fn get(&self,key:&str)->Option<&Value> {self.as_object()?.get(key)}
 pub fn as_str(&self)->Option<&str>{if let Self::String(v)=self{Some(v)}else{None}}
 pub fn as_bool(&self)->Option<bool>{if let Self::Bool(v)=self{Some(*v)}else{None}}
 pub fn as_u64(&self)->Option<u64>{match self{Self::Unsigned(v)=>Some(*v),Self::Integer(v)=>u64::try_from(*v).ok(),_=>None}}
 pub fn as_i64(&self)->Option<i64>{match self{Self::Integer(v)=>Some(*v),Self::Unsigned(v)=>i64::try_from(*v).ok(),_=>None}}
 pub fn as_array(&self)->Option<&Vec<Value>>{if let Self::Array(v)=self{Some(v)}else{None}}
 pub fn as_object(&self)->Option<&BTreeMap<String,Value>>{if let Self::Object(v)=self{Some(v)}else{None}}
}
#[derive(Debug,Clone)]pub struct Error(pub String);
impl fmt::Display for Error {fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{write!(f,"{}",self.0)}}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T,Error>;
pub trait ToJson {fn to_json(&self)->Value;}
pub trait FromJson: Sized {fn from_json(v:&Value)->Result<Self>;}
pub fn from_str<T:FromJson>(src:&str)->Result<T>{T::from_json(&parse(src)?)}
pub fn from_value<T:FromJson>(v:Value)->Result<T>{T::from_json(&v)}
pub fn to_string<T:ToJson+?Sized>(v:&T)->Result<String>{stringify(&v.to_json(),false)}
pub fn to_vec_pretty<T:ToJson+?Sized>(v:&T)->Result<Vec<u8>>{Ok(stringify(&v.to_json(),true)?.into_bytes())}
pub fn parse(src:&str)->Result<Value>{let mut p=Parser{bytes:src.as_bytes(),pos:0,depth:0};let value=p.value()?;p.ws();if p.pos!=p.bytes.len(){return p.fail("trailing JSON content")}Ok(value)}
impl ToJson for Value{fn to_json(&self)->Value{self.clone()}}
impl FromJson for Value{fn from_json(v:&Value)->Result<Self>{Ok(v.clone())}}
impl ToJson for String{fn to_json(&self)->Value{Value::String(self.clone())}}
impl ToJson for str{fn to_json(&self)->Value{Value::String(self.into())}}
impl FromJson for String{fn from_json(v:&Value)->Result<Self>{v.as_str().map(str::to_owned).ok_or_else(||Error("expected string".into()))}}
impl ToJson for bool{fn to_json(&self)->Value{Value::Bool(*self)}}
impl FromJson for bool{fn from_json(v:&Value)->Result<Self>{v.as_bool().ok_or_else(||Error("expected bool".into()))}}
macro_rules! ints {($($ty:ty),*)=>{$(impl ToJson for $ty {fn to_json(&self)->Value{Value::Unsigned(*self as u64)}}
impl FromJson for $ty {fn from_json(v:&Value)->Result<Self>{v.as_u64().and_then(|n|<$ty>::try_from(n).ok()).ok_or_else(||Error("integer out of range".into()))}})*};}
ints!(u8,u16,u32,u64,usize);
impl<T:ToJson> ToJson for Vec<T>{fn to_json(&self)->Value{Value::Array(self.iter().map(ToJson::to_json).collect())}}
impl<T:FromJson> FromJson for Vec<T>{fn from_json(v:&Value)->Result<Self>{v.as_array().ok_or_else(||Error("expected array".into()))?.iter().map(T::from_json).collect()}}
impl<T:ToJson> ToJson for Option<T>{fn to_json(&self)->Value{self.as_ref().map(ToJson::to_json).unwrap_or(Value::Null)}}
impl<T:FromJson> FromJson for Option<T>{fn from_json(v:&Value)->Result<Self>{if matches!(v,Value::Null){Ok(None)}else{T::from_json(v).map(Some)}}}

pub fn object(items:impl IntoIterator<Item=(&'static str,Value)>)->Value{
 Value::Object(items.into_iter().map(|(k,v)|(k.to_owned(),v)).collect())
}
pub fn required<'a>(v:&'a Value,key:&str)->Result<&'a Value>{v.get(key).ok_or_else(||Error(format!("missing required field '{key}'")))}
pub fn field<T:FromJson>(v:&Value,key:&str)->Result<T>{T::from_json(required(v,key)?).map_err(|e|Error(format!("{key}: {}",e.0)))}
pub fn default_field<T:FromJson+Default>(v:&Value,key:&str)->Result<T>{match v.get(key){Some(v)=>T::from_json(v),None=>Ok(T::default())}}

pub fn stringify(v:&Value,pretty:bool)->Result<String>{let mut out=String::new();render(v,&mut out,0,pretty)?;Ok(out)}
fn render(v:&Value,out:&mut String,depth:usize,pretty:bool)->Result<()> {
 match v {
  Value::Null=>out.push_str("null"),Value::Bool(x)=>out.push_str(if *x{"true"}else{"false"}),
  Value::Integer(x)=>out.push_str(&x.to_string()),Value::Unsigned(x)=>out.push_str(&x.to_string()),
  Value::Float(x)=>{if !x.is_finite(){return Err(Error("non-finite JSON number".into()))}let n=x.to_string();out.push_str(&n);if !n.contains('.')&&!n.contains('e')&&!n.contains('E'){out.push_str(".0");}},
  Value::String(x)=>escape(x,out),
  Value::Array(xs)=>{out.push('[');for(i,x)in xs.iter().enumerate(){if i>0{out.push(',')};if pretty{out.push('\n');indent(out,depth+1)}render(x,out,depth+1,pretty)?;}if pretty&&!xs.is_empty(){out.push('\n');indent(out,depth)}out.push(']');},
  Value::Object(xs)=>{out.push('{');for(i,(key,x))in xs.iter().enumerate(){if i>0{out.push(',')};if pretty{out.push('\n');indent(out,depth+1)}escape(key,out);out.push(':');if pretty{out.push(' ')}render(x,out,depth+1,pretty)?;}if pretty&&!xs.is_empty(){out.push('\n');indent(out,depth)}out.push('}');}
 };Ok(())
}
fn indent(out:&mut String,n:usize){for _ in 0..n.min(64){out.push_str("  ")}}
fn escape(src:&str,out:&mut String){out.push('"');for ch in src.chars(){match ch{'"'=>out.push_str("\\\""),'\\'=>out.push_str("\\\\"),'\n'=>out.push_str("\\n"),'\r'=>out.push_str("\\r"),'\t'=>out.push_str("\\t"),c if c<'\u{20}'=>out.push_str(&format!("\\u{:04x}",c as u32)),c=>out.push(c)}}out.push('"')}
struct Parser<'a>{bytes:&'a [u8],pos:usize,depth:usize}
impl Parser<'_>{
 fn fail<T>(&self,msg:&str)->Result<T>{Err(Error(format!("{msg} at byte {}",self.pos)))}
 fn ws(&mut self){while matches!(self.bytes.get(self.pos),Some(b' '|b'\t'|b'\n'|b'\r')){self.pos+=1}}
 fn one(&mut self,b:u8)->Result<()>{if self.bytes.get(self.pos)==Some(&b){self.pos+=1;Ok(())}else{self.fail("unexpected character")}}
 fn value(&mut self)->Result<Value>{self.ws();if self.depth>=128{return self.fail("JSON nesting too deep")}
 let b=match self.bytes.get(self.pos){Some(b)=>*b,None=>return self.fail("unexpected end of input")};
 match b {b'{' => self.object(),b'['=>self.array(),b'"'=>self.string().map(Value::String),
  b't' if self.token(b"true")=>Ok(Value::Bool(true)),b'f' if self.token(b"false")=>Ok(Value::Bool(false)),
  b'n' if self.token(b"null")=>Ok(Value::Null),b'-'|b'0'..=b'9'=>self.number(),_=>self.fail("invalid JSON token")}
 }
 fn token(&mut self,token:&[u8])->bool{if self.bytes[self.pos..].starts_with(token){self.pos+=token.len();true}else{false}}
 fn object(&mut self)->Result<Value>{self.one(b'{')?;self.depth+=1;let mut map=BTreeMap::new();self.ws();if self.bytes.get(self.pos)==Some(&b'}'){self.pos+=1;self.depth-=1;return Ok(Value::Object(map))}
 loop{self.ws();let key=self.string()?;self.ws();self.one(b':')?;let val=self.value()?;map.insert(key,val);self.ws();match self.bytes.get(self.pos){Some(b',')=>self.pos+=1,Some(b'}')=>{self.pos+=1;break},_=>return self.fail("expected ',' or '}'")}}
 self.depth-=1;Ok(Value::Object(map))}
 fn array(&mut self)->Result<Value>{self.one(b'[')?;self.depth+=1;let mut xs=Vec::new();self.ws();if self.bytes.get(self.pos)==Some(&b']'){self.pos+=1;self.depth-=1;return Ok(Value::Array(xs))}
 loop{xs.push(self.value()?);self.ws();match self.bytes.get(self.pos){Some(b',')=>self.pos+=1,Some(b']')=>{self.pos+=1;break},_=>return self.fail("expected ',' or ']'")}}
 self.depth-=1;Ok(Value::Array(xs))}
 fn string(&mut self)->Result<String>{self.one(b'"')?;let mut out=String::new();let mut start=self.pos;
 loop{let ch=match self.bytes.get(self.pos){Some(ch)=>*ch,None=>return self.fail("unterminated string")};match ch{
 b'"'=>{out.push_str(std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_|Error("invalid UTF-8".into()))?);self.pos+=1;return Ok(out)},
 b'\\'=>{out.push_str(std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_|Error("invalid UTF-8".into()))?);self.pos+=1;
 let esc=*self.bytes.get(self.pos).ok_or_else(||Error("unterminated escape".into()))?;self.pos+=1;
 match esc{b'"'=>out.push('"'),b'\\'=>out.push('\\'),b'/'=>out.push('/'),b'b'=>out.push('\u{8}'),b'f'=>out.push('\u{c}'),b'n'=>out.push('\n'),b'r'=>out.push('\r'),b't'=>out.push('\t'),b'u'=>{
 let first=self.hex()?;let cp=if (0xD800..=0xDBFF).contains(&first){self.one(b'\\')?;self.one(b'u')?;let second=self.hex()?;if !(0xDC00..=0xDFFF).contains(&second){return self.fail("invalid surrogate pair")}0x10000+((first as u32-0xD800)<<10)+(second as u32-0xDC00)}else{if (0xDC00..=0xDFFF).contains(&first){return self.fail("isolated low surrogate")}first as u32};
 out.push(char::from_u32(cp).ok_or_else(||Error("invalid unicode escape".into()))?);},_=>return self.fail("invalid escape")};start=self.pos;
 },0..=31=>return self.fail("control character in string"),_=>self.pos+=1}}
 }
 fn hex(&mut self)->Result<u16>{let mut n=0u16;for _ in 0..4{let b=*self.bytes.get(self.pos).ok_or_else(||Error("truncated unicode escape".into()))?;self.pos+=1;n=(n<<4)|(b as char).to_digit(16).ok_or_else(||Error("invalid hex escape".into()))? as u16;}Ok(n)}
 fn number(&mut self)->Result<Value>{let start=self.pos;if self.bytes.get(self.pos)==Some(&b'-'){self.pos+=1;}
 match self.bytes.get(self.pos){Some(b'0')=>self.pos+=1,Some(b'1'..=b'9')=>{while matches!(self.bytes.get(self.pos),Some(b'0'..=b'9')){self.pos+=1}},_=>return self.fail("invalid number")}
 let mut fractional=false;
 if self.bytes.get(self.pos)==Some(&b'.'){fractional=true;self.pos+=1;let begin=self.pos;while matches!(self.bytes.get(self.pos),Some(b'0'..=b'9')){self.pos+=1}if begin==self.pos{return self.fail("invalid fraction")}}
 if matches!(self.bytes.get(self.pos),Some(b'e'|b'E')){fractional=true;self.pos+=1;if matches!(self.bytes.get(self.pos),Some(b'+'|b'-')){self.pos+=1};let begin=self.pos;while matches!(self.bytes.get(self.pos),Some(b'0'..=b'9')){self.pos+=1}if begin==self.pos{return self.fail("invalid exponent")}}
 let n=std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_|Error("invalid number encoding".into()))?;
 if fractional{let v=n.parse::<f64>().map_err(|_|Error("invalid floating number".into()))?;if !v.is_finite(){return self.fail("number exceeds representable range")}Ok(Value::Float(v))}
 else if let Ok(v)=n.parse::<i64>(){Ok(Value::Integer(v))}else if let Ok(v)=n.parse::<u64>(){Ok(Value::Unsigned(v))}else{self.fail("integer out of range")}
 }
}
#[cfg(test)]mod tests {use super::*;
 #[test]fn roundtrip_unicode(){let src=r#"{"q":"💡\\uD83D\\uDE80","items":[true,2,null,-5.4]}"#;let v=parse(src).unwrap();assert_eq!(parse(&stringify(&v,true).unwrap()).unwrap(),v);}
 #[test]fn rejects_trailing(){assert!(parse("{} []").is_err());assert!(parse("[1,]").is_err());assert!(parse("{\"a\":1,}").is_err());}
 #[test]fn integer_precision(){let v=parse("9007199254740993").unwrap();assert_eq!(stringify(&v,false).unwrap(),"9007199254740993");}
 #[test]fn unicode_escape_pair(){let v=parse(r#""\uD83D\uDE80""#).unwrap();assert_eq!(v.as_str(),Some("🚀"));}
 #[test]fn invalid_surrogate_rejected(){assert!(parse(r#""\uD800""#).is_err());}
 #[test]fn float_stays_float(){let v=parse("1.0").unwrap();assert_eq!(parse(&stringify(&v,false).unwrap()).unwrap(),v);}

}
