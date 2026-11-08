//! Typed, zero-third-party-dependency command argument and return adapters.
//! All conversions are explicit and report errors rather than panicking.
use crate::{Error, Result, Value};

/// Convert a dynamically typed MIVL argument to a Rust function parameter.
pub trait FromCommandArg: Sized {
    fn from_command_arg(value: &Value) -> Result<Self>;
}
impl FromCommandArg for Value {
    fn from_command_arg(v: &Value) -> Result<Self> {Ok(v.clone())}
}
impl FromCommandArg for String {
    fn from_command_arg(v: &Value) -> Result<Self> {
        match v { Value::String(s) => Ok(s.clone()), _ => Err(Error::command("expected a string argument")) }
    }
}
impl FromCommandArg for bool {
    fn from_command_arg(v: &Value) -> Result<Self> {
        match v {Value::Bool(b) => Ok(*b),_ => Err(Error::command("expected a bool argument"))}
    }
}
impl FromCommandArg for f64 {
    fn from_command_arg(v: &Value) -> Result<Self> {
        match v {Value::Number(n) if n.is_finite() => Ok(*n),_ => Err(Error::command("expected a finite number argument"))}
    }
}
impl FromCommandArg for f32 {
    fn from_command_arg(v: &Value) -> Result<Self> {
        let n=f64::from_command_arg(v)?;
        if n < f32::MIN as f64 || n > f32::MAX as f64 {return Err(Error::command("f32 argument out of range"));}
        Ok(n as f32)
    }
}
fn integer(v:&Value,min:f64,max:f64) -> Result<f64> {
    let n=f64::from_command_arg(v)?;
    if !n.is_finite() || n.fract()!=0.0 || n<min || n>max {
        Err(Error::command("integer argument out of range or not integral"))
    } else {Ok(n)}
}
macro_rules! integer_argument {
    ($t:ty,$min:expr,$max:expr) => {
        impl FromCommandArg for $t {
            fn from_command_arg(v: &Value) -> Result<Self> {
                Ok(integer(v,$min as f64,$max as f64)? as $t)
            }
        }
    }
}
integer_argument!(i32,i32::MIN,i32::MAX);
integer_argument!(u32,u32::MIN,u32::MAX);
// MIVL Number uses IEEE-754 f64. Preserve integers precisely, even on 64-bit.
integer_argument!(i64,-9_007_199_254_740_991_i64,9_007_199_254_740_991_i64);
integer_argument!(u64,0_u64,9_007_199_254_740_991_u64);
impl FromCommandArg for usize {
    fn from_command_arg(v: &Value) -> Result<Self> {
        Ok(integer(v,0.0,(usize::MAX as f64).min(9_007_199_254_740_991.0))? as usize)
    }
}
impl FromCommandArg for Vec<Value> {
    fn from_command_arg(v: &Value) -> Result<Self> {
        match v {Value::List(v)=>Ok(v.clone()),_=>Err(Error::command("expected an array argument"))}
    }
}

/// Convert a Rust function return value to a result for MIVL.
pub trait IntoCommandResult {
    fn into_command_result(self) -> Result<Value>;
}
impl IntoCommandResult for Value {
    fn into_command_result(self)->Result<Value> {Ok(self)}
}
impl IntoCommandResult for () {
    fn into_command_result(self)->Result<Value> {Ok(Value::Null)}
}
impl IntoCommandResult for String {
    fn into_command_result(self)->Result<Value> {Ok(Value::String(self))}
}
impl IntoCommandResult for &str {
    fn into_command_result(self)->Result<Value> {Ok(Value::String(self.into()))}
}
impl IntoCommandResult for bool {
    fn into_command_result(self)->Result<Value> {Ok(Value::Bool(self))}
}
impl IntoCommandResult for Vec<Value> {
    fn into_command_result(self)->Result<Value> {Ok(Value::List(self))}
}
impl IntoCommandResult for f64 {
    fn into_command_result(self)->Result<Value> {
        if self.is_finite() {Ok(Value::Number(self))} else {Err(Error::command("command returned a non-finite number"))}
    }
}
impl IntoCommandResult for f32 {
    fn into_command_result(self)->Result<Value> {(self as f64).into_command_result()}
}
macro_rules! integer_result {
    ($t:ty) => {
        impl IntoCommandResult for $t {
            fn into_command_result(self)->Result<Value> {
                let v=self as f64;
                if v as $t != self || !v.is_finite() || v.abs()>9_007_199_254_740_991.0 {
                    return Err(Error::command("integer command result cannot be represented exactly by MIVL Number"));
                }
                Ok(Value::Number(v))
            }
        }
    }
}
integer_result!(i32);
integer_result!(u32);
integer_result!(i64);
integer_result!(u64);
integer_result!(usize);
impl<T,E> IntoCommandResult for std::result::Result<T,E>
where T:IntoCommandResult,E:std::fmt::Display {
    fn into_command_result(self)->Result<Value> {
        self.map_err(|e|Error::command(e.to_string()))?.into_command_result()
    }
}
