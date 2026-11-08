//! Typed shader expression emission; all CPU-side and hardware-independent.
use alloc::{string::{String, ToString}, vec::Vec, format};
use core::fmt;
use crate::{Backend, Stage, Type, Scalar, CapabilitySet, Capability, Intrinsic, Signature, Family};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Expression validation or unsupported-target error, before shader compilation.
pub enum Error {
    InvalidIdentifier,
    InvalidType,
    InvalidLiteral,
    /// The named intrinsic was not found in the catalog.
    UnknownIntrinsicName,
    /// A backend-specific function pointer was given a different backend emitter.
    BackendMismatch { expected: Backend, actual: Backend },
    Unsupported { operation: &'static str, backend: Backend },
    FeatureDisabled(Capability),
    WrongStage { operation: &'static str, required: Stage },
    WrongArity { operation: &'static str, min:u8, max:u8, actual:usize },
    WrongTypes { operation: &'static str },
    ExplicitReturnTypeRequired(&'static str),
    FunctionHasNoReturn,
    FunctionReturnTypeMismatch,
    DuplicateParameter,
}
impl fmt::Display for Error {
    fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{write!(f,"{:?}",self)}
}
#[cfg(feature="std")]
impl std::error::Error for Error {}

/// Check the ASCII syntax for an unqualified shader identifier.
/// Does not check target-language reserved words.
pub fn is_identifier(s:&str)->bool {
    let mut it=s.bytes();
    let Some(c)=it.next() else{return false};
    if !(c.is_ascii_alphabetic()||c==b'_'){return false}
    it.all(|c|c.is_ascii_alphanumeric()||c==b'_')
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A typed expression represented by shader source; not an executed GPU instruction.
pub struct Expr { code:String, ty:Type }
impl Expr {
    /// Construct a shader variable reference using a simple ASCII identifier.
    pub fn variable(name:&str,ty:Type)->Result<Self,Error>{
        if !is_identifier(name){return Err(Error::InvalidIdentifier)}
        if !ty.valid()||ty==Type::Void{return Err(Error::InvalidType)}
        Ok(Self{code:name.to_string(),ty})
    }
    /// Caller is responsible for shader-source safety and type correctness.
    /// Use for backend-specific variables, indexed values and struct members.
    pub fn from_source_unchecked(source:impl Into<String>,ty:Type)->Self{
        Self{code:source.into(),ty}
    }
    /// Create a finite 32-bit floating literal.
    pub fn float(v:f32)->Result<Self,Error>{
        if !v.is_finite(){return Err(Error::InvalidLiteral)}
        Ok(Self{code:format!("{:?}",v),ty:Type::F32})
    }
    /// Create a 32-bit signed integer literal.
    pub fn int(v:i32)->Self {Self{code:format!("{}",v),ty:Type::I32}}
    /// Create a 32-bit unsigned integer literal.
    pub fn uint(v:u32)->Self {Self{code:format!("{}u",v),ty:Type::U32}}
    /// Create a 64-bit unsigned integer literal for HIP wave64 ballot masks.
    pub fn uint64(v:u64)->Self {Self{code:format!("{}ULL",v),ty:Type::U64}}
    /// Create a Boolean literal.
    pub fn boolean(v:bool)->Self {Self{code:if v{"true"}else{"false"}.into(),ty:Type::BOOL}}
    /// Borrow the shader-source expression.
    pub fn code(&self)->&str {&self.code}
    /// Return the emitter-side type annotation.
    pub const fn ty(&self)->Type {self.ty}
}

#[derive(Clone, Copy, Debug)]
/// Stateless shader expression emitter configured with one target and stage.
pub struct Emitter { pub backend:Backend, pub stage:Stage, pub capabilities:CapabilitySet }
impl Emitter {
    /// Construct an emitter without probing any graphics hardware.
    pub const fn new(backend:Backend,stage:Stage,capabilities:CapabilitySet)->Self{
        Self {backend,stage,capabilities}
    }
    /// Check catalog spelling, declared shader capability, and stage.
    /// Does not validate the physical device or shader compiler version.
    pub fn supports(&self,op:Intrinsic)->bool {
        let meta=op.info();
        (!matches!(self.backend,Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip) || self.stage==Stage::Compute)
        && (self.backend!=Backend::IntelOpenCl || meta.capability!=Capability::IntelSubgroupsShort || self.capabilities.contains(Capability::IntelSubgroups))
        && (self.backend!=Backend::NvidiaCuda || meta.capability!=Capability::NvidiaSm80 || self.capabilities.contains(Capability::NvidiaWarp))
        && (self.backend!=Backend::AmdHip || meta.capability!=Capability::AmdGcn || self.capabilities.contains(Capability::AmdWave))
        && meta.spelling(self.backend).is_some()
        && self.capabilities.contains(meta.capability)
        && meta.stage.map_or(true,|s|s==self.stage)
    }
    /// Perform preflight checking without invoking any shader compiler.
    fn validate_call(&self,op:Intrinsic,args:&[Expr])->Result<&'static str,Error>{
        if matches!(self.backend,Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip) && self.stage!=Stage::Compute {
            return Err(Error::WrongStage{operation:op.info().name,required:Stage::Compute});
        }
        let info=op.info();
        let symbol=info.spelling(self.backend)
            .ok_or(Error::Unsupported{operation:info.name,backend:self.backend})?;
        if !self.capabilities.contains(info.capability) {return Err(Error::FeatureDisabled(info.capability))}
        if self.backend==Backend::IntelOpenCl && info.capability==Capability::IntelSubgroupsShort
            && !self.capabilities.contains(Capability::IntelSubgroups) {
            return Err(Error::FeatureDisabled(Capability::IntelSubgroups));
        }
        if self.backend==Backend::NvidiaCuda && info.capability==Capability::NvidiaSm80
            && !self.capabilities.contains(Capability::NvidiaWarp) {
            return Err(Error::FeatureDisabled(Capability::NvidiaWarp));
        }
        if self.backend==Backend::AmdHip && info.capability==Capability::AmdGcn
            && !self.capabilities.contains(Capability::AmdWave) {
            return Err(Error::FeatureDisabled(Capability::AmdWave));
        }
        if let Some(stage)=info.stage {if stage!=self.stage{return Err(Error::WrongStage{operation:info.name,required:stage})}}
        if args.len()<info.arity.min as usize||args.len()>info.arity.max as usize{
            return Err(Error::WrongArity{operation:info.name,min:info.arity.min,max:info.arity.max,actual:args.len()});
        }
        Ok(symbol)
    }
    /// Result type is inferred for operations with safe, portable signatures.
    /// Opaque operations (textures, atomics, subgroup-specific return types)
    /// deliberately require `call_typed`, instead of guessing a type.
    /// Emit a built-in call and infer types for supported simple signatures.
    /// Returns an error instead of guessing special texture/atomic results.
    pub fn call(&self,op:Intrinsic,args:&[Expr])->Result<Expr,Error>{
        let symbol=self.validate_call(op,args)?;
        if matches!(self.backend,Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip) && args.iter().any(|a|matches!(a.ty(),Type::Matrix(..)|Type::Vector(Scalar::Bool,_))) {
            return Err(Error::WrongTypes{operation:op.info().name});
        }
        let ty=if op==Intrinsic::Transpose && args.len()==1 {
            if let Type::Matrix(c,r)=args[0].ty {Type::Matrix(r,c)}
            else {return Err(Error::WrongTypes{operation:op.info().name})}
        }else if matches!(op,Intrinsic::Unpack2x16Float|Intrinsic::Unpack2x16Unorm|Intrinsic::Unpack2x16Snorm|
            Intrinsic::UnpackHalf2x16|Intrinsic::UnpackUnorm2x16|Intrinsic::UnpackSnorm2x16) && args.len()==1 {
            if args[0].ty==Type::U32 {Type::VEC2}
            else {return Err(Error::WrongTypes{operation:op.info().name})}
        }else if op==Intrinsic::Select {
            if args.len()!=3 || args[0].ty!=args[1].ty || !args[2].ty.is_bool(){
                return Err(Error::WrongTypes{operation:op.info().name})
            }
            args[0].ty
        } else {infer_type(op.info().signature,args).ok_or_else(||{
            if op.info().signature==Signature::Special { Error::ExplicitReturnTypeRequired(op.info().name) }
            else {Error::WrongTypes{operation:op.info().name}}
        })?};
        self.render(op,args,symbol,ty)
    }
    /// Special operations use an explicitly declared return type. This is
    /// needed for texture formats, atomics and backend-specific subgroup ops.
    /// Backends will still perform their own shader-type validation at runtime.
    /// Emit a built-in expression with an explicit result type.
    /// Use this for texture, atomic, subgroup and backend-specific calls.
    pub fn call_typed(&self,op:Intrinsic,args:&[Expr],return_ty:Type)->Result<Expr,Error>{
        let symbol=self.validate_call(op,args)?;
        if matches!(self.backend,Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip)
            && (matches!(return_ty,Type::Matrix(..)|Type::Vector(Scalar::Bool,_))
                || args.iter().any(|a|matches!(a.ty(),Type::Matrix(..)|Type::Vector(Scalar::Bool,_)))){
            return Err(Error::WrongTypes{operation:op.info().name});
        }
        if !return_ty.valid(){return Err(Error::InvalidType)}
        if op.info().signature!=Signature::Special {
            let expected=if matches!(op,Intrinsic::Unpack2x16Float|Intrinsic::Unpack2x16Unorm|
                Intrinsic::Unpack2x16Snorm|Intrinsic::UnpackHalf2x16|Intrinsic::UnpackUnorm2x16|
                Intrinsic::UnpackSnorm2x16) && args.len()==1 {
                if args[0].ty==Type::U32 {Type::VEC2}
                else {return Err(Error::WrongTypes{operation:op.info().name})}
            }else if op==Intrinsic::Transpose && args.len()==1 {
                if let Type::Matrix(c,r)=args[0].ty {Type::Matrix(r,c)}
                else {return Err(Error::WrongTypes{operation:op.info().name})}
            }else{infer_type(op.info().signature,args)
                .ok_or(Error::WrongTypes{operation:op.info().name})?};
            if expected!=return_ty{return Err(Error::WrongTypes{operation:op.info().name})}
        } else {
            // Even opaque texture/atomic signatures must carry the right leading resource.
            let valid=match op.info().family {
                Family::Texture => args.first().is_some_and(|a| {
                    if op==Intrinsic::TextureStore {a.ty==Type::StorageTexture2D}
                    else {a.ty.is_texture()}
                }),
                Family::Atomic => args.first().is_some_and(|a|a.ty.is_atomic()),
                _=>true,
            };
            if !valid{return Err(Error::WrongTypes{operation:op.info().name})}
        }
        self.render(op,args,symbol,return_ty)
    }
    /// Lower validated operations to native-call or infix syntax.
    fn render(&self,op:Intrinsic,args:&[Expr],symbol:&str,ty:Type)->Result<Expr,Error>{
        if self.backend==Backend::NvidiaCuda {
            return crate::nvidia::render(op,args,ty,self.backend);
        }
        if self.backend==Backend::AmdHip {
            return crate::amd::render(op,args,ty);
        }
        if self.backend==Backend::IntelOpenCl {
            // OpenCL C vector comparisons produce integer masks, unlike WGSL bool vectors.
            if args.first().is_some_and(|v|v.ty.is_vector())
                && matches!(op,Intrinsic::Equal|Intrinsic::NotEqual|Intrinsic::Less|Intrinsic::LessEqual|Intrinsic::Greater|Intrinsic::GreaterEqual){
                return Err(Error::WrongTypes{operation:op.info().name});
            }
            // OpenCL-C stores vectors in native scalar/vector types, but no matrix/vector-bool builtins.
            if matches!(ty,Type::Matrix(..)|Type::Vector(Scalar::Bool,_)) {
                return Err(Error::WrongTypes{operation:op.info().name});
            }
            if op==Intrinsic::Sign && !args[0].ty.is_float(){
                return Err(Error::WrongTypes{operation:op.info().name});
            }
            if op==Intrinsic::Abs && args[0].ty.is_integer(){
                let call=format!("abs({})",args[0].code);
                return Ok(Expr{code:call,ty});
            }
            if matches!(op,Intrinsic::WorkgroupBarrier|Intrinsic::StorageBarrier) {
                let fence=if op==Intrinsic::WorkgroupBarrier{"CLK_LOCAL_MEM_FENCE"}else{"CLK_GLOBAL_MEM_FENCE"};
                return Ok(Expr{code:format!("barrier({})",fence),ty:Type::Void});
            }
            // Validate native subgroup shuffle overloads conservatively.
            if matches!(op,Intrinsic::SubgroupShuffle|Intrinsic::SubgroupShuffleXor|
                Intrinsic::SubgroupShuffleUp|Intrinsic::SubgroupShuffleDown){
                let data=args.first().ok_or(Error::WrongTypes{operation:op.info().name})?;
                if !(data.ty.is_numeric() && data.ty==ty){return Err(Error::WrongTypes{operation:op.info().name});}
                if op==Intrinsic::SubgroupShuffleUp||op==Intrinsic::SubgroupShuffleDown {
                    if args.len()!=3 || args[1].ty!=data.ty || args[2].ty!=Type::U32{
                        return Err(Error::WrongTypes{operation:op.info().name});
                    }
                }else if args.len()!=2 || args[1].ty!=Type::U32{
                    return Err(Error::WrongTypes{operation:op.info().name});
                }
            }
            let name=op.info().name;
            if name.starts_with("intelSubGroupBlock") {
                let read=name.contains("Read");
                let width=if name.ends_with('4'){4}else if name.ends_with('2'){2}else{1};
                let element=if width==1 {Type::U32}else{Type::Vector(Scalar::U32,width)};
                if name.ends_with('8') || name.contains("Us") {
                    return Err(Error::Unsupported{operation:name,backend:self.backend});
                }
                if args.first().map(|a|a.ty)!=Some(Type::U32) {
                    return Err(Error::WrongTypes{operation:name});
                }
                if read {
                    if ty!=element{return Err(Error::WrongTypes{operation:name});}
                } else if ty!=Type::Void || args.len()!=2 || args[1].ty!=element {
                    return Err(Error::WrongTypes{operation:name});
                }
            }
        }
        let infix=match op{
            Intrinsic::Add=>Some("+"),Intrinsic::Sub=>Some("-"),
            Intrinsic::Multiply=>Some("*"),Intrinsic::Divide=>Some("/"),
            Intrinsic::Modulo=>Some("%"),
            Intrinsic::Equal=>Some("=="), Intrinsic::NotEqual=>Some("!="),
            Intrinsic::Less=>Some("<"),Intrinsic::LessEqual=>Some("<="),
            Intrinsic::Greater=>Some(">"),Intrinsic::GreaterEqual=>Some(">="),
            Intrinsic::LogicalAnd=>Some("&&"),Intrinsic::LogicalOr=>Some("||"),
            Intrinsic::BitAnd=>Some("&"),Intrinsic::BitOr=>Some("|"),
            Intrinsic::BitXor=>Some("^"),Intrinsic::ShiftLeft=>Some("<<"),
            Intrinsic::ShiftRight=>Some(">>"), _=>None,
        };
        let code=if let Some(operator)=infix{
            // GLSL's vector comparisons require equal/lessThan rather than an infix operator.
            if self.backend==Backend::Glsl && args[0].ty.is_vector() && matches!(op,
                Intrinsic::Equal|Intrinsic::NotEqual|Intrinsic::Less|Intrinsic::LessEqual|Intrinsic::Greater|Intrinsic::GreaterEqual) {
                let function=match op {
                    Intrinsic::Equal=>"equal",Intrinsic::NotEqual=>"notEqual",
                    Intrinsic::Less=>"lessThan",Intrinsic::LessEqual=>"lessThanEqual",
                    Intrinsic::Greater=>"greaterThan",_=>"greaterThanEqual",
                };
                format!("{}({}, {})",function,args[0].code,args[1].code)
            }else if op==Intrinsic::Modulo && args[0].ty.is_float() && self.backend==Backend::Glsl{
                format!("mod({}, {})",args[0].code,args[1].code)
            }else if op==Intrinsic::Modulo && args[0].ty.is_float() && self.backend==Backend::Hlsl{
                format!("fmod({}, {})",args[0].code,args[1].code)
            }else{format!("({} {} {})",args[0].code,operator,args[1].code)}
        }else if op==Intrinsic::Negate||op==Intrinsic::LogicalNot||op==Intrinsic::BitNot {
            let op=if op==Intrinsic::Negate{"-"}else if op==Intrinsic::LogicalNot{"!"}else{"~"};
            format!("({}{})",op,args[0].code)
        }else if op==Intrinsic::Select && self.backend==Backend::Hlsl{
            return Err(Error::Unsupported{operation:op.info().name,backend:self.backend})
        }else if matches!(op,Intrinsic::RotateLeft|Intrinsic::RotateRight){
            let x=&args[0].code;let n=&args[1].code;
            // Only unsigned integer inputs are supported by the synthetic rotate.
            if args[0].ty!=Type::U32||args[1].ty!=Type::U32{return Err(Error::WrongTypes{operation:op.info().name})}
            if op==Intrinsic::RotateLeft {format!("(({} << ({} & 31u)) | ({} >> ((32u - ({} & 31u)) & 31u)))",x,n,x,n)}
            else {format!("(({} >> ({} & 31u)) | ({} << ((32u - ({} & 31u)) & 31u)))",x,n,x,n)}
        }else{
            let args_joined=args.iter().map(|a|a.code.as_str()).collect::<Vec<_>>().join(", ");
            format!("{}({})",symbol,args_joined)
        };
        Ok(Expr{code,ty})
    }
    /// Produce a scalar/vector/matrix constructor expression in target syntax.
    /// The shader compiler ultimately checks constructor overloads.
    pub fn construct(&self,ty:Type,args:&[Expr])->Result<Expr,Error>{
        if !ty.valid() || ty==Type::Void {return Err(Error::InvalidType)}
        if matches!(self.backend,Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip) && matches!(ty,Type::Matrix(..)|Type::Vector(Scalar::Bool,_)){
            return Err(Error::InvalidType);
        }
        if !(matches!(ty,Type::Scalar(_) | Type::Vector(_,_) | Type::Matrix(_,_))) {return Err(Error::InvalidType)}
        let list=args.iter().map(|e|e.code.as_str()).collect::<Vec<_>>().join(", ");
        let code=if self.backend==Backend::IntelOpenCl {
            format!("({})({})",ty.source(self.backend),list)
        }else if matches!(self.backend,Backend::NvidiaCuda|Backend::AmdHip) && matches!(ty,Type::Vector(..)){
            format!("make_{}({})",ty.source(self.backend),list)
        }else{
            format!("{}({})",ty.source(self.backend),list)
        };
        Ok(Expr{code,ty})
    }
}

/// Infer the shader result type only for well-defined, backend-neutral cases.
fn infer_type(sig:Signature, args:&[Expr])->Option<Type>{
    use Signature::*;
    if sig==Signature::Barrier && args.is_empty() {return Some(Type::Void)}
    let first=args.first()?.ty;
    let same=args.iter().all(|e|e.ty==first);
    let all_float=same&&first.is_float();
    let all_numeric=same&&first.is_numeric();
    let all_integer=same&&first.is_integer();
    match sig {
        UnaryFloat if args.len()==1&&first.is_float()=>Some(first),
        UnaryNumber if args.len()==1&&first.is_numeric()=>Some(first),
        UnaryInteger if args.len()==1&&first.is_integer()=>Some(first),
        BinaryFloat if args.len()==2&&all_float=>Some(first),
        BinaryNumber if args.len()==2&&all_numeric=>Some(first),
        BinaryInteger if args.len()==2&&all_integer=>Some(first),
        TernaryFloat if args.len()==3&&all_float=>Some(first),
        TernaryNumber if args.len()==3&&all_numeric=>Some(first),
        UnaryFloatVector if args.len()==1&&first.is_float()&&first.is_vector()=>Some(first),
        BinaryFloatVector if args.len()==2&&all_float&&first.is_vector()=>Some(first),
        TernaryFloatVector if args.len()==3&&all_float&&first.is_vector()=>Some(first),
        Length if args.len()==1&&first.is_float()&&first.is_vector()=>Some(Type::F32),
        Distance if args.len()==2&&all_float&&first.is_vector()=>Some(Type::F32),
        Dot if args.len()==2&&all_float&&first.is_vector()=>Some(Type::F32),
        Cross if args.len()==2&&all_float&&first==Type::VEC3=>Some(first),
        Refract if args.len()==3&&first.is_float()&&first.is_vector()
            &&args[1].ty==first&&args[2].ty==Type::F32=>Some(first),
        Matrix if args.len()==1&&matches!(first,Type::Matrix(_, _))=>Some(first),
        MatrixScalar if args.len()==1&&matches!(first,Type::Matrix(n,m) if n==m)=>Some(Type::F32),
        Equality if args.len()==2&&same=>Some(match first {Type::Vector(_,n)=>Type::Vector(Scalar::Bool,n),_=>Type::BOOL}),
        Comparison if args.len()==2&&all_numeric=>Some(match first {Type::Vector(_,n)=>Type::Vector(Scalar::Bool,n),_=>Type::BOOL}),
        BoolBinary if args.len()==2&&same&&first==Type::BOOL=>Some(Type::BOOL),
        BoolUnary if args.len()==1&&first==Type::BOOL=>Some(Type::BOOL),
        ReductionBool if args.len()==1&&first.is_bool()&&first.is_vector()=>Some(Type::BOOL),
        Pack if args.len()==1&&first.is_float()&&first.is_vector()=>Some(Type::U32),
        Unpack if args.len()==1&&first==Type::U32=>Some(Type::VEC4),
        Barrier if args.is_empty()=>Some(Type::Void),
        _=>None,
    }
}

/// Build a single shader helper function (not a complete entry-point module).
pub struct FunctionBuilder {
    emitter: Emitter,
    name:String,
    parameters:Vec<(String,Type)>,
    body:Vec<String>,
    returns:Type,
    returned:bool,
    locals:Vec<String>,
}
impl FunctionBuilder {
    /// Begin a shader helper-function declaration.
    pub fn new(emitter:Emitter,name:&str,returns:Type)->Result<Self,Error>{
        if !is_identifier(name){return Err(Error::InvalidIdentifier)}
        if !returns.valid(){return Err(Error::InvalidType)}
        Ok(Self{emitter,name:name.to_string(),parameters:Vec::new(),body:Vec::new(),returns,returned:false,locals:Vec::new()})
    }
    /// Append a typed parameter; duplicate identifiers are rejected.
    pub fn parameter(&mut self,name:&str,ty:Type)->Result<Expr,Error>{
        let expression=Expr::variable(name,ty)?;
        if self.parameters.iter().any(|(s,_)|s==name)||self.locals.iter().any(|s|s==name){return Err(Error::DuplicateParameter)}
        self.parameters.push((name.to_string(),ty));Ok(expression)
    }
    /// Declare an immutable local expression (WGSL `let`, HLSL/GLSL initialized variable).
    pub fn let_value(&mut self,name:&str,expression:&Expr)->Result<Expr,Error>{
        if !is_identifier(name){return Err(Error::InvalidIdentifier)}
        if self.parameters.iter().any(|(s,_)|s==name)||self.locals.iter().any(|s|s==name) {return Err(Error::DuplicateParameter)}
        let lhs=match self.emitter.backend{
            Backend::Wgsl=>format!("let {}: {} = {}",name,expression.ty.source(self.emitter.backend),expression.code),
            _=>format!("{} {} = {}",expression.ty.source(self.emitter.backend),name,expression.code),
        };
        self.body.push(format!("    {};",lhs));
        self.locals.push(name.to_string());
        Expr::variable(name,expression.ty)
    }
    /// Append one expression statement, including void-valued barrier/atomic calls.
    pub fn statement(&mut self,expression:&Expr){self.body.push(format!("    {};",expression.code));}
    /// Append a typed return statement.
    pub fn return_expr(&mut self,expression:&Expr)->Result<(),Error>{
        if self.returns!=expression.ty{return Err(Error::FunctionReturnTypeMismatch)}
        self.body.push(format!("    return {};",expression.code));self.returned=true;Ok(())
    }
    /// Append the target language's fragment-stage `discard` statement.
    /// This is a statement rather than a call expression in shader languages.
    pub fn discard(&mut self)->Result<(),Error>{
        if self.emitter.stage!=Stage::Fragment {
            return Err(Error::WrongStage{operation:"discard",required:Stage::Fragment});
        }
        self.body.push("    discard;".to_string());
        Ok(())
    }
    /// Serialize the helper-function declaration to shader source.
    pub fn build(self)->Result<String,Error>{
        if !self.returned && self.returns!=Type::Void{return Err(Error::FunctionHasNoReturn)}
        let signature=self.parameters.iter().map(|(name,ty)|match self.emitter.backend{
            Backend::Wgsl=>format!("{}: {}",name,ty.source(self.emitter.backend)),
            _=>format!("{} {}",ty.source(self.emitter.backend),name),
        }).collect::<Vec<_>>().join(", ");
        let header=match self.emitter.backend{
            Backend::Wgsl => if self.returns==Type::Void {format!("fn {}({}) {{",self.name,signature)}
            else {format!("fn {}({}) -> {} {{",self.name,signature,self.returns.source(self.emitter.backend))},
            _=>format!("{} {}({}) {{",self.returns.source(self.emitter.backend),self.name,signature),
        };
        Ok(format!("{}\n{}\n}}\n",header,self.body.join("\n")))
    }
}
