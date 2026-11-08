//! Phase 1: typed, hardware-independent compute-kernel AST and source generation.
//!
//! This module is additive: the original catalog, `Type`, `Expr`, and backends
//! retain their existing public APIs. All methods generate source only.
use alloc::{boxed::Box, format, string::{String, ToString}, vec::Vec};
use crate::{Backend, is_identifier};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Number { Bool, I8, U8, I16, U16, I32, U32, I64, U64, F16, F32, F64, Bf16 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AddressSpace { Global, Workgroup, Constant, Private, Generic }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Access { ReadOnly, ReadWrite, WriteOnly }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataType {
    Void, Scalar(Number), Vector(Number, u8), Array(Box<DataType>, usize),
    Pointer(Box<DataType>, AddressSpace, Access), Record(String), Tuple(Vec<DataType>),
}
impl DataType {
    pub fn validate(&self) -> Result<(), IrError> { match self {
        Self::Void|Self::Scalar(_) => Ok(()),
        Self::Vector(n, w) if [2,3,4,8,16].contains(w) && *n != Number::Bool => Ok(()),
        Self::Vector(_, _) => Err(IrError::InvalidType),
        Self::Array(ty, n) if *n>0 && *n<=1_000_000 => ty.validate(),
        Self::Pointer(ty, _, _) => { ty.validate()?; if **ty==Self::Void {Err(IrError::InvalidType)} else {Ok(())} },
        Self::Record(name) if is_identifier(name) => Ok(()),
        Self::Tuple(items) if (2..=8).contains(&items.len()) => { for item in items { item.validate()?; if *item==Self::Void || matches!(item, Self::Pointer(_,_,_)) { return Err(IrError::InvalidType); } } Ok(()) },
        _=>Err(IrError::InvalidType),
    }}
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field { pub name:String, pub ty:DataType }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record { pub name:String, pub fields:Vec<Field> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameter { pub name:String, pub ty:DataType }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin { GlobalId, LocalId, GroupId, GlobalSize, LocalSize, NumGroups }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binary { Add, Sub, Mul, Div, Rem, Lt, Le, Gt, Ge, Eq, Ne, And, Or }
/// Portable numerical operations with audited CUDA, HIP, and OpenCL lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComputeOp {
    Abs, Sqrt, Sin, Cos, Tan, Exp, Log, Floor, Ceil,
    Min, Max, Clamp, Fma, Pow, Dot, Length,
    Trunc, Round, Rint, Exp2, Exp10, Expm1, Log2, Log10, Log1p,
    Cbrt, Erf, Erfc, Tgamma, Lgamma, Asin, Acos, Atan,
    Sinh, Cosh, Tanh, Asinh, Acosh, Atanh, Fabs,
    Hypot, Atan2, Fmod, Copysign, Fdim, NextAfter,
    Rsqrt, Reciprocal, Fract, SinPi, CosPi, TanPi,
    AddSat, SubSat, AbsDiff, Rotate, Popcount, Clz, Ctz, MulHi, SinCos,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Var(String,DataType), Bool(bool), Int(i64,DataType), UInt(u64,DataType), FloatBits(u32), F64Bits(u64),
    Binary(Binary,Box<Value>,Box<Value>,DataType),
    Load(Box<Value>,Box<Value>,DataType), Builtin(Builtin,u8),
    Field(Box<Value>,String,DataType), Call(String,Vec<Value>,DataType),
    Cast(Box<Value>,DataType), Tuple(Vec<Value>), TupleAt(Box<Value>,usize,DataType),
    RecordInit(String,Vec<(String,Value)>), ArrayInit(Vec<Value>,DataType),
    VectorInit(Vec<Value>,Number), Index(Box<Value>,Box<Value>,DataType),
    Compute(ComputeOp,Vec<Value>,DataType),
}
impl Value { pub fn ty(&self)->DataType { match self {
    Self::Var(_,t)|Self::Int(_,t)|Self::UInt(_,t)|Self::Binary(_,_,_,t)|Self::Load(_,_,t)|Self::Field(_,_,t)|Self::Call(_,_,t)|Self::Cast(_,t)|Self::TupleAt(_,_,t)|Self::ArrayInit(_,t)|Self::Index(_,_,t)=>t.clone(),
    Self::Compute(_,_,t)=>t.clone(),
    Self::RecordInit(name,_)=>DataType::Record(name.clone()),
    Self::VectorInit(items,n)=>DataType::Vector(*n,items.len() as u8),
    Self::Tuple(items)=>DataType::Tuple(items.iter().map(Value::ty).collect()),
    Self::Bool(_)=>DataType::Scalar(Number::Bool), Self::FloatBits(_)=>DataType::Scalar(Number::F32), Self::F64Bits(_)=>DataType::Scalar(Number::F64),
    Self::Builtin(_,_)=>DataType::Scalar(Number::U32),
}}}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Statement {
    Let(String,DataType,Value), Store(Value,Value,Value), Assign(String,Value),
    If(Value,Vec<Statement>,Vec<Statement>), While(Value,Vec<Statement>),
    ForRange { index:String, start:Value, end:Value, body:Vec<Statement> },
    Return(Option<Value>), Expr(Value), Barrier, SetIndex(Value,Value,Value),
    /// Static workgroup-local array, visible to all work-items in the kernel.
    Shared { name:String, element:Number, len:usize },
    /// Native 32-bit integer atomic add (backend-defined ordering); old value discarded.
    AtomicAdd { pointer:Value, index:Value, delta:Value },
    /// Phase 7: typed atomic with explicit order and scope.
    Atomic(AtomicStatement),
    /// Phase 7: work-item memory fence (not an execution barrier).
    Fence(MemoryFenceSpec),
    /// OpenCL cooperative copy, requiring uniform participation.
    AsyncCopy { destination:Value, source:Value, count:Value, event:String },
    /// Wait for a previously declared async workgroup copy event.
    AwaitCopy(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function { pub name:String, pub args:Vec<Parameter>, pub result:DataType, pub body:Vec<Statement> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kernel { pub name:String, pub args:Vec<Parameter>, pub body:Vec<Statement>, pub workgroup:[u32;3] }
#[derive(Clone, Debug, Default)]
pub struct Program { pub records:Vec<Record>, pub functions:Vec<Function>, pub kernels:Vec<Kernel> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrError { InvalidIdentifier, InvalidType, TypeMismatch, DuplicateName, InvalidDimension, InvalidWorkgroup, InvalidControlFlow, UnknownSymbol, UnknownRecord, UnknownFunction, UnknownField, InvalidLayout, Unsupported {backend:Backend, detail:&'static str} }

pub fn variable(name:&str, ty:DataType)->Result<Value,IrError> {
    if !is_identifier(name) { return Err(IrError::InvalidIdentifier); }
    ty.validate()?; if ty==DataType::Void {return Err(IrError::InvalidType)};
    Ok(Value::Var(name.into(),ty))
}
pub fn global_id(axis:u8)->Result<Value,IrError>{if axis>2 {Err(IrError::InvalidDimension)}else {Ok(Value::Builtin(Builtin::GlobalId,axis))}}
pub fn binary(op:Binary,a:Value,b:Value)->Result<Value,IrError>{
    let t=a.ty(); if t!=b.ty(){return Err(IrError::TypeMismatch)};
    if matches!(op,Binary::And|Binary::Or){if t!=DataType::Scalar(Number::Bool){return Err(IrError::InvalidType)}}
    else if !matches!(t,DataType::Scalar(n) if n!=Number::Bool) && !matches!(t,DataType::Vector(_, _)){return Err(IrError::InvalidType)}
    let result=if matches!(op,Binary::Lt|Binary::Le|Binary::Gt|Binary::Ge|Binary::Eq|Binary::Ne|Binary::And|Binary::Or){DataType::Scalar(Number::Bool)}else {t};
    Ok(Value::Binary(op,Box::new(a),Box::new(b),result))
}
/// Builds a typed portable math operation. The same signature is rechecked
/// by whole-program validation, so directly constructed ASTs cannot bypass it.
pub fn compute(op:ComputeOp,args:Vec<Value>)->Result<Value,IrError>{
    let result=compute_result(op,&args.iter().map(Value::ty).collect::<Vec<_>>())?;
    Ok(Value::Compute(op,args,result))
}
/// Returns a typed variable referring to a `Statement::Shared` declaration.
pub fn shared(name:&str,element:Number)->Result<Value,IrError>{
    if !is_identifier(name) || is_reserved(name) {return Err(IrError::InvalidIdentifier)}
    if element==Number::Bool || element==Number::Bf16 {return Err(IrError::InvalidType)}
    Ok(Value::Var(name.into(),DataType::Pointer(
        Box::new(DataType::Scalar(element)),AddressSpace::Workgroup,Access::ReadWrite)))
}
/// Constructs a checked 32-bit integer atomic addition statement.
pub fn atomic_add(pointer:Value,index:Value,delta:Value)->Result<Statement,IrError>{
    let vt=delta.ty();
    if !matches!(vt,DataType::Scalar(Number::I32|Number::U32)) ||
       !matches!(pointer.ty(),DataType::Pointer(inner,AddressSpace::Global|AddressSpace::Workgroup,Access::ReadWrite) if *inner==vt) ||
       !matches!(index.ty(),DataType::Scalar(Number::U32|Number::I32)) {return Err(IrError::InvalidType)}
    Ok(Statement::AtomicAdd{pointer,index,delta})
}
pub fn load(pointer:Value,index:Value)->Result<Value,IrError>{
    let ty=match pointer.ty(){DataType::Pointer(item,_,_)=>*item, _=>return Err(IrError::InvalidType)};
    if !matches!(index.ty(),DataType::Scalar(Number::I32|Number::U32|Number::I64|Number::U64)){return Err(IrError::InvalidType)};
    Ok(Value::Load(Box::new(pointer),Box::new(index),ty))
}
impl Program {
    pub fn add_record(&mut self, record:Record)->Result<(),IrError>{
        if !is_identifier(&record.name){return Err(IrError::InvalidIdentifier)}
        if self.records.iter().any(|r|r.name==record.name) || self.functions.iter().any(|f|f.name==record.name) || self.kernels.iter().any(|k|k.name==record.name){return Err(IrError::DuplicateName)}
        for f in &record.fields{if !is_identifier(&f.name){return Err(IrError::InvalidIdentifier)} f.ty.validate()?;
            if f.ty==DataType::Void{return Err(IrError::InvalidType)} }
        for (i,f) in record.fields.iter().enumerate(){if record.fields[..i].iter().any(|prev|prev.name==f.name){return Err(IrError::DuplicateName)}}
        if record.fields.is_empty(){return Err(IrError::InvalidType)}
        self.records.push(record);Ok(())
    }
    pub fn add_kernel(&mut self,k:Kernel)->Result<(),IrError>{
        if !is_identifier(&k.name){return Err(IrError::InvalidIdentifier)}
        if self.kernels.iter().any(|f|f.name==k.name)||self.functions.iter().any(|f|f.name==k.name)||self.records.iter().any(|r|r.name==k.name){return Err(IrError::DuplicateName)}
        if k.workgroup.contains(&0) || k.workgroup.iter().any(|&n|n>1024) || k.workgroup.iter().fold(1u64,|a,&v|a*v as u64)>1024{return Err(IrError::InvalidWorkgroup)}
        validate_parameters(&k.args)?; self.kernels.push(k);Ok(())
    }
    pub fn add_function(&mut self,f:Function)->Result<(),IrError>{
        if !is_identifier(&f.name){return Err(IrError::InvalidIdentifier)}
        if self.functions.iter().any(|k|k.name==f.name)||self.kernels.iter().any(|k|k.name==f.name)||self.records.iter().any(|r|r.name==f.name){return Err(IrError::DuplicateName)}
        validate_parameters(&f.args)?; f.result.validate()?;self.functions.push(f);Ok(())
    }
    pub fn emit(&self,backend:Backend)->Result<String,IrError>{
        self.validate()?;
        if matches!(backend,Backend::Wgsl|Backend::Glsl|Backend::Hlsl) && self.kernels.len()>1 {
            return Err(IrError::Unsupported{backend,detail:"multiple shader entrypoints: use emit_kernel() for one entrypoint"});
        }
        // Validate all source-generating nodes before emitting any source text.
        for r in &self.records {for f in &r.fields{if backend==Backend::Wgsl {render_ty(&f.ty,backend)?;} else {render_declaration(&f.name,&f.ty,backend)?;}}}
        for f in &self.functions {
            if matches!(backend,Backend::Wgsl|Backend::Hlsl) && block_uses_any_builtin(&f.body) {
                return Err(IrError::Unsupported{backend,detail:"shader helper must receive invocation data as explicit arguments"});
            }
            if f.result!=DataType::Void {render_ty(&f.result,backend)?;}else if backend!=Backend::Wgsl{render_ty(&f.result,backend)?;}for a in &f.args{render_param(a,backend)?;} validate_block(&f.body,backend)?;}
        for k in &self.kernels {
            if backend==Backend::IntelOpenCl && k.args.iter().any(|arg|matches!(&arg.ty,
                DataType::Pointer(_,AddressSpace::Private,_) |
                DataType::Pointer(_,AddressSpace::Constant,Access::ReadWrite|Access::WriteOnly))) {
                return Err(IrError::Unsupported{backend,detail:"OpenCL kernel argument has invalid address space or access"});
            }
            if !matches!(backend,Backend::Wgsl|Backend::Glsl|Backend::Hlsl){for a in &k.args{render_param(a,backend)?;}}
            validate_block(&k.body,backend)?;
        }
        let mut s=String::new();
        if backend==Backend::Glsl {s.push_str("#version 450\n");}
        if backend==Backend::IntelOpenCl {s.push_str("// OpenCL C compute source\n");}
        if matches!(backend,Backend::NvidiaCuda|Backend::AmdHip) {s.push_str("#include <math.h>\n");}
        if backend==Backend::NvidiaCuda && program_uses_f16(self) {s.push_str("#include <cuda_fp16.h>\n");}
        if backend==Backend::AmdHip && program_uses_f16(self) {s.push_str("#include <hip/hip_fp16.h>\n");}
        if backend==Backend::NvidiaCuda && program_uses_bf16(self) {s.push_str("#include <cuda_bf16.h>\n");}
        if backend==Backend::AmdHip && program_uses_bf16(self) {s.push_str("#include <hip/hip_bf16.h>\n");}
        if backend==Backend::Wgsl && program_uses_f16(self) {s.push_str("enable f16;\n");}
        if backend==Backend::IntelOpenCl && program_uses_f16(self) {s.push_str("#pragma OPENCL EXTENSION cl_khr_fp16 : enable\n");}
        for r in ordered_records(self)? {
            if backend==Backend::Wgsl{s.push_str(&format!("struct {} {{\n",r.name));for f in &r.fields{s.push_str(&format!("    {}: {},\n",f.name,render_ty(&f.ty,backend)?));}s.push_str("}\n");}
            else {let ordinary_shader=matches!(backend,Backend::Glsl|Backend::Hlsl);s.push_str(&format!("{}struct {} {{\n",if ordinary_shader{""}else{"typedef "},r.name));for f in &r.fields{s.push_str(&format!("    {};\n",render_declaration(&f.name,&f.ty,backend)?));}if ordinary_shader{s.push_str("};\n")}else{s.push_str(&format!("}} {};\n",r.name));}}
        }
        if backend==Backend::Hlsl {s.push_str(&emit_hlsl_record_constructors(self)?);}
        s.push_str(&emit_tuple_declarations(self,backend)?);
        if backend!=Backend::Wgsl { for f in &self.functions {
            let args=render_params(&f.args,backend)?;
            let prefix=if matches!(backend,Backend::NvidiaCuda|Backend::AmdHip){"__device__ "}else{""};
            s.push_str(&format!("{}{} {}({});\n",prefix,render_ty(&f.result,backend)?,f.name,args));
        }}
        for f in &self.functions {
            let args=render_params(&f.args,backend)?;
            let decl=match backend{Backend::Wgsl=>{if f.result==DataType::Void {format!("fn {}({})",f.name,args)}else{format!("fn {}({}) -> {}",f.name,args,render_ty(&f.result,backend)?) }},Backend::NvidiaCuda|Backend::AmdHip=>format!("__device__ {} {}({})",render_ty(&f.result,backend)?,f.name,args),_=>format!("{} {}({})",render_ty(&f.result,backend)?,f.name,args)};
            s.push_str(&decl);s.push_str(" {\n");emit_block(&mut s,&f.body,backend,1)?;s.push_str("}\n");
        }
        for k in &self.kernels {
            match backend {
                Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip => {
                    let args=render_params(&k.args,backend)?;
                    let decl=match backend {
                        Backend::IntelOpenCl=>format!("__kernel void {}({})",k.name,args),
                        _=>format!("extern \"C\" __global__ void {}({})",k.name,args),
                    };
                    s.push_str(&decl);s.push_str(" {\n");emit_block(&mut s,&k.body,backend,1)?;s.push_str("}\n");
                }
                Backend::Wgsl|Backend::Glsl|Backend::Hlsl => emit_shader_kernel(&mut s,k,backend,self)?,
            }
        }
        Ok(s)
    }
}
fn validate_parameters(args:&[Parameter])->Result<(),IrError>{
    for (i,p) in args.iter().enumerate(){if !is_identifier(&p.name)||is_reserved(&p.name){return Err(IrError::InvalidIdentifier)} p.ty.validate()?;if p.ty==DataType::Void{return Err(IrError::InvalidType)}
        if args[..i].iter().any(|a|a.name==p.name){return Err(IrError::DuplicateName)} }
    Ok(())
}
fn scalar(n:Number,b:Backend)->Result<&'static str,IrError>{
    use Number::*;
    let code=match (n,b) {
        (Bool,_)=>"bool",(I8,Backend::IntelOpenCl)=>"char",(U8,Backend::IntelOpenCl)=>"uchar",
        (I16,Backend::IntelOpenCl)=>"short",(U16,Backend::IntelOpenCl)=>"ushort",
        (I32,Backend::IntelOpenCl)=>"int",(U32,Backend::IntelOpenCl)=>"uint",
        (I64,Backend::IntelOpenCl)=>"long",(U64,Backend::IntelOpenCl)=>"ulong",
        (F16,Backend::IntelOpenCl)=>"half",(F32,Backend::IntelOpenCl)=>"float",(F64,Backend::IntelOpenCl)=>"double",
        (I8,Backend::NvidiaCuda|Backend::AmdHip)=>"signed char",(U8,Backend::NvidiaCuda|Backend::AmdHip)=>"unsigned char",
        (I16,Backend::NvidiaCuda|Backend::AmdHip)=>"short",(U16,Backend::NvidiaCuda|Backend::AmdHip)=>"unsigned short",
        (I32,Backend::NvidiaCuda|Backend::AmdHip)=>"int",(U32,Backend::NvidiaCuda|Backend::AmdHip)=>"unsigned int",
        (I64,Backend::NvidiaCuda|Backend::AmdHip)=>"long long",(U64,Backend::NvidiaCuda|Backend::AmdHip)=>"unsigned long long",
        (F16,Backend::NvidiaCuda|Backend::AmdHip)=>"__half",
        (Bf16,Backend::NvidiaCuda)=>"__nv_bfloat16",
        (Bf16,Backend::AmdHip)=>"hip_bfloat16",
        (F32,Backend::NvidiaCuda|Backend::AmdHip)=>"float",(F64,Backend::NvidiaCuda|Backend::AmdHip)=>"double",
        (I32,Backend::Wgsl)=>"i32",(U32,Backend::Wgsl)=>"u32",(F32,Backend::Wgsl)=>"f32",(F16,Backend::Wgsl)=>"f16",
        (I32,Backend::Glsl)=>"int",(U32,Backend::Glsl)=>"uint",(F32,Backend::Glsl)=>"float",(F64,Backend::Glsl)=>"double",
        (I32,Backend::Hlsl)=>"int",(U32,Backend::Hlsl)=>"uint",(F32,Backend::Hlsl)=>"float",(F64,Backend::Hlsl)=>"double",
        _=>return Err(IrError::Unsupported{backend:b,detail:"numeric type requires target feature or native representation"}),
    };Ok(code)
}
fn render_ty(ty:&DataType,b:Backend)->Result<String,IrError>{match ty {
    DataType::Void=>if b==Backend::Wgsl{Err(IrError::Unsupported{backend:b,detail:"WGSL has no void value type"})}else{Ok("void".into())},DataType::Scalar(n)=>Ok(scalar(*n,b)?.into()),
    DataType::Record(n)=>Ok(n.clone()),
    DataType::Tuple(_)=>Ok(tuple_name(ty)),
    DataType::Array(inner,n)=>if b==Backend::Wgsl{Ok(format!("array<{}, {}>",render_ty(inner,b)?,n))}else{Err(IrError::Unsupported{backend:b,detail:"arrays must be rendered in declaration context"})},
    DataType::Pointer(_,_,_)=>Err(IrError::Unsupported{backend:b,detail:"pointer types require declaration context"}),
    DataType::Vector(n,w)=>{
        if ![2,3,4,8,16].contains(w){return Err(IrError::InvalidType)}
        if matches!(b,Backend::Wgsl|Backend::Glsl|Backend::Hlsl) && *w>4{return Err(IrError::Unsupported{backend:b,detail:"WGSL vectors are at most four lanes"})}
        let t=scalar(*n,b)?;
        match b {Backend::Wgsl=>Ok(format!("vec{}<{}>",w,t)),Backend::Glsl=>{let prefix=match n {Number::I32=>"ivec",Number::U32=>"uvec",Number::F32=>"vec",Number::F64=>"dvec",Number::Bool=>"bvec",_=>return Err(IrError::Unsupported{backend:b,detail:"GLSL vector scalar type unsupported"})};Ok(format!("{}{}",prefix,w))},Backend::IntelOpenCl|Backend::Hlsl=>Ok(format!("{}{}",t,w)),
            Backend::NvidiaCuda|Backend::AmdHip=>{
                let prefix=match n{Number::F32=>"float",Number::F64=>"double",Number::I32=>"int",Number::U32=>"uint",
                _=>return Err(IrError::Unsupported{backend:b,detail:"no native CUDA/HIP vector representation"})};
                if *w>4{return Err(IrError::Unsupported{backend:b,detail:"CUDA/HIP vector widths over four need explicit lowering"})}
                Ok(format!("{}{}",prefix,w))
            }}
    },
}}
fn render_param(p:&Parameter,b:Backend)->Result<String,IrError>{
    match &p.ty {
        DataType::Pointer(ty,space,access)=>{
            let el=render_ty(ty,b)?;
            let prefix=match (b,space) {
                (Backend::IntelOpenCl,AddressSpace::Global)=>"__global ",
                (Backend::IntelOpenCl,AddressSpace::Workgroup)=>"__local ",
                (Backend::IntelOpenCl,AddressSpace::Constant)=>"__constant ",
                (Backend::IntelOpenCl,AddressSpace::Private)=>"__private ",
                (Backend::NvidiaCuda|Backend::AmdHip,AddressSpace::Global)=>"",
                (Backend::NvidiaCuda|Backend::AmdHip,AddressSpace::Constant)=>return Err(IrError::Unsupported{backend:b,detail:"native constant memory needs a constant symbol, not a kernel pointer parameter"}),
                _=>return Err(IrError::Unsupported{backend:b,detail:"address space cannot be represented as kernel parameter"}),
            };
            let c=if *access==Access::ReadOnly {"const "}else{""};
            Ok(format!("{}{}{}* {}",c,prefix,el,p.name))
        },
        DataType::Array(_,_)=>render_declaration(&p.name,&p.ty,b),
        _=> if b==Backend::Wgsl {Ok(format!("{}: {}",p.name,render_ty(&p.ty,b)?))}else{Ok(format!("{} {}",render_ty(&p.ty,b)?,p.name))},
    }
}
fn render_params(ps:&[Parameter],b:Backend)->Result<String,IrError>{ps.iter().map(|p|render_param(p,b)).collect::<Result<Vec<_>,_>>().map(|v|v.join(", "))}
fn val(v:&Value,b:Backend)->Result<String,IrError>{match v {
    Value::Var(s,t)=>{render_reference_type(t,b)?; if !is_identifier(s){return Err(IrError::InvalidIdentifier)} Ok(s.clone())},
    Value::Bool(x)=>Ok(x.to_string()), Value::Int(x,t)=>{render_ty(t,b)?;Ok(x.to_string())},
    Value::UInt(x,t)=>{render_ty(t,b)?;Ok(if *t==DataType::Scalar(Number::U64) {format!("{}ULL",x)}else {format!("{}u",x)})},
    Value::F64Bits(bits)=>{let value=f64::from_bits(*bits);if !value.is_finite(){return Err(IrError::InvalidType)}
        if b==Backend::Glsl || b==Backend::Hlsl{Ok(format!("double({:?})",value))}else{Ok(format!("{:?}",value))}},
    Value::FloatBits(bits)=>{let x=f32::from_bits(*bits);if !x.is_finite(){return Err(IrError::InvalidType)}let mut t=format!("{:?}",x);if matches!(b,Backend::Wgsl|Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip){t.push_str("f");}Ok(t)},
    Value::Binary(op,a,c,_)=>{let sym=match op{Binary::Add=>"+",Binary::Sub=>"-",Binary::Mul=>"*",Binary::Div=>"/",Binary::Rem=>"%",Binary::Lt=>"<",Binary::Le=>"<=",Binary::Gt=>">",Binary::Ge=>">=",Binary::Eq=>"==",Binary::Ne=>"!=",Binary::And=>"&&",Binary::Or=>"||"};
        if matches!(b,Backend::NvidiaCuda|Backend::AmdHip){if let DataType::Vector(n,w)=a.ty(){
            if ![2,3,4].contains(&w) {return Err(IrError::Unsupported{backend:b,detail:"vector binary width unsupported"})}
            let prefix=match n{Number::F32=>"float",Number::F64=>"double",Number::I32=>"int",Number::U32=>"uint",
                _=>return Err(IrError::Unsupported{backend:b,detail:"vector binary type unsupported"})};
            if matches!(op,Binary::Lt|Binary::Le|Binary::Gt|Binary::Ge|Binary::Eq|Binary::Ne|Binary::And|Binary::Or){return Err(IrError::Unsupported{backend:b,detail:"vector comparison needs a mask result"})}
            let lhs=val(a,b)?;let rhs=val(c,b)?;let mut components=Vec::new();
            for part in ["x","y","z","w"].iter().take(w as usize){components.push(format!("(({}).{} {} ({}).{})",lhs,part,sym,rhs,part));}
            return Ok(format!("make_{}{}({})",prefix,w,components.join(", ")))
        }}
        Ok(format!("({} {} {})",val(a,b)?,sym,val(c,b)?))},
    Value::Load(p,i,_)=>Ok(format!("{}[{}]",val(p,b)?,val(i,b)?)),
    Value::Builtin(kind,axis)=>{if *axis>2{return Err(IrError::InvalidDimension)} let ax=["x","y","z"][*axis as usize];
        let s=match b {
            Backend::IntelOpenCl=>match kind{Builtin::GlobalId=>"get_global_id",Builtin::LocalId=>"get_local_id",Builtin::GroupId=>"get_group_id",Builtin::GlobalSize=>"get_global_size",Builtin::LocalSize=>"get_local_size",Builtin::NumGroups=>"get_num_groups"},
            Backend::NvidiaCuda|Backend::AmdHip=>return Ok(match kind{
                Builtin::GlobalId=>format!("((unsigned int)(blockIdx.{ax} * blockDim.{ax} + threadIdx.{ax}))"),
                Builtin::LocalId=>format!("threadIdx.{ax}"),Builtin::GroupId=>format!("blockIdx.{ax}"),
                Builtin::GlobalSize=>format!("(gridDim.{ax} * blockDim.{ax})"),Builtin::LocalSize=>format!("blockDim.{ax}"),Builtin::NumGroups=>format!("gridDim.{ax}")}),
            Backend::Wgsl|Backend::Glsl|Backend::Hlsl=>return Ok(shader_builtin(*kind,ax,b)?),
        };Ok(format!("((uint){}({}))",s,axis))
    },
    Value::Field(o,f,_)=>{if !is_identifier(f){return Err(IrError::InvalidIdentifier)}Ok(format!("{}.{}",val(o,b)?,f))},
    Value::Compute(op,args,_)=>emit_compute(*op,args,b),
    Value::Call(name,args,_)=>{if !is_identifier(name){return Err(IrError::InvalidIdentifier)}Ok(format!("{}({})",name,args.iter().map(|x|val(x,b)).collect::<Result<Vec<_>,_>>()?.join(", ")))},
    Value::Cast(value,to)=>{let from=value.ty();let inner=val(value,b)?;
        if !matches!(from,DataType::Scalar(_)) || !matches!(to,DataType::Scalar(_)){return Err(IrError::InvalidType)}
        let target=render_ty(to,b)?;
        if b==Backend::NvidiaCuda {
            if from==DataType::Scalar(Number::F32) && *to==DataType::Scalar(Number::Bf16){return Ok(format!("__float2bfloat16_rn({})",inner))}
            if from==DataType::Scalar(Number::Bf16) && *to==DataType::Scalar(Number::F32){return Ok(format!("__bfloat162float({})",inner))}
        }
        if b==Backend::Wgsl {Ok(format!("{}({})",target,inner))}
        else {Ok(format!("(({})({}))",target,inner))}},
    Value::Tuple(values)=>{let ty=v.ty();let fields=values.iter().map(|x|val(x,b)).collect::<Result<Vec<_>,_>>()?.join(", ");
        if b==Backend::Wgsl {Ok(format!("{}({})",tuple_name(&ty),fields))}
        else if b==Backend::IntelOpenCl {Ok(format!("(({}){{{}}})",tuple_name(&ty),fields))}
        else if matches!(b,Backend::Glsl) {Ok(format!("{}({})",tuple_name(&ty),fields))}
        else if b==Backend::Hlsl {Ok(format!("_mata_ctor_{}({})",tuple_name(&ty),fields))}
        else {Ok(format!("{}{{{}}}",tuple_name(&ty),fields))}},
    Value::TupleAt(parent,i,_)=>Ok(format!("{}.value{}",val(parent,b)?,i)),
    Value::RecordInit(name,fields)=>{
        if b==Backend::Hlsl && fields.iter().any(|(_,v)|matches!(v.ty(),DataType::Array(_,_)|DataType::Pointer(_,_,_))) {
            return Err(IrError::Unsupported{backend:b,detail:"HLSL array-valued record constructor requires explicit field assignments"});
        }
        let args=fields.iter().map(|(_,value)|val(value,b)).collect::<Result<Vec<_>,_>>()?.join(", ");
        Ok(match b{Backend::Wgsl|Backend::Glsl=>format!("{}({})",name,args),
            Backend::IntelOpenCl=>format!("(({}){{{}}})",name,args),
            Backend::Hlsl=>format!("_mata_make_{}({})",name,args),
            _=>format!("{}{{{}}}",name,args)})
    },
    Value::ArrayInit(items,t)=>{
        let args=items.iter().map(|v|val(v,b)).collect::<Result<Vec<_>,_>>()?.join(", ");
        match b{Backend::Wgsl=>Ok(format!("{}({})",render_ty(t,b)?,args)),
            Backend::Glsl=>{if let DataType::Array(inner,n)=t{Ok(format!("{}[{}]({})",render_ty(inner,b)?,n,args))}else{Err(IrError::InvalidType)}},
            _=>Ok(format!("{{{}}}",args))}
    },
    Value::VectorInit(items,n)=>{
        let args=items.iter().map(|v|val(v,b)).collect::<Result<Vec<_>,_>>()?.join(", ");
        let ty=DataType::Vector(*n,items.len() as u8);
        let ty_name=render_ty(&ty,b)?;
        match b {Backend::IntelOpenCl=>Ok(format!("(({})({}))",ty_name,args)),
            Backend::NvidiaCuda|Backend::AmdHip=>Ok(format!("make_{}({})",ty_name,args)),
            _=>Ok(format!("{}({})",ty_name,args))}
    },
    Value::Index(parent,index,_)=>Ok(format!("{}[{}]",val(parent,b)?,val(index,b)?)),
}}
fn render_reference_type(t:&DataType,b:Backend)->Result<(),IrError>{match t{DataType::Pointer(inner,_,_)|DataType::Array(inner,_)=>render_reference_type(inner,b),_=>{render_ty(t,b)?;Ok(())}}}
fn validate_block(stmts:&[Statement],b:Backend)->Result<(),IrError>{let mut discard=String::new();emit_block(&mut discard,stmts,b,0)}
fn emit_block(s:&mut String,stmts:&[Statement],b:Backend,depth:usize)->Result<(),IrError>{
    if depth>64{return Err(IrError::InvalidControlFlow)}
    for stmt in stmts {
        let pad="    ".repeat(depth);
        match stmt {
            Statement::Let(name,ty,x)=>{if !is_identifier(name){return Err(IrError::InvalidIdentifier)}if &x.ty()!=ty{return Err(IrError::TypeMismatch)}let init=val(x,b)?;
                if b==Backend::Wgsl{s.push_str(&format!("{}var {}: {} = {};\n",pad,name,render_ty(ty,b)?,init));}
                else{s.push_str(&format!("{}{} = {};\n",pad,render_declaration(name,ty,b)?,init));}},
            Statement::Store(p,i,x)=>{if let DataType::Pointer(ty,_,access)=p.ty(){if access==Access::ReadOnly{return Err(IrError::TypeMismatch)}if *ty!=x.ty(){return Err(IrError::TypeMismatch)}}else{return Err(IrError::InvalidType)}
                s.push_str(&format!("{}{}[{}] = {};\n",pad,val(p,b)?,val(i,b)?,val(x,b)?));},
            Statement::Assign(name,x)=>{if !is_identifier(name){return Err(IrError::InvalidIdentifier)}s.push_str(&format!("{}{} = {};\n",pad,name,val(x,b)?));},
            Statement::If(c,yes,no)=>{if c.ty()!=DataType::Scalar(Number::Bool){return Err(IrError::TypeMismatch)}s.push_str(&format!("{}if ({}) {{\n",pad,val(c,b)?));emit_block(s,yes,b,depth+1)?;s.push_str(&format!("{}}}",pad));if !no.is_empty(){s.push_str(" else {\n");emit_block(s,no,b,depth+1)?;s.push_str(&format!("{}}}",pad));}s.push('\n');},
            Statement::While(c,body)=>{if c.ty()!=DataType::Scalar(Number::Bool){return Err(IrError::TypeMismatch)}s.push_str(&format!("{}while ({}) {{\n",pad,val(c,b)?));emit_block(s,body,b,depth+1)?;s.push_str(&format!("{}}}\n",pad));},
            Statement::ForRange{index,start,end,body}=>{if !is_identifier(index){return Err(IrError::InvalidIdentifier)}if start.ty()!=DataType::Scalar(Number::I32)||end.ty()!=DataType::Scalar(Number::I32){return Err(IrError::TypeMismatch)}
                let init=if b==Backend::Wgsl{"var"}else{"int"};let decl=if b==Backend::Wgsl{format!("{} {}: i32",init,index)}else{format!("{} {}",init,index)};
                s.push_str(&format!("{}for ({} = {}; {} < {}; {}++) {{\n",pad,decl,val(start,b)?,index,val(end,b)?,index));emit_block(s,body,b,depth+1)?;s.push_str(&format!("{}}}\n",pad));},
            Statement::Return(v)=>{if let Some(v)=v{s.push_str(&format!("{}return {};\n",pad,val(v,b)?));}else{s.push_str(&format!("{}return;\n",pad));}},
            Statement::Expr(v)=>{
                if matches!(b,Backend::Wgsl) && !matches!(v,Value::Call(_,_,_)){
                    return Err(IrError::Unsupported{backend:b,detail:"WGSL expression statements must be calls"});
                }
                s.push_str(&format!("{}{};\n",pad,val(v,b)?))
            },
            Statement::SetIndex(parent,index,value)=>{
                s.push_str(&format!("{}{}[{}] = {};\n",pad,val(parent,b)?,val(index,b)?,val(value,b)?));
            },
            Statement::Shared{name,element,len}=>{
                if !matches!(b,Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip) {
                    return Err(IrError::Unsupported{backend:b,detail:"static shared arrays require native workgroup declarations"});
                }
                if *len==0 || *len>1_000_000 || *element==Number::Bool{return Err(IrError::InvalidType)}
                let qualifier=if b==Backend::IntelOpenCl{"__local"}else{"__shared__"};
                s.push_str(&format!("{}{} {} {}[{}];\n",pad,qualifier,scalar(*element,b)?,name,len));
            },
            Statement::AtomicAdd{pointer,index,delta}=>{
                if !matches!(b,Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip) {
                    return Err(IrError::Unsupported{backend:b,detail:"portable 32-bit atomic add is limited to OpenCL/CUDA/HIP in Phase 3"});
                }
                if !matches!(pointer.ty(),DataType::Pointer(_,AddressSpace::Global|AddressSpace::Workgroup,Access::ReadWrite)) {return Err(IrError::InvalidType)}
                let intrinsic=if b==Backend::IntelOpenCl{"atomic_add"}else{"atomicAdd"};
                s.push_str(&format!("{}{}(&{}[{}], {});\n",pad,intrinsic,val(pointer,b)?,val(index,b)?,val(delta,b)?));
            },
            Statement::Atomic(_)|Statement::Fence(_)|Statement::AsyncCopy{..}|Statement::AwaitCopy(_)=>sync_emit(s,stmt,b,depth)?,
            Statement::Barrier=>{let name=match b{Backend::IntelOpenCl=>"barrier(CLK_LOCAL_MEM_FENCE)",Backend::NvidiaCuda|Backend::AmdHip=>"__syncthreads()",Backend::Wgsl=>"workgroupBarrier()",Backend::Glsl=>"barrier()",Backend::Hlsl=>"GroupMemoryBarrierWithGroupSync()"};s.push_str(&format!("{}{};\n",pad,name));},
        }
    }Ok(())
}


/// Emit an ordinary declaration, keeping C-style array dimensions after the name.
fn render_declaration(name:&str,ty:&DataType,b:Backend)->Result<String,IrError>{
    if !is_identifier(name){return Err(IrError::InvalidIdentifier)}
    match ty {
        DataType::Array(inner,n) => {
            if b==Backend::Wgsl {Ok(format!("{}: {}",name,render_ty(ty,b)?))}
            else {let mut dims=alloc::vec![*n];let mut base=inner.as_ref();
                while let DataType::Array(item,size)=base {dims.push(*size);base=item;}
                let mut declaration=format!("{} {}",render_ty(base,b)?,name);
                for dim in dims{declaration.push_str(&format!("[{}]",dim));}Ok(declaration)}
        }
        _=>if b==Backend::Wgsl {Ok(format!("{}: {}",name,render_ty(ty,b)?))}
        else {Ok(format!("{} {}",render_ty(ty,b)?,name))}
    }
}
fn shader_builtin(kind:Builtin,ax:&str,b:Backend)->Result<String,IrError>{
    let base=match (b,kind) {
        (Backend::Wgsl,Builtin::GlobalId)=>"_global_id",(Backend::Wgsl,Builtin::LocalId)=>"_local_id",(Backend::Wgsl,Builtin::GroupId)=>"_group_id",
        (Backend::Wgsl,Builtin::LocalSize)=>"_local_size",(Backend::Wgsl,Builtin::GlobalSize)=>"_global_size",(Backend::Wgsl,Builtin::NumGroups)=>"_num_groups",
        (Backend::Glsl,Builtin::GlobalId)=>"gl_GlobalInvocationID",(Backend::Glsl,Builtin::LocalId)=>"gl_LocalInvocationID",(Backend::Glsl,Builtin::GroupId)=>"gl_WorkGroupID",
        (Backend::Glsl,Builtin::LocalSize)=>"gl_WorkGroupSize",(Backend::Glsl,Builtin::GlobalSize)=>"_global_size",(Backend::Glsl,Builtin::NumGroups)=>"gl_NumWorkGroups",
        (Backend::Hlsl,Builtin::GlobalId)=>"_global_id",(Backend::Hlsl,Builtin::LocalId)=>"_local_id",(Backend::Hlsl,Builtin::GroupId)=>"_group_id",
        (Backend::Hlsl,Builtin::LocalSize)=>"_local_size",(Backend::Hlsl,Builtin::GlobalSize)=>"_global_size",(Backend::Hlsl,Builtin::NumGroups)=>"_num_groups",
        _=>return Err(IrError::Unsupported{backend:b,detail:"unsupported shader builtin"}),
    };
    let expr=format!("{}.{}",base,ax);
    // All portable work IDs are u32; native 64-bit get_global_id is narrowed explicitly.
    Ok(expr)
}
fn emit_shader_kernel(s:&mut String,k:&Kernel,b:Backend,program:&Program)->Result<(),IrError>{
    let mut resources=String::new();
    let mut uniform_fields=String::new();
    for (i,p) in k.args.iter().enumerate(){
        if let DataType::Pointer(inner,space,access)=&p.ty {
            if !matches!(inner.as_ref(),DataType::Scalar(_)|DataType::Vector(_,_)|DataType::Record(_)) {
                return Err(IrError::Unsupported{backend:b,detail:"shader buffer elements must be scalar, vector, or record"});
            }
            // A declared resource must have a representable host-side layout.
            // This also rejects booleans and pointer-containing records.
            let _=program.layout_for_backend(inner,b,AddressSpace::Global)?;
            if !matches!(space,AddressSpace::Global){return Err(IrError::Unsupported{backend:b,detail:"only global pointers are supported as shader buffers"})}
            let elem=render_ty(inner,b)?;
            match b {
                Backend::Wgsl=>{
                    let mode=if *access==Access::ReadOnly{"read"}else{"read_write"};
                    if *access==Access::WriteOnly{return Err(IrError::Unsupported{backend:b,detail:"WGSL has no write-only storage buffer"})}
                    resources.push_str(&format!("@group(0) @binding({}) var<storage, {}> {}: array<{}>;\n",i,mode,p.name,elem));
                }
                Backend::Glsl=>{
                    let restrict=if *access==Access::ReadOnly{"readonly "}else if *access==Access::WriteOnly{"writeonly "}else{""};
                    resources.push_str(&format!("layout(std430, binding = {}) {}buffer _buffer_{} {{ {} {}[]; }};\n",i,restrict,p.name,elem,p.name));
                }
                Backend::Hlsl=>{
                    let qualifier=if *access==Access::ReadOnly{"StructuredBuffer"}else{"RWStructuredBuffer"};
                    let reg=if *access==Access::ReadOnly{'t'}else{'u'};
                    resources.push_str(&format!("{}<{}> {} : register({}{});\n",qualifier,elem,p.name,reg,i));
                }
                _=>unreachable!(),
            }
        } else {
            if b==Backend::Wgsl {let _=program.wgsl_uniform_layout(&p.ty)?;}
            if b==Backend::Hlsl {let _=program.hlsl_cbuffer_layout(&p.ty)?;}
            if b==Backend::Glsl {let _=program.uniform_layout(&p.ty)?;}
            if b==Backend::Wgsl {
                if matches!(p.ty,DataType::Scalar(Number::Bool)|DataType::Vector(Number::Bool,_)) {return Err(IrError::InvalidLayout)}
                if !matches!(p.ty,DataType::Scalar(_) | DataType::Vector(_,_) | DataType::Record(_)){return Err(IrError::Unsupported{backend:b,detail:"scalar uniform argument type unsupported"})}
                uniform_fields.push_str(&format!("    {}: {},\n",p.name,render_ty(&p.ty,b)?));
            } else if b==Backend::Glsl {
                resources.push_str(&format!("layout(location = {}) uniform {} {};\n",i,render_ty(&p.ty,b)?,p.name));
            } else {
                uniform_fields.push_str(&format!("    {} {};\n",render_ty(&p.ty,b)?,p.name));
            }
        }
    }
    if b==Backend::Wgsl && !uniform_fields.is_empty(){
        // The wrapper exposes uniform fields as lexical `let` declarations.
        resources.push_str(&format!("struct _Parameters_{} {{\n{}}}\n@group(0) @binding({}) var<uniform> _params_{}: _Parameters_{};\n",k.name,uniform_fields,k.args.len(),k.name,k.name));
    }
    if b==Backend::Hlsl && !uniform_fields.is_empty(){resources.push_str(&format!("cbuffer _Parameters_{} : register(b0) {{\n{}}};\n",k.name,uniform_fields));}
    if b==Backend::Hlsl && block_uses_global_size(&k.body) {
        resources.push_str("cbuffer _MataDispatch : register(b1) { uint3 _mata_num_groups; };\n");
    }
    // GLSL has one entry point per compute translation unit; when multiple
    // kernels are registered, callers must compile entry points individually.
    s.push_str(&resources);
    match b {
        Backend::Wgsl=>{
            s.push_str(&format!("@compute @workgroup_size({}, {}, {}) fn {}(@builtin(global_invocation_id) _global_id: vec3<u32>, @builtin(local_invocation_id) _local_id: vec3<u32>, @builtin(workgroup_id) _group_id: vec3<u32>, @builtin(num_workgroups) _num_groups: vec3<u32>) {{\n",k.workgroup[0],k.workgroup[1],k.workgroup[2],k.name));
            s.push_str(&format!("    let _local_size: vec3<u32> = vec3<u32>({}, {}, {});\n    let _global_size: vec3<u32> = _num_groups * _local_size;\n",k.workgroup[0],k.workgroup[1],k.workgroup[2]));
            for p in &k.args {if !matches!(p.ty,DataType::Pointer(_,_,_)){s.push_str(&format!("    let {} = _params_{}.{};\n",p.name,k.name,p.name));}}
        }
        Backend::Glsl=>{
            s.push_str(&format!("layout(local_size_x = {}, local_size_y = {}, local_size_z = {}) in;\nvoid main() {{\n",k.workgroup[0],k.workgroup[1],k.workgroup[2]));
            s.push_str("    uvec3 _global_size = gl_NumWorkGroups * gl_WorkGroupSize;\n");
        }
        Backend::Hlsl=>{
            s.push_str(&format!("[numthreads({}, {}, {})] void {}(uint3 _global_id : SV_DispatchThreadID, uint3 _local_id : SV_GroupThreadID, uint3 _group_id : SV_GroupID) {{\n",k.workgroup[0],k.workgroup[1],k.workgroup[2],k.name));
            s.push_str(&format!("    uint3 _local_size = uint3({}, {}, {});\n",k.workgroup[0],k.workgroup[1],k.workgroup[2]));
            if block_uses_global_size(&k.body) {
                s.push_str("    uint3 _num_groups = _mata_num_groups;\n    uint3 _global_size = _num_groups * _local_size;\n");
            }
        }
        _=>unreachable!(),
    }
    emit_block(s,&k.body,b,1)?;
    s.push_str("}\n");
    Ok(())
}
fn block_uses_global_size(block:&[Statement])->bool{
    fn uses(v:&Value)->bool {match v {
        Value::Builtin(Builtin::GlobalSize|Builtin::NumGroups,_)=>true,
        Value::Binary(_,a,b,_)|Value::Load(a,b,_)|Value::Index(a,b,_)=>uses(a)||uses(b),
        Value::Field(a,_,_)|Value::Cast(a,_)|Value::TupleAt(a,_,_)=>uses(a),
        Value::Call(_,args,_)|Value::Compute(_,args,_)|Value::Tuple(args)|Value::ArrayInit(args,_)|Value::VectorInit(args,_)=>args.iter().any(uses),
        Value::RecordInit(_,args)=>args.iter().any(|(_,v)|uses(v)),_=>false}}
    block.iter().any(|s|match s{Statement::Let(_,_,v)|Statement::Assign(_,v)|Statement::Expr(v)=>uses(v),Statement::Store(a,b,c)|Statement::SetIndex(a,b,c)=>uses(a)||uses(b)||uses(c),Statement::AtomicAdd{pointer,index,delta}=>uses(pointer)||uses(index)||uses(delta),Statement::Shared{..}=>false,Statement::If(c,a,b)=>uses(c)||block_uses_global_size(a)||block_uses_global_size(b),Statement::While(c,a)=>uses(c)||block_uses_global_size(a),Statement::ForRange{start,end,body,..}=>uses(start)||uses(end)||block_uses_global_size(body),Statement::Return(v)=>v.as_ref().map_or(false,uses),Statement::Atomic(a)=>uses(&a.pointer)||uses(&a.index)||a.value.as_ref().map_or(false,uses)||a.expected.as_ref().map_or(false,uses),Statement::AsyncCopy{destination,source,count,..}=>uses(destination)||uses(source)||uses(count),Statement::Fence(_)|Statement::AwaitCopy(_)|Statement::Barrier=>false})
}

include!("ir_compute.rs");
include!("ir_layout.rs");
include!("ir_semantics.rs");
include!("ir_safety.rs");
include!("ir_sync.rs");

fn block_uses_any_builtin(block:&[Statement])->bool {
    fn has(v:&Value)->bool{match v {
        Value::Builtin(_,_)=>true,
        Value::Binary(_,a,b,_)|Value::Load(a,b,_)|Value::Index(a,b,_)=>has(a)||has(b),
        Value::Field(v,_,_)|Value::Cast(v,_)|Value::TupleAt(v,_,_)=>has(v),
        Value::Call(_,vs,_)|Value::Compute(_,vs,_)|Value::Tuple(vs)|Value::ArrayInit(vs,_)|Value::VectorInit(vs,_)=>vs.iter().any(has),
        Value::RecordInit(_,vs)=>vs.iter().any(|(_,v)|has(v)),_=>false
    }}
    block.iter().any(|stmt|match stmt {
        Statement::Let(_,_,v)|Statement::Assign(_,v)|Statement::Expr(v)=>has(v),
        Statement::Store(a,b,c)|Statement::SetIndex(a,b,c)=>has(a)||has(b)||has(c),Statement::AtomicAdd{pointer,index,delta}=>has(pointer)||has(index)||has(delta),
        Statement::If(c,a,b)=>has(c)||block_uses_any_builtin(a)||block_uses_any_builtin(b),
        Statement::While(c,a)=>has(c)||block_uses_any_builtin(a),
        Statement::ForRange{start,end,body,..}=>has(start)||has(end)||block_uses_any_builtin(body),
        Statement::Return(v)=>v.as_ref().map_or(false,has),Statement::Atomic(a)=>has(&a.pointer)||has(&a.index)||a.value.as_ref().map_or(false,has)||a.expected.as_ref().map_or(false,has),Statement::AsyncCopy{destination,source,count,..}=>has(destination)||has(source)||has(count),Statement::Fence(_)|Statement::AwaitCopy(_)|Statement::Barrier|Statement::Shared{..}=>false,
    })
}
