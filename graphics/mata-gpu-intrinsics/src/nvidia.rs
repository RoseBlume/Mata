//! NVIDIA CUDA C++ device-source target: **no host CUDA API or compiler linkage**.
//!
//! `KernelBuilder` creates CUDA C++ source; a caller must compile it with NVRTC
//! or nvcc and launch with a CUDA runtime/driver **at runtime**. This module
//! never detects an NVIDIA GPU or invokes a driver while building the Rust crate.
//!
//! Warp intrinsics require a participating-lane mask: never assume `0xffffffff`
//! is safe after branching, and avoid warp/CTA barriers in divergent paths.
//! The `NvidiaSm80` capability is a *declaration*, not GPU feature discovery.
use alloc::{format, string::{String,ToString}, vec::Vec};
use crate::{Backend,Capability,CapabilitySet,Emitter,Error,Expr,Intrinsic,Scalar,Stage,Type,is_identifier};

/// NVIDIA CUDA-C++ mapping for operations with compatible semantics and signatures.
///
/// A missing mapping is deliberate: CUDA float vectors are structs (unlike
/// shader SIMD values), and texture/sampler or generic subgroup intrinsics do
/// not share a universally equivalent CUDA expression form.
pub fn spelling(op:Intrinsic)->Option<&'static str>{
    use Intrinsic::*;
    Some(match op {
        Sin=>"sinf",Cos=>"cosf",Tan=>"tanf",Asin=>"asinf",Acos=>"acosf",Atan=>"atanf",
        Sinh=>"sinhf",Cosh=>"coshf",Tanh=>"tanhf",Asinh=>"asinhf",Acosh=>"acoshf",Atanh=>"atanhf",
        Exp=>"expf",Exp2=>"exp2f",Log=>"logf",Log2=>"log2f",Sqrt=>"sqrtf",
        InverseSqrt=>"rsqrtf",Floor=>"floorf",Ceil=>"ceilf",Trunc=>"truncf",Round=>"roundf",
        Pow=>"powf",Atan2=>"atan2f",Fma=>"fmaf",Abs=>"fabsf",
        Min=>"fminf",Max=>"fmaxf",Clamp=>"fminf",Saturate=>"fminf",
        Mix=>"lerp",Step=>"step",Smoothstep=>"smoothstep",Fract=>"fract",
        Degrees=>"degrees",Radians=>"radians",Sign=>"sign",
        Add=>"+",Sub=>"-",Multiply=>"*",Divide=>"/",Modulo=>"%",Negate=>"-",
        Equal=>"==",NotEqual=>"!=",Less=>"<",LessEqual=>"<=",Greater=>">",GreaterEqual=>">=",
        LogicalAnd=>"&&",LogicalOr=>"||",LogicalNot=>"!",BitAnd=>"&",BitOr=>"|",
        BitXor=>"^",BitNot=>"~",ShiftLeft=>"<<",ShiftRight=>">>",Select=>"?:",
        RotateLeft=>"rotl",RotateRight=>"rotr",
        CountOneBits=>"__popc",ReverseBits|BitfieldReverse=>"__brev",
        AtomicAdd=>"atomicAdd",AtomicMin=>"atomicMin",AtomicMax=>"atomicMax",
        AtomicAnd=>"atomicAnd",AtomicOr=>"atomicOr",AtomicXor=>"atomicXor",
        AtomicExchange=>"atomicExch",AtomicCompSwap=>"atomicCAS",
        WorkgroupBarrier=>"__syncthreads",
        NvidiaActiveMask=>"__activemask",
        NvidiaBallotSync=>"__ballot_sync",NvidiaAnySync=>"__any_sync",NvidiaAllSync=>"__all_sync",
        NvidiaShuffleSync=>"__shfl_sync",NvidiaShuffleUpSync=>"__shfl_up_sync",
        NvidiaShuffleDownSync=>"__shfl_down_sync",NvidiaShuffleXorSync=>"__shfl_xor_sync",
        NvidiaSyncWarp=>"__syncwarp",NvidiaMatchAnySync=>"__match_any_sync",
        NvidiaReduceAddSync=>"__reduce_add_sync",NvidiaReduceMinSync=>"__reduce_min_sync",
        NvidiaReduceMaxSync=>"__reduce_max_sync",NvidiaReduceAndSync=>"__reduce_and_sync",
        NvidiaReduceOrSync=>"__reduce_or_sync",NvidiaReduceXorSync=>"__reduce_xor_sync",
        _=>return None,
    })
}
fn bad(op:Intrinsic)->Error{Error::WrongTypes{operation:op.info().name}}
fn unsupported(op:Intrinsic)->Error{Error::Unsupported{operation:op.info().name,backend:Backend::NvidiaCuda}}
fn integer(t:Type)->bool{matches!(t,Type::I32|Type::U32)}
fn scalar(t:Type)->bool{matches!(t,Type::F32|Type::I32|Type::U32|Type::BOOL)}
fn all(args:&[Expr],t:Type)->bool{args.iter().all(|e|e.ty()==t)}
fn code(args:&[Expr])->String{args.iter().map(|x|x.code()).collect::<Vec<_>>().join(", ")}

/// Lower a validated, typed operation to CUDA C++ source.
///
/// Unsupported overloads return an error rather than assuming CUDA vector
/// math, C++ template overloads, or warp signatures match WGSL/OpenCL.
pub(crate) fn render(op:Intrinsic,args:&[Expr],ty:Type,backend:Backend)->Result<Expr,Error>{
    use Intrinsic::*;
    debug_assert_eq!(backend,Backend::NvidiaCuda);
    let native=spelling(op).ok_or_else(||unsupported(op))?;
    let first=args.first().map(|e|e.ty());
    let source=match op {
        // Float math, no implicit float2/float3 operator support in CUDA C++.
        Sin|Cos|Tan|Asin|Acos|Atan|Sinh|Cosh|Tanh|Asinh|Acosh|Atanh|
        Exp|Exp2|Log|Log2|Sqrt|InverseSqrt|Floor|Ceil|Trunc|Round => {
            if args.len()!=1||first!=Some(Type::F32)||ty!=Type::F32{return Err(bad(op))}
            format!("{}({})",native,code(args))
        }
        Pow|Atan2=>{
            if args.len()!=2||!all(args,Type::F32)||ty!=Type::F32{return Err(bad(op))}
            format!("{}({})",native,code(args))
        }
        Fma=>{
            if args.len()!=3||!all(args,Type::F32)||ty!=Type::F32{return Err(bad(op))}
            format!("fmaf({})",code(args))
        }
        Abs=>{
            if args.len()!=1||first!=Some(ty)||!matches!(ty,Type::F32|Type::I32){return Err(bad(op))}
            format!("{}({})",if ty==Type::F32{"fabsf"}else{"abs"},code(args))
        }
        Sign=>{
            if first!=Some(Type::F32)||ty!=Type::F32{return Err(bad(op))}
            format!("((float)(({} > 0.0f) - ({} < 0.0f)))",args[0].code(),args[0].code())
        }
        Min|Max=>{
            if args.len()!=2||!scalar(ty)||ty==Type::BOOL||!all(args,ty){return Err(bad(op))}
            let f=match (op,ty){(Min,Type::F32)=>"fminf",(Max,Type::F32)=>"fmaxf",(Min,_)=>"min",_=>"max"};
            format!("{}({})",f,code(args))
        }
        Clamp=>{
            if args.len()!=3||!scalar(ty)||ty==Type::BOOL||!all(args,ty){return Err(bad(op))}
            let (mi,ma)=if ty==Type::F32{("fminf","fmaxf")}else{("min","max")};
            format!("{}({}({}, {}), {})",mi,ma,args[0].code(),args[1].code(),args[2].code())
        }
        Saturate=>{
            if first!=Some(Type::F32)||ty!=Type::F32{return Err(bad(op))}
            format!("fminf(fmaxf({}, 0.0f), 1.0f)",args[0].code())
        }
        Mix=>{
            if args.len()!=3||!all(args,Type::F32)||ty!=Type::F32{return Err(bad(op))}
            format!("fmaf(({} - {}), {}, {})",args[1].code(),args[0].code(),args[2].code(),args[0].code())
        }
        Step=>{
            if args.len()!=2||!all(args,Type::F32)||ty!=Type::F32{return Err(bad(op))}
            format!("(({} < {}) ? 0.0f : 1.0f)",args[1].code(),args[0].code())
        }
        Smoothstep=>{
            if args.len()!=3||!all(args,Type::F32)||ty!=Type::F32{return Err(bad(op))}
            let t=format!("fminf(fmaxf(({} - {}) / ({} - {}), 0.0f), 1.0f)",args[2].code(),args[0].code(),args[1].code(),args[0].code());
            format!("(({t}) * ({t}) * (3.0f - 2.0f * ({t})))")
        }
        Fract=>{
            if first!=Some(Type::F32)||ty!=Type::F32{return Err(bad(op))}
            format!("({} - floorf({}))",args[0].code(),args[0].code())
        }
        Degrees|Radians=>{
            if first!=Some(Type::F32)||ty!=Type::F32{return Err(bad(op))}
            let f=if op==Degrees{"57.29577951308232f"}else{"0.017453292519943295f"};
            format!("({} * {})",args[0].code(),f)
        }
        Add|Sub|Multiply|Divide|Modulo|BitAnd|BitOr|BitXor|ShiftLeft|ShiftRight|
        Equal|NotEqual|Less|LessEqual|Greater|GreaterEqual|LogicalAnd|LogicalOr=>{
            if args.len()!=2||!scalar(args[0].ty())||args[0].ty()!=args[1].ty(){return Err(bad(op))}
            let a=args[0].ty();
            let cmp=matches!(op,Equal|NotEqual|Less|LessEqual|Greater|GreaterEqual|LogicalAnd|LogicalOr);
            if cmp {if ty!=Type::BOOL{return Err(bad(op))}}else if ty!=a{return Err(bad(op))}
            if matches!(op,BitAnd|BitOr|BitXor|ShiftLeft|ShiftRight|Modulo) && !integer(a){return Err(bad(op))}
            if matches!(op,LogicalAnd|LogicalOr) && a!=Type::BOOL {return Err(bad(op))}
            if !cmp && a==Type::BOOL{return Err(bad(op))}
            format!("({} {} {})",args[0].code(),native,args[1].code())
        }
        Negate|BitNot|LogicalNot=>{
            if args.len()!=1||first!=Some(ty){return Err(bad(op))}
            if matches!(op,Negate) && !(ty==Type::F32||ty==Type::I32){return Err(bad(op))}
            if matches!(op,BitNot)&&!integer(ty){return Err(bad(op))}
            if matches!(op,LogicalNot)&&ty!=Type::BOOL{return Err(bad(op))}
            format!("({}{})",native,args[0].code())
        }
        Select=>{
            if args.len()!=3||args[0].ty()!=args[1].ty()||args[2].ty()!=Type::BOOL||ty!=args[0].ty()||!scalar(ty){return Err(bad(op))}
            format!("({} ? {} : {})",args[2].code(),args[1].code(),args[0].code())
        }
        RotateLeft|RotateRight=>{
            if args.len()!=2||!all(args,Type::U32)||ty!=Type::U32{return Err(bad(op))}
            let x=args[0].code();let n=args[1].code();
            if op==RotateLeft {format!("(({} << ({} & 31u)) | ({} >> ((32u - ({} & 31u)) & 31u)))",x,n,x,n)}
            else {format!("(({} >> ({} & 31u)) | ({} << ((32u - ({} & 31u)) & 31u)))",x,n,x,n)}
        }
        CountOneBits|ReverseBits|BitfieldReverse=>{
            if first!=Some(Type::U32)||ty!=Type::U32{return Err(bad(op))}
            format!("{}({})",native,code(args))
        }
        AtomicAdd|AtomicMin|AtomicMax|AtomicAnd|AtomicOr|AtomicXor|AtomicExchange|AtomicCompSwap=>{
            let base=match first{Some(Type::Atomic(s)) if matches!(s,Scalar::I32|Scalar::U32)=>Type::Scalar(s),_=>return Err(bad(op))};
            let needed=if op==AtomicCompSwap{3}else{2};
            if args.len()!=needed||ty!=base||args[1..].iter().any(|a|a.ty()!=base){return Err(bad(op))}
            format!("{}({})",native,code(args))
        }
        WorkgroupBarrier=>{
            if !args.is_empty()||ty!=Type::Void{return Err(bad(op))}
            "__syncthreads()".into()
        }
        NvidiaActiveMask=>{
            if !args.is_empty()||ty!=Type::U32{return Err(bad(op))}
            "__activemask()".into()
        }
        NvidiaBallotSync|NvidiaAnySync|NvidiaAllSync|NvidiaMatchAnySync=>{
            if args.len()!=2||args[0].ty()!=Type::U32{return Err(bad(op))}
            if op==NvidiaMatchAnySync {
                if !integer(args[1].ty())||ty!=Type::U32{return Err(bad(op))}
            }else if !matches!(args[1].ty(),Type::BOOL|Type::I32){return Err(bad(op))}
            else if ty != (if op==NvidiaBallotSync{Type::U32}else{Type::I32}){return Err(bad(op))}
            format!("{}({})",native,code(args))
        }
        NvidiaShuffleSync|NvidiaShuffleUpSync|NvidiaShuffleDownSync|NvidiaShuffleXorSync=>{
            if !(3..=4).contains(&args.len())||args[0].ty()!=Type::U32||
               !matches!(args[1].ty(),Type::F32|Type::I32|Type::U32)||ty!=args[1].ty()||
               !integer(args[2].ty())|| (args.len()==4 && !integer(args[3].ty())){return Err(bad(op))}
            format!("{}({})",native,code(args))
        }
        NvidiaSyncWarp=>{
            if args.len()!=1||args[0].ty()!=Type::U32||ty!=Type::Void{return Err(bad(op))}
            format!("__syncwarp({})",code(args))
        }
        NvidiaReduceAddSync|NvidiaReduceMinSync|NvidiaReduceMaxSync|
        NvidiaReduceAndSync|NvidiaReduceOrSync|NvidiaReduceXorSync=>{
            if args.len()!=2||!all(args,Type::U32)||ty!=Type::U32{return Err(bad(op))}
            format!("{}({})",native,code(args))
        }
        _=>return Err(unsupported(op)),
    };
    Ok(Expr::from_source_unchecked(source,ty))
}

/// Access permissions for a CUDA global-memory buffer parameter.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum BufferAccess{
    /// A pointer declared `const T* __restrict__`.
    ReadOnly,
    /// A pointer declared `T* __restrict__`.
    ReadWrite,
}

/// Typed expression handle for a CUDA device-buffer parameter.
#[derive(Clone,Debug)]
pub struct Buffer {name:String,element:Type,access:BufferAccess}
impl Buffer {
    /// Name of the C++ kernel parameter.
    pub fn name(&self)->&str {&self.name}
    /// Element type of the device allocation.
    pub const fn element_type(&self)->Type {self.element}
    /// Whether the kernel can write this buffer through the handle.
    pub const fn access(&self)->BufferAccess {self.access}
    /// Generate a typed indexed load, without accessing host/GPU memory.
    pub fn load(&self,index:&Expr)->Result<Expr,Error>{
        if !integer(index.ty()){return Err(Error::InvalidType)}
        Ok(Expr::from_source_unchecked(format!("{}[{}]",self.name,index.code()),self.element))
    }
    /// Generate an atomic-pointer expression suitable for CUDA's `atomicAdd` etc.
    ///
    /// Caller must ensure alignment, valid pointer scope and device support.
    pub fn atomic_address_at(&self,index:&Expr)->Result<Expr,Error>{
        if !integer(index.ty())||self.access!=BufferAccess::ReadWrite{return Err(Error::InvalidType)}
        let Type::Scalar(s)=self.element else{return Err(Error::InvalidType)};
        if !matches!(s,Scalar::I32|Scalar::U32){return Err(Error::InvalidType)}
        Ok(Expr::from_source_unchecked(format!("({} + {})",self.name,index.code()),Type::Atomic(s)))
    }
}

/// Build CUDA C++ `__global__` kernel **source**, without linking CUDA.
///
/// Resulting source can be compiled by NVRTC/nvcc at runtime on a supported
/// NVIDIA CUDA device; this crate does not manage allocations or launches.
pub struct KernelBuilder {
    name:String,
    caps:CapabilitySet,
    params:Vec<String>,
    names:Vec<String>,
    statements:Vec<String>,
}
impl KernelBuilder {
    /// Create a CUDA kernel declaration using a valid unqualified identifier.
    pub fn new(name:&str,caps:CapabilitySet)->Result<Self,Error>{
        if !is_identifier(name){return Err(Error::InvalidIdentifier)}
        Ok(Self{name:name.into(),caps,params:Vec::new(),names:Vec::new(),statements:Vec::new()})
    }
    /// Obtain a matching expression emitter.
    pub const fn emitter(&self)->Emitter {Emitter::new(Backend::NvidiaCuda,Stage::Compute,self.caps)}
    fn register(&mut self,name:&str)->Result<(),Error>{
        if !is_identifier(name){return Err(Error::InvalidIdentifier)}
        if self.names.iter().any(|s|s==name){return Err(Error::DuplicateParameter)}
        self.names.push(name.to_string());Ok(())
    }
    fn supported_element(t:Type)->bool{matches!(t,Type::Scalar(Scalar::F32|Scalar::I32|Scalar::U32))}
    /// Add read-only global buffer argument.
    pub fn read_only_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>{self.buffer(name,element,BufferAccess::ReadOnly)}
    /// Add writable global buffer argument.
    pub fn read_write_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>{self.buffer(name,element,BufferAccess::ReadWrite)}
    fn buffer(&mut self,name:&str,element:Type,access:BufferAccess)->Result<Buffer,Error>{
        if !Self::supported_element(element){return Err(Error::InvalidType)}
        self.register(name)?;
        let t=element.source(Backend::NvidiaCuda);
        let qualifier=if access==BufferAccess::ReadOnly{"const "}else{""};
        self.params.push(format!("{}{}* __restrict__ {}",qualifier,t,name));
        Ok(Buffer{name:name.into(),element,access})
    }
    /// Add a scalar by-value kernel argument.
    pub fn value(&mut self,name:&str,ty:Type)->Result<Expr,Error>{
        if !Self::supported_element(ty){return Err(Error::InvalidType)}
        self.register(name)?;
        self.params.push(format!("{} {}",ty.source(Backend::NvidiaCuda),name));
        Expr::variable(name,ty)
    }
    /// Declare a typed local variable from an expression.
    pub fn let_value(&mut self,name:&str,expr:&Expr)->Result<Expr,Error>{
        if !Self::supported_element(expr.ty()){return Err(Error::InvalidType)}
        self.register(name)?;
        self.statements.push(format!("    {} {} = {};",expr.ty().source(Backend::NvidiaCuda),name,expr.code()));
        Expr::variable(name,expr.ty())
    }
    /// Generate 1D global index `blockIdx.x * blockDim.x + threadIdx.x`.
    ///
    /// Arithmetic uses 32-bit unsigned integers; launches must avoid overflow.
    pub fn index_1d(&mut self,name:&str)->Result<Expr,Error>{
        self.register(name)?;
        self.statements.push(format!("    unsigned int {} = blockIdx.x * blockDim.x + threadIdx.x;",name));
        Expr::variable(name,Type::U32)
    }
    /// Add an indexed global-memory store, validating access and element type.
    pub fn store(&mut self,buffer:&Buffer,index:&Expr,value:&Expr)->Result<(),Error>{
        if buffer.access!=BufferAccess::ReadWrite||!integer(index.ty())||buffer.element!=value.ty(){return Err(Error::InvalidType)}
        self.statements.push(format!("    {}[{}] = {};",buffer.name,index.code(),value.code()));
        Ok(())
    }
    /// Emit a statement using the source of a previously validated expression.
    pub fn statement(&mut self,expression:&Expr){self.statements.push(format!("    {};",expression.code()));}
    /// Return from a kernel if `index >= limit` (uniformity must be considered
    /// by the caller before inserting workgroup or warp barriers).
    pub fn return_if_out_of_bounds(&mut self,index:&Expr,limit:&Expr)->Result<(),Error>{
        if index.ty()!=Type::U32||limit.ty()!=Type::U32{return Err(Error::InvalidType)}
        self.statements.push(format!("    if ({} >= {}) return;",index.code(),limit.code()));Ok(())
    }
    /// Return a fully formed CUDA C++ `extern "C" __global__` function definition.
    ///
    /// This text does not include CUDA runtime registration code, device memory
    /// allocation, an NVRTC compiler call, or a launch configuration.
    pub fn build(self)->Result<String,Error>{
        if self.caps.contains(Capability::NvidiaSm80)&&!self.caps.contains(Capability::NvidiaWarp){
            return Err(Error::FeatureDisabled(Capability::NvidiaWarp));
        }
        let mut out=format!("// CUDA C++ device code; compile with NVRTC or nvcc on the target runtime.\nextern \"C\" __global__ void {}({}) {{\n",self.name,self.params.join(", "));
        for line in self.statements{out.push_str(&line);out.push('\n');}
        out.push_str("}\n");Ok(out)
    }
}
