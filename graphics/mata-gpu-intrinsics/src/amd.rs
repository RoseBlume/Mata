//! AMD ROCm HIP C++ device source, generated entirely on the host.
//!
//! AMD HIP kernels can be compiled later with hipcc or hiprtc; building this
//! Rust crate never links ROCm and never asks the machine for an AMD GPU.
//!
//! The `AmdWave` capability permits wavefront operations, whose widths may be
//! **32 or 64 lanes** depending on the device and compilation target. The
//! `AmdGcn` capability additionally permits AMDGCN-only compiler builtins.
//! Both capabilities are *declarations*, not driver capability discovery.
//!
//! Cross-lane `readlane` and `writelane` require a uniform lane selector;
//! permutation source lanes must be active. Divergent use of synchronization
//! primitives is unsafe and requires the caller to enforce convergence.
use alloc::{format,string::{String,ToString},vec::Vec};
use crate::{Backend,Capability,CapabilitySet,Emitter,Error,Expr,Intrinsic,Scalar,Stage,Type,is_identifier};

/// Native HIP name for a supported operation, including AMD extensions.
/// Returns `None` for operations without a proven HIP lowering.
///
/// Most scalar math and integer atomics are shared with the CUDA-compatible
/// HIP API. NVIDIA-only operations, textures, and unmodeled vector overloads
/// are excluded deliberately.
pub fn spelling(op:Intrinsic)->Option<&'static str>{
    use Intrinsic::*;
    let name=match op {
        AmdWaveSize=>"warpSize",AmdLaneId=>"__lane_id",
        AmdShuffle=>"__shfl",AmdShuffleXor=>"__shfl_xor",
        AmdShuffleUp=>"__shfl_up",AmdShuffleDown=>"__shfl_down",
        AmdBallot=>"__ballot",AmdActiveMask=>"__activemask",
        AmdAny=>"__any",AmdAll=>"__all",AmdWaveSync=>"__syncwarp",
        AmdReadFirstLane=>"__builtin_amdgcn_readfirstlane",
        AmdReadLane=>"__builtin_amdgcn_readlane",
        AmdWriteLane=>"__builtin_amdgcn_writelane",
        AmdDsBpermute=>"__builtin_amdgcn_ds_bpermute",
        AmdDsPermute=>"__builtin_amdgcn_ds_permute",
        AmdFastSin=>"__sinf",AmdFastCos=>"__cosf",
        AmdFastExp=>"__expf",AmdFastLog=>"__logf",
        AmdFastLog2=>"__log2f",AmdFastPow=>"__powf",
        AmdFastRcp=>"__frcp_rn",
        AmdBytePerm=>"__byte_perm",
        AmdFunnelShiftLeft=>"__funnelshift_l",
        AmdFunnelShiftRight=>"__funnelshift_r",
        AmdMul24=>"__mul24",AmdUMul24=>"__umul24",
        AmdSad=>"__sad",AmdUSad=>"__usad",
        AmdPopcount64=>"__popcll",AmdReverseBits64=>"__brevll",
        AmdThreadFenceBlock=>"__threadfence_block",
        AmdThreadFenceDevice=>"__threadfence",
        AmdThreadFenceSystem=>"__threadfence_system",
        _=>{
            // Only common HIP math, scalar ops, and atomics (not CUDA-only warp
            // operations or unsupported vector, texture and special signatures).
            return crate::nvidia::spelling(op).filter(|_|!op.info().name.starts_with("nvidia"));
        }
    };
    Some(name)
}
fn bad(op:Intrinsic)->Error{Error::WrongTypes{operation:op.info().name}}
fn unsupported(op:Intrinsic)->Error{Error::Unsupported{operation:op.info().name,backend:Backend::AmdHip}}
fn is_u32(e:&Expr)->bool{e.ty()==Type::U32}
fn numeric32(t:Type)->bool{matches!(t,Type::F32|Type::I32|Type::U32)}
fn same(args:&[Expr],ty:Type)->bool{args.iter().all(|e|e.ty()==ty)}

/// Convert a typed intrinsic into HIP device-source expression text.
///
/// Per-operation overloads are checked before emitting text. Compiler target
/// flags, exact GPU architecture and runtime support must still be validated
/// by the HIP compiler when an application compiles the resulting kernel.
pub(crate) fn render(op:Intrinsic,args:&[Expr],ty:Type)->Result<Expr,Error>{
    use Intrinsic::*;
    let fn_name=spelling(op).ok_or_else(||unsupported(op))?;
    let src=match op {
        AmdWaveSize=>{
            if !args.is_empty()||ty!=Type::U32{return Err(bad(op))}
            "((unsigned int)warpSize)".into()
        },
        AmdLaneId=>{
            if !args.is_empty()||ty!=Type::U32{return Err(bad(op))}
            "((unsigned int)__lane_id())".into()
        },
        AmdActiveMask|AmdBallot=>{
            if ty!=Type::U64{return Err(bad(op))}
            if op==AmdActiveMask {
                if !args.is_empty(){return Err(bad(op))}
                "((unsigned long long)__activemask())".into()
            }else{
                if args.len()!=1||!matches!(args[0].ty(),Type::BOOL|Type::I32){return Err(bad(op))}
                format!("((unsigned long long)__ballot({}))",args[0].code())
            }
        },
        AmdShuffle|AmdShuffleXor|AmdShuffleUp|AmdShuffleDown=>{
            if args.len()!=2||!numeric32(args[0].ty())||!is_u32(&args[1])||ty!=args[0].ty(){return Err(bad(op))}
            format!("{}({}, (int)({}), warpSize)",fn_name,args[0].code(),args[1].code())
        },
        AmdAny|AmdAll=>{
            if args.len()!=1||!matches!(args[0].ty(),Type::BOOL|Type::I32)||ty!=Type::I32{return Err(bad(op))}
            format!("{}({})",fn_name,args[0].code())
        },
        AmdWaveSync=>{
            if !args.is_empty()||ty!=Type::Void{return Err(bad(op))}
            "__syncwarp()".into()
        },
        AmdReadFirstLane=>{
            if args.len()!=1||!is_u32(&args[0])||ty!=Type::U32{return Err(bad(op))}
            format!("{}({})",fn_name,args[0].code())
        },
        AmdReadLane|AmdDsBpermute|AmdDsPermute=>{
            if args.len()!=2||!same(args,Type::U32)||ty!=Type::U32{return Err(bad(op))}
            if op==AmdReadLane {
                format!("{}({}, {})",fn_name,args[0].code(),args[1].code())
            }else{
                // __builtin_amdgcn_ds_*permute takes a byte address, not lane id.
                format!("((unsigned int){}(((int)({})) * 4, (int)({})))",
                    fn_name,args[1].code(),args[0].code())
            }
        },
        AmdWriteLane=>{
            if args.len()!=3||!same(args,Type::U32)||ty!=Type::U32{return Err(bad(op))}
            format!("{}({}, {}, {})",fn_name,args[0].code(),args[1].code(),args[2].code())
        },
        AmdFastSin|AmdFastCos|AmdFastExp|AmdFastLog|AmdFastLog2|AmdFastRcp=>{
            if args.len()!=1||!same(args,Type::F32)||ty!=Type::F32{return Err(bad(op))}
            format!("{}({})",fn_name,args[0].code())
        },
        AmdFastPow=>{
            if args.len()!=2||!same(args,Type::F32)||ty!=Type::F32{return Err(bad(op))}
            format!("{}({}, {})",fn_name,args[0].code(),args[1].code())
        },
        AmdBytePerm|AmdFunnelShiftLeft|AmdFunnelShiftRight=>{
            if args.len()!=3||!same(args,Type::U32)||ty!=Type::U32{return Err(bad(op))}
            format!("{}({}, {}, {})",fn_name,args[0].code(),args[1].code(),args[2].code())
        },
        AmdMul24|AmdUMul24=>{
            let t=if op==AmdMul24{Type::I32}else{Type::U32};
            if args.len()!=2||!same(args,t)||ty!=t{return Err(bad(op))}
            format!("{}({}, {})",fn_name,args[0].code(),args[1].code())
        },
        AmdSad|AmdUSad=>{
            let t=if op==AmdSad{Type::I32}else{Type::U32};
            if args.len()!=3||!same(args,t)||ty!=t{return Err(bad(op))}
            format!("{}({}, {}, {})",fn_name,args[0].code(),args[1].code(),args[2].code())
        },
        AmdPopcount64=>{
            if args.len()!=1||args[0].ty()!=Type::U64||ty!=Type::U32{return Err(bad(op))}
            format!("{}({})",fn_name,args[0].code())
        },
        AmdReverseBits64=>{
            if args.len()!=1||args[0].ty()!=Type::U64||ty!=Type::U64{return Err(bad(op))}
            format!("{}({})",fn_name,args[0].code())
        },
        AmdThreadFenceBlock|AmdThreadFenceDevice|AmdThreadFenceSystem=>{
            if !args.is_empty()||ty!=Type::Void{return Err(bad(op))}
            format!("{}()",fn_name)
        },
        _=>{
            // Portable HIP subset shares CUDA-compatible scalar math / atomics.
            // Pass the backend required by its typed lowering and translate its
            // error's backend field; HIP's supported spellings are separately gated.
            return crate::nvidia::render(op,args,ty,Backend::NvidiaCuda).map_err(|e|match e {
                Error::Unsupported{operation,..}=>Error::Unsupported{operation,backend:Backend::AmdHip},
                rest=>rest,
            });
        }
    };
    Ok(Expr::from_source_unchecked(src,ty))
}

/// Access mode for an AMD HIP global buffer parameter.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum BufferAccess {
    /// Kernel reads from the buffer but does not write through this handle.
    ReadOnly,
    /// Kernel reads and writes through this handle.
    ReadWrite,
}
/// Typed source-only HIP global-memory buffer argument.
#[derive(Clone,Debug)]
pub struct Buffer {name:String,element:Type,access:BufferAccess}
impl Buffer {
    /// Get the emitted parameter identifier.
    pub fn name(&self)->&str{&self.name}
    /// Get the buffer's element type.
    pub const fn element_type(&self)->Type{self.element}
    /// Get whether the device kernel may write through this handle.
    pub const fn access(&self)->BufferAccess{self.access}
    /// Emit indexed load, without touching host or device memory.
    pub fn load(&self,index:&Expr)->Result<Expr,Error>{
        if !matches!(index.ty(),Type::I32|Type::U32){return Err(Error::InvalidType)}
        Ok(Expr::from_source_unchecked(format!("{}[{}]",self.name,index.code()),self.element))
    }
    /// Emit a writable integer-atomic address expression for HIP's atomic API.
    pub fn atomic_address_at(&self,index:&Expr)->Result<Expr,Error>{
        if !matches!(index.ty(),Type::I32|Type::U32)||self.access!=BufferAccess::ReadWrite{return Err(Error::InvalidType)}
        let Type::Scalar(s)=self.element else{return Err(Error::InvalidType)};
        if !matches!(s,Scalar::I32|Scalar::U32){return Err(Error::InvalidType)}
        Ok(Expr::from_source_unchecked(format!("({} + {})",self.name,index.code()),Type::Atomic(s)))
    }
}
/// Build a standalone HIP kernel source string without the ROCm SDK installed.
///
/// Kernel arguments are scalars and typed global buffers. The resulting C++
/// source must be compiled and launched by an application via HIP at runtime.
/// No compiler executable, driver or GPU is accessed by this builder.
pub struct KernelBuilder{
    name:String,caps:CapabilitySet,params:Vec<String>,names:Vec<String>,statements:Vec<String>,
}
impl KernelBuilder{
    /// Construct a HIP kernel with a valid unqualified name.
    pub fn new(name:&str,caps:CapabilitySet)->Result<Self,Error>{
        if !is_identifier(name){return Err(Error::InvalidIdentifier)}
        Ok(Self{name:name.into(),caps,params:Vec::new(),names:Vec::new(),statements:Vec::new()})
    }
    /// Return a configured compute-stage AMD HIP expression emitter.
    pub const fn emitter(&self)->Emitter{Emitter::new(Backend::AmdHip,Stage::Compute,self.caps)}
    fn register(&mut self,name:&str)->Result<(),Error>{
        if !is_identifier(name){return Err(Error::InvalidIdentifier)}
        if self.names.iter().any(|n|n==name){return Err(Error::DuplicateParameter)}
        self.names.push(name.to_string());Ok(())
    }
    fn supported_element(t:Type)->bool{matches!(t,Type::F32|Type::I32|Type::U32|Type::U64)}
    /// Declare a read-only pointer-to-device-buffer argument.
    pub fn read_only_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>{
        self.buffer(name,element,BufferAccess::ReadOnly)
    }
    /// Declare a writable pointer-to-device-buffer argument.
    pub fn read_write_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>{
        self.buffer(name,element,BufferAccess::ReadWrite)
    }
    fn buffer(&mut self,name:&str,element:Type,access:BufferAccess)->Result<Buffer,Error>{
        if !Self::supported_element(element){return Err(Error::InvalidType)}
        self.register(name)?;
        let qualifier=if access==BufferAccess::ReadOnly{"const "}else{""};
        self.params.push(format!("{}{}* __restrict__ {}",qualifier,element.source(Backend::AmdHip),name));
        Ok(Buffer{name:name.into(),element,access})
    }
    /// Declare scalar kernel parameter with the requested type.
    pub fn value(&mut self,name:&str,ty:Type)->Result<Expr,Error>{
        if !Self::supported_element(ty){return Err(Error::InvalidType)}
        self.register(name)?;
        self.params.push(format!("{} {}",ty.source(Backend::AmdHip),name));
        Expr::variable(name,ty)
    }
    /// Declare a local scalar with a validated typed initializer.
    pub fn let_value(&mut self,name:&str,expr:&Expr)->Result<Expr,Error>{
        if !Self::supported_element(expr.ty()){return Err(Error::InvalidType)}
        self.register(name)?;
        self.statements.push(format!("    {} {} = {};",expr.ty().source(Backend::AmdHip),name,expr.code()));
        Expr::variable(name,expr.ty())
    }
    /// Generate a 1D global thread index (`blockIdx.x * blockDim.x + threadIdx.x`).
    pub fn index_1d(&mut self,name:&str)->Result<Expr,Error>{
        self.register(name)?;
        self.statements.push(format!("    unsigned int {} = blockIdx.x * blockDim.x + threadIdx.x;",name));
        Expr::variable(name,Type::U32)
    }
    /// Emit indexed store after validating access permissions and element type.
    pub fn store(&mut self,buffer:&Buffer,index:&Expr,value:&Expr)->Result<(),Error>{
        if buffer.access!=BufferAccess::ReadWrite||!matches!(index.ty(),Type::I32|Type::U32)||buffer.element!=value.ty(){return Err(Error::InvalidType)}
        self.statements.push(format!("    {}[{}] = {};",buffer.name,index.code(),value.code()));Ok(())
    }
    /// Append a prevalidated expression as a HIP statement.
    pub fn statement(&mut self,expr:&Expr){self.statements.push(format!("    {};",expr.code()));}
    /// Emit early-return guard (`if index >= limit`) for bounds checking.
    ///
    /// Do not place group or wave barriers after divergent early returns.
    pub fn return_if_out_of_bounds(&mut self,index:&Expr,limit:&Expr)->Result<(),Error>{
        if index.ty()!=Type::U32||limit.ty()!=Type::U32{return Err(Error::InvalidType)}
        self.statements.push(format!("    if ({} >= {}) return;",index.code(),limit.code()));Ok(())
    }
    /// Return HIP C++ kernel source with the HIP runtime header included.
    ///
    /// Compiles independently of the host GPU, but compiling this returned
    /// *device source* needs a compatible HIP compiler/toolchain at runtime.
    pub fn build(self)->Result<String,Error>{
        if self.caps.contains(Capability::AmdGcn)&&!self.caps.contains(Capability::AmdWave){return Err(Error::FeatureDisabled(Capability::AmdWave));}
        let mut out=format!("// AMD HIP C++ kernel; compile with HIP-Clang/hipRTC at runtime.\n#include <hip/hip_runtime.h>\nextern \"C\" __global__ void {}({}) {{\n",self.name,self.params.join(", "));
        for stmt in self.statements{out.push_str(&stmt);out.push('\n');}
        out.push_str("}\n");Ok(out)
    }
}
