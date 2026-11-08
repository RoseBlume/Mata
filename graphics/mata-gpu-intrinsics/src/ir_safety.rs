// Phase 2: opt-in target/device profiles and static safety validation.
// All profile feature claims must come from caller-supplied capability probes;
// source backend choice alone is never evidence of GPU hardware support.
use alloc::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Feature {
    Fp16, Fp64, Bfloat16, Int8, Int16, Int64,
    Subgroups, WorkgroupCollectives, GenericAddressSpace,
    NonUniformWorkgroups, ReadWriteImages, Pipes, DeviceEnqueue,
    Svm, FineGrainedSvm, Atomic64, SystemScopeAtomics,
    CooperativeGroups, AsynchronousCopy, Clusters, TensorInstructions,
}

impl Feature {
    pub const fn opencl_feature_macro(self) -> Option<&'static str> {
        match self {
            Self::Fp16 => Some("__opencl_c_fp16"),
            Self::Fp64 => Some("__opencl_c_fp64"),
            Self::Int64 => None, // OpenCL C 64-bit integers are not an OpenCL 3 optional-feature macro
            Self::Subgroups => Some("__opencl_c_subgroups"),
            Self::WorkgroupCollectives => Some("__opencl_c_work_group_collective_functions"),
            Self::GenericAddressSpace => Some("__opencl_c_generic_address_space"),
            Self::ReadWriteImages => Some("__opencl_c_read_write_images"),
            Self::Pipes => Some("__opencl_c_pipes"),
            Self::DeviceEnqueue => Some("__opencl_c_device_enqueue"),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LanguageVersion { pub major:u16, pub minor:u16 }
impl LanguageVersion {
    pub const fn new(major:u16,minor:u16)->Self {Self {major,minor}}
    pub const fn at_least(self,major:u16,minor:u16)->bool {
        self.major>major || (self.major==major && self.minor>=minor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceLimits {
    /// None is UNKNOWN, not unlimited. Device-query-backed strict profiles
    /// reject unknown values when the kernel actually requires the limit.
    pub max_workgroup_size: Option<u32>,
    pub max_workgroup_dimensions: [Option<u32>;3],
    pub max_storage_bindings: Option<u32>,
    pub max_uniform_bytes: Option<usize>,
    pub max_buffer_element_bytes: Option<usize>,
}
impl Default for DeviceLimits {
    fn default()->Self {Self {
        max_workgroup_size:None,max_workgroup_dimensions:[None;3],
        max_storage_bindings:None,max_uniform_bytes:None,max_buffer_element_bytes:None,
    }}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetProfile {
    pub backend: Backend,
    pub language: LanguageVersion,
    /// Explicitly asserted by device/toolchain probing, not inferred.
    pub features: Vec<Feature>,
    pub limits: DeviceLimits,
    /// If enabled, a queried limit must be present before it is used.
    pub strict_limits: bool,
}
impl TargetProfile {
    pub fn offline(backend:Backend)->Self {
        let language=match backend {
            Backend::IntelOpenCl=>LanguageVersion::new(3,0),
            Backend::Glsl=>LanguageVersion::new(4,5),
            Backend::Hlsl=>LanguageVersion::new(6,0),
            Backend::Wgsl=>LanguageVersion::new(1,0),
            Backend::NvidiaCuda|Backend::AmdHip=>LanguageVersion::new(1,0),
        };
        Self{backend,language,features:Vec::new(),limits:DeviceLimits::default(),strict_limits:false}
    }
    pub fn supports(&self, feature:Feature)->bool {self.features.contains(&feature)}
    pub fn enable(&mut self, feature:Feature) {if !self.supports(feature){self.features.push(feature)}}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    InvalidProfile, InvalidWorkgroup, UnknownDeviceLimit, UnsupportedFeature,
    InvalidAddressSpace, InvalidAccess, InvalidLayout, DivergentBarrier,
    DivergentExit, InvalidAtomicOrder, InvalidAtomicScope, InvalidAtomicType,
    IrValidation, SourceGeneration,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub backend:Backend,
    pub kind:DiagnosticKind,
    pub kernel:Option<String>,
    /// Structured source path: e.g. `body[2].if.true[1]`, not a fake line number.
    pub location:String,
    pub message:String,
    pub required_feature:Option<Feature>,
}
impl core::fmt::Display for Diagnostic {
    fn fmt(&self,f:&mut core::fmt::Formatter<'_>)->core::fmt::Result {
        write!(f,"{:?} on {:?} at {}: {}",self.kind,self.backend,self.location,self.message)?;
        if let Some(ref kernel)=self.kernel {write!(f," [kernel/function: {}]",kernel)?;}
        if let Some(feature)=self.required_feature {write!(f," [required: {:?}]",feature)?;}
        Ok(())
    }
}
#[cfg(feature="std")]
impl std::error::Error for Diagnostic {}
impl Diagnostic {
    fn new(profile:&TargetProfile,kind:DiagnosticKind,kernel:Option<&str>,location:&str,message:&str,feature:Option<Feature>)->Self {
        Self{backend:profile.backend,kind,kernel:kernel.map(String::from),location:String::from(location),message:String::from(message),required_feature:feature}
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicOrder { Relaxed, Acquire, Release, AcqRel, SeqCst }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicScope { Workgroup, Device, System }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicKind { Load, Store, ReadModifyWrite, CompareExchange }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtomicSpec {
    pub kind:AtomicKind,
    pub order:AtomicOrder,
    /// CompareExchange failure order, ignored for other operations.
    pub failure:Option<AtomicOrder>,
    pub scope:AtomicScope,
    pub scalar:Number,
    pub address_space:AddressSpace,
    pub alignment:u32,
}
impl AtomicSpec {
    /// Phase 2 contract check; Phase 7 will add actual atomic IR and lowering.
    pub fn validate(&self,profile:&TargetProfile)->Result<(),Diagnostic> {
        let diagnostic=|kind,msg:&str,feature|Diagnostic::new(profile,kind,None,"atomic",msg,feature);
        if self.kind==AtomicKind::Load && matches!(self.order,AtomicOrder::Release|AtomicOrder::AcqRel) ||
           self.kind==AtomicKind::Store && matches!(self.order,AtomicOrder::Acquire|AtomicOrder::AcqRel) {
            return Err(diagnostic(DiagnosticKind::InvalidAtomicOrder,"invalid memory ordering for atomic load/store",None));
        }
        match (self.kind,self.failure) {
            (AtomicKind::CompareExchange,Some(order))=>{
                if matches!(order,AtomicOrder::Release|AtomicOrder::AcqRel) || atomic_strength(order)>atomic_strength(self.order) ||
                    (self.order==AtomicOrder::Release && order!=AtomicOrder::Relaxed) ||
                    (self.order==AtomicOrder::AcqRel && !matches!(order,AtomicOrder::Relaxed|AtomicOrder::Acquire)) {
                    return Err(diagnostic(DiagnosticKind::InvalidAtomicOrder,"invalid compare-exchange failure order",None));
                }
            }
            (AtomicKind::CompareExchange,None)=>return Err(diagnostic(DiagnosticKind::InvalidAtomicOrder,"compare-exchange requires a failure order",None)),
            (_,Some(_))=>return Err(diagnostic(DiagnosticKind::InvalidAtomicOrder,"failure order is valid only for compare-exchange",None)),
            _=>(),
        }
        if !matches!(self.scalar,Number::I32|Number::U32|Number::I64|Number::U64) {
            return Err(diagnostic(DiagnosticKind::InvalidAtomicType,"portable atomic requires 32- or 64-bit integer storage",None));
        }
        let width=if matches!(self.scalar,Number::I64|Number::U64){8}else{4};
        if self.alignment<width || self.alignment%width!=0 {
            return Err(diagnostic(DiagnosticKind::InvalidAtomicType,"atomic storage alignment is inadequate",None));
        }
        if !matches!(self.address_space,AddressSpace::Global|AddressSpace::Workgroup) {
            return Err(diagnostic(DiagnosticKind::InvalidAddressSpace,"atomics must be in global or workgroup memory",None));
        }
        if width==8 && !profile.supports(Feature::Atomic64) {
            return Err(diagnostic(DiagnosticKind::UnsupportedFeature,"64-bit atomics not confirmed by target device",Some(Feature::Atomic64)));
        }
        if self.scope==AtomicScope::System && !profile.supports(Feature::SystemScopeAtomics) {
            return Err(diagnostic(DiagnosticKind::UnsupportedFeature,"system-scope atomic support not confirmed",Some(Feature::SystemScopeAtomics)));
        }
        if self.scope==AtomicScope::System && self.address_space==AddressSpace::Workgroup ||
            self.scope==AtomicScope::Device && self.address_space==AddressSpace::Workgroup {
            return Err(diagnostic(DiagnosticKind::InvalidAtomicScope,"local atomics cannot have broader-than-workgroup scope",None));
        }
        Ok(())
    }
}
/// Contract-only memory fence descriptor. Actual code emission is scheduled
/// for Phase 7 along with atomic and asynchronous synchronization lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FenceDomain { Local, Global, Image }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryFenceSpec {
    pub order:AtomicOrder,
    pub scope:AtomicScope,
    pub domain:FenceDomain,
}
impl MemoryFenceSpec {
    pub fn validate(&self,profile:&TargetProfile)->Result<(),Diagnostic>{
        if self.scope==AtomicScope::System && !profile.supports(Feature::SystemScopeAtomics) {
            return Err(Diagnostic::new(profile,DiagnosticKind::UnsupportedFeature,None,"fence","system-scope memory fence was not confirmed by target device",Some(Feature::SystemScopeAtomics)));
        }
        if self.domain==FenceDomain::Local && self.scope!=AtomicScope::Workgroup {
            return Err(Diagnostic::new(profile,DiagnosticKind::InvalidAtomicScope,None,"fence","workgroup-local memory cannot be ordered at device/system scope using a portable fence",None));
        }
        Ok(())
    }
}

fn atomic_strength(order:AtomicOrder)->u8 { match order {
    AtomicOrder::Relaxed=>0,AtomicOrder::Acquire|AtomicOrder::Release=>1,
    AtomicOrder::AcqRel=>2,AtomicOrder::SeqCst=>3,
}}

fn required_numeric_feature(n:Number)->Option<Feature>{match n {
    Number::F16=>Some(Feature::Fp16),Number::F64=>Some(Feature::Fp64),Number::Bf16=>Some(Feature::Bfloat16),
    Number::I8|Number::U8=>Some(Feature::Int8), Number::I16|Number::U16=>Some(Feature::Int16),
    Number::I64|Number::U64=>Some(Feature::Int64),_=>None,
}}
fn scan_type(ty:&DataType,features:&mut Vec<Feature>){
    match ty {
        DataType::Scalar(n)|DataType::Vector(n,_)=>if let Some(f)=required_numeric_feature(*n){if !features.contains(&f){features.push(f)}},
        DataType::Pointer(inner,space,_)=>{
            if *space==AddressSpace::Generic && !features.contains(&Feature::GenericAddressSpace){features.push(Feature::GenericAddressSpace)}
            scan_type(inner,features)
        },
        DataType::Array(inner,_)=>scan_type(inner,features),
        DataType::Tuple(items)=>for item in items{scan_type(item,features)}, _=>(),
    }
}
fn scan_value(value:&Value,features:&mut Vec<Feature>){
    scan_type(&value.ty(),features);
    match value {
        Value::Binary(_,a,b,_)|Value::Load(a,b,_)|Value::Index(a,b,_)=>{scan_value(a,features);scan_value(b,features)},
        Value::Field(a,_,_)|Value::Cast(a,_)|Value::TupleAt(a,_,_)=>scan_value(a,features),
        Value::Call(_,args,_)|Value::Compute(_,args,_)|Value::Tuple(args)|Value::ArrayInit(args,_)|Value::VectorInit(args,_)=>for arg in args{scan_value(arg,features)},
        Value::RecordInit(_,fields)=>for (_,v) in fields{scan_value(v,features)}, _=>(),
    }
}
fn scan_body(body:&[Statement],features:&mut Vec<Feature>){
    for stmt in body {match stmt {
        Statement::Let(_,t,v)=>{scan_type(t,features);scan_value(v,features)},
        Statement::Store(a,b,v)|Statement::SetIndex(a,b,v)=>{scan_value(a,features);scan_value(b,features);scan_value(v,features)},
        Statement::AtomicAdd{pointer,index,delta}=>{scan_value(pointer,features);scan_value(index,features);scan_value(delta,features)},
        Statement::Atomic(a)=>{scan_value(&a.pointer,features);scan_value(&a.index,features);if let Some(v)=&a.value{scan_value(v,features)}if let Some(v)=&a.expected{scan_value(v,features)}},
        Statement::AsyncCopy{destination,source,count,..}=>{scan_value(destination,features);scan_value(source,features);scan_value(count,features);if !features.contains(&Feature::AsynchronousCopy){features.push(Feature::AsynchronousCopy)}},
        Statement::Fence(_)|Statement::AwaitCopy(_)=>(),
        Statement::Shared{element,..}=>scan_type(&DataType::Scalar(*element),features),
        Statement::Assign(_,v)|Statement::Expr(v)=>scan_value(v,features),
        Statement::If(cond,yes,no)=>{scan_value(cond,features);scan_body(yes,features);scan_body(no,features)},
        Statement::While(cond,body)=>{scan_value(cond,features);scan_body(body,features)},
        Statement::ForRange{start,end,body,..}=>{scan_value(start,features);scan_value(end,features);scan_body(body,features)},
        Statement::Return(Some(v))=>scan_value(v,features),_=>(),
    }}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Uniformity { Uniform, Varying }
impl Uniformity {fn combine(self,rhs:Self)->Self{if self==Self::Uniform && rhs==Self::Uniform {Self::Uniform}else{Self::Varying}}}
fn value_uniformity(value:&Value,env:&BTreeMap<String,Uniformity>)->Uniformity{
    match value {
        Value::Bool(_)|Value::Int(_,_)|Value::UInt(_,_)|Value::FloatBits(_)|Value::F64Bits(_)=>Uniformity::Uniform,
        Value::Var(name,_)=>*env.get(name).unwrap_or(&Uniformity::Varying),
        Value::Builtin(Builtin::GroupId|Builtin::GlobalSize|Builtin::LocalSize|Builtin::NumGroups,_)=>Uniformity::Uniform,
        Value::Builtin(_,_)=>Uniformity::Varying,
        Value::Load(_,_,_)=>Uniformity::Varying, // conservative: memory may be modified by another invocation
        Value::Binary(_,a,b,_)|Value::Index(a,b,_)=>value_uniformity(a,env).combine(value_uniformity(b,env)),
        Value::Field(a,_,_)|Value::Cast(a,_)|Value::TupleAt(a,_,_)=>value_uniformity(a,env),
        // A helper may read per-invocation builtins/memory despite uniform arguments.
        // Without a proven function summary, its result is conservatively varying.
        Value::Call(_,_,_)=>Uniformity::Varying,
        Value::Compute(_,args,_)=>args.iter().fold(Uniformity::Uniform,|u,v|u.combine(value_uniformity(v,env))),
        Value::Tuple(args)|Value::ArrayInit(args,_)|Value::VectorInit(args,_)=>{
            args.iter().fold(Uniformity::Uniform,|u,v|u.combine(value_uniformity(v,env)))
        },
        Value::RecordInit(_,fields)=>fields.iter().fold(Uniformity::Uniform,|u,(_,v)|u.combine(value_uniformity(v,env))),
    }
}
fn contains_return(block:&[Statement])->bool{
    block.iter().any(|stmt|match stmt {
        Statement::Return(_)=>true,
        Statement::If(_,a,b)=>contains_return(a)||contains_return(b),
        Statement::While(_,b)|Statement::ForRange{body:b,..}=>contains_return(b),
        _=>false,
    })
}
fn join_uniform_vars(before:&BTreeMap<String,Uniformity>,lhs:&BTreeMap<String,Uniformity>,rhs:&BTreeMap<String,Uniformity>)->BTreeMap<String,Uniformity>{
    let mut merged=before.clone();
    for (name,original) in before {
        let a=lhs.get(name).copied().unwrap_or(*original);
        let b=rhs.get(name).copied().unwrap_or(*original);
        merged.insert(name.clone(),a.combine(b));
    }
    merged
}
fn analyze_calls(value:&Value,p:&Program,profile:&TargetProfile,kernel:&str,path:&str,env:&BTreeMap<String,Uniformity>,convergent:bool,depth:u8)->Result<(),Diagnostic>{
    if depth>32{return Err(Diagnostic::new(profile,DiagnosticKind::IrValidation,Some(kernel),path,"helper-call analysis recursion limit",None))}
    match value {
        Value::Call(name,args,_)=>{
            for arg in args{analyze_calls(arg,p,profile,kernel,path,env,convergent,depth+1)?}
            if let Some(f)=p.functions.iter().find(|f|f.name==*name){
                let mut locals=BTreeMap::new();for (param,arg) in f.args.iter().zip(args){locals.insert(param.name.clone(),value_uniformity(arg,env));}
                analyze_block(&f.body,p,profile,kernel,&format!("{}.call({})",path,name),&mut locals,convergent,depth+1)?;
            }
        }
        Value::Binary(_,a,b,_)|Value::Load(a,b,_)|Value::Index(a,b,_)=>{
            analyze_calls(a,p,profile,kernel,path,env,convergent,depth+1)?;
            analyze_calls(b,p,profile,kernel,path,env,convergent,depth+1)?;
        }
        Value::Field(a,_,_)|Value::Cast(a,_)|Value::TupleAt(a,_,_)=>analyze_calls(a,p,profile,kernel,path,env,convergent,depth+1)?,
        Value::Tuple(args)|Value::Compute(_,args,_)|Value::ArrayInit(args,_)|Value::VectorInit(args,_)=>for a in args{analyze_calls(a,p,profile,kernel,path,env,convergent,depth+1)?},
        Value::RecordInit(_,fs)=>for (_,a) in fs{analyze_calls(a,p,profile,kernel,path,env,convergent,depth+1)?},
        _=>(),
    }
    Ok(())
}
fn analyze_block(body:&[Statement],p:&Program,profile:&TargetProfile,kernel:&str,path:&str,env:&mut BTreeMap<String,Uniformity>,convergent:bool,depth:u8)->Result<(),Diagnostic>{
    if depth>32{return Err(Diagnostic::new(profile,DiagnosticKind::IrValidation,Some(kernel),path,"control-flow nesting exceeds analysis limit",None))}
    let mut still_convergent=convergent;
    for (i,stmt) in body.iter().enumerate() {
        let here=format!("{}[{}]",path,i);
        match stmt {
            Statement::Let(name,_,value)=>{
                analyze_calls(value,p,profile,kernel,&here,env,still_convergent,depth)?;
                env.insert(name.clone(),value_uniformity(value,env));
            }
            Statement::Assign(name,value)=>{
                analyze_calls(value,p,profile,kernel,&here,env,still_convergent,depth)?;
                env.insert(name.clone(),value_uniformity(value,env));
            }
            Statement::Store(a,b,c)|Statement::SetIndex(a,b,c)=>for v in [a,b,c]{analyze_calls(v,p,profile,kernel,&here,env,still_convergent,depth)?},
            Statement::AtomicAdd{pointer,index,delta}=>for v in [pointer,index,delta]{analyze_calls(v,p,profile,kernel,&here,env,still_convergent,depth)?},
            Statement::Atomic(a)=>{
                for v in [&a.pointer,&a.index]{analyze_calls(v,p,profile,kernel,&here,env,still_convergent,depth)?}
                if let Some(v)=&a.value{analyze_calls(v,p,profile,kernel,&here,env,still_convergent,depth)?}
                if let Some(v)=&a.expected{analyze_calls(v,p,profile,kernel,&here,env,still_convergent,depth)?}
                sync_validation(stmt,profile,kernel,&here,still_convergent)?;
                if let Some(name)=&a.result{env.insert(name.clone(),Uniformity::Varying);}
            },
            Statement::Fence(_)=>sync_validation(stmt,profile,kernel,&here,still_convergent)?,
            Statement::AsyncCopy{destination,source,count,..}=>{
                for v in [destination,source,count]{analyze_calls(v,p,profile,kernel,&here,env,still_convergent,depth)?}
                sync_validation(stmt,profile,kernel,&here,still_convergent)?;
            },
            Statement::AwaitCopy(_)=>sync_validation(stmt,profile,kernel,&here,still_convergent)?,
            Statement::Shared{name,..}=>{env.insert(name.clone(),Uniformity::Uniform);},
            Statement::Expr(v)|Statement::Return(Some(v))=>analyze_calls(v,p,profile,kernel,&here,env,still_convergent,depth)?,
            Statement::Barrier=>{
                if !still_convergent {return Err(Diagnostic::new(profile,DiagnosticKind::DivergentBarrier,Some(kernel),&here,"workgroup barrier may not be reached by all workgroup invocations",None));}
            }
            Statement::If(cond,a,b)=>{
                analyze_calls(cond,p,profile,kernel,&here,env,still_convergent,depth)?;
                let cond_uniform=value_uniformity(cond,env)==Uniformity::Uniform;
                let mut yes=env.clone();let mut no=env.clone();
                analyze_block(a,p,profile,kernel,&format!("{}.if.true",here),&mut yes,still_convergent&&cond_uniform,depth+1)?;
                analyze_block(b,p,profile,kernel,&format!("{}.if.false",here),&mut no,still_convergent&&cond_uniform,depth+1)?;
                let original=env.clone();
                *env=join_uniform_vars(env,&yes,&no);
                if !cond_uniform {
                    // Both divergent arms may assign different constants, even
                    // if both expressions were individually group-uniform.
                    for key in original.keys() {
                        if yes.contains_key(key) || no.contains_key(key) {
                            env.insert(key.clone(),Uniformity::Varying);
                        }
                    }
                }
                if !cond_uniform && (contains_return(a)||contains_return(b)) {still_convergent=false;}
            }
            Statement::While(cond,nested)=>{
                analyze_calls(cond,p,profile,kernel,&here,env,still_convergent,depth)?;
                let uniform=value_uniformity(cond,env)==Uniformity::Uniform;
                let mut nested_env=env.clone();
                // A uniform initial while predicate may become nonuniform on later iterations.
                // Conservatively forbid barriers inside a while loop until fixed-point proof exists.
                analyze_block(nested,p,profile,kernel,&format!("{}.while",here),&mut nested_env,false,depth+1)?;
                *env=join_uniform_vars(env,&nested_env,env);
                if !uniform || contains_return(nested){still_convergent=false;}
            }
            Statement::ForRange{index,start,end,body:nested}=>{
                analyze_calls(start,p,profile,kernel,&here,env,still_convergent,depth)?;
                analyze_calls(end,p,profile,kernel,&here,env,still_convergent,depth)?;
                let uniform=value_uniformity(start,env)==Uniformity::Uniform&&value_uniformity(end,env)==Uniformity::Uniform;
                let mut nested_env=env.clone();
                nested_env.insert(index.clone(),if uniform{Uniformity::Uniform}else{Uniformity::Varying});
                analyze_block(nested,p,profile,kernel,&format!("{}.for",here),&mut nested_env,still_convergent&&uniform,depth+1)?;
                *env=join_uniform_vars(env,&nested_env,env);
                if !uniform || contains_return(nested){still_convergent=false;}
            }
            Statement::Return(None)=>(),
        }
    }
    Ok(())
}
fn validate_argument_space(arg:&Parameter,profile:&TargetProfile,function:Option<&str>)->Result<(),Diagnostic> {
    if let DataType::Pointer(_,space,access)=&arg.ty {
        if *space==AddressSpace::Constant && *access!=Access::ReadOnly {
            return Err(Diagnostic::new(profile,DiagnosticKind::InvalidAccess,function,&arg.name,"constant memory must be read-only",None))
        }
        if matches!(profile.backend,Backend::Wgsl|Backend::Glsl|Backend::Hlsl) && *space!=AddressSpace::Global {
            return Err(Diagnostic::new(profile,DiagnosticKind::InvalidAddressSpace,function,&arg.name,"only global pointers are supported by this shader source emitter",None))
        }
        if matches!(profile.backend,Backend::NvidiaCuda|Backend::AmdHip) && *space!=AddressSpace::Global {
            return Err(Diagnostic::new(profile,DiagnosticKind::InvalidAddressSpace,function,&arg.name,"native CUDA/HIP kernel pointer arguments currently require global memory; constant and shared memory use dedicated symbols",None))
        }
        if profile.backend==Backend::IntelOpenCl && *space==AddressSpace::Private {
            return Err(Diagnostic::new(profile,DiagnosticKind::InvalidAddressSpace,function,&arg.name,"OpenCL entrypoint arguments cannot target private memory",None))
        }
    }
    Ok(())
}
fn validate_kernel_limits(k:&Kernel,p:&Program,profile:&TargetProfile)->Result<(),Diagnostic>{
    let error=|kind,msg:&str|Diagnostic::new(profile,kind,Some(&k.name),"workgroup",msg,None);
    let product=k.workgroup.iter().try_fold(1u64,|acc,&n|acc.checked_mul(u64::from(n))).ok_or_else(||error(DiagnosticKind::InvalidWorkgroup,"workgroup product overflows"))?;
    if product==0 {return Err(error(DiagnosticKind::InvalidWorkgroup,"workgroup sizes cannot be zero"))}
    match profile.limits.max_workgroup_size {
        Some(max) if product>u64::from(max)=>return Err(error(DiagnosticKind::InvalidWorkgroup,"workgroup size exceeds target device limit")),
        None if profile.strict_limits=>return Err(error(DiagnosticKind::UnknownDeviceLimit,"maximum workgroup size was not queried")),
        _=>(),
    }
    for axis in 0..3 {
        match profile.limits.max_workgroup_dimensions[axis] {
            Some(max) if k.workgroup[axis]>max=>return Err(error(DiagnosticKind::InvalidWorkgroup,"workgroup axis exceeds target device limit")),
            None if profile.strict_limits=>return Err(error(DiagnosticKind::UnknownDeviceLimit,"workgroup axis limit was not queried")),
            _=>(),
        }
    }
    let buffers=k.args.iter().filter(|a|matches!(a.ty,DataType::Pointer(_,_,_))).count();
    if let Some(max)=profile.limits.max_storage_bindings {
        if buffers>max as usize {return Err(error(DiagnosticKind::InvalidLayout,"resource bindings exceed target limit"))}
    }else if profile.strict_limits && buffers>0 {return Err(error(DiagnosticKind::UnknownDeviceLimit,"resource binding limit was not queried"))}
    for arg in &k.args {
        validate_argument_space(arg,profile,Some(&k.name))?;
        match &arg.ty {
            DataType::Pointer(inner,space,access)=>{
                if *space==AddressSpace::Constant && *access!=Access::ReadOnly{return Err(Diagnostic::new(profile,DiagnosticKind::InvalidAccess,Some(&k.name),&arg.name,"constant memory must be read-only",None))}
                if *space==AddressSpace::Generic && profile.backend==Backend::IntelOpenCl && !profile.supports(Feature::GenericAddressSpace){return Err(Diagnostic::new(profile,DiagnosticKind::UnsupportedFeature,Some(&k.name),&arg.name,"OpenCL generic address space was not reported by this device",Some(Feature::GenericAddressSpace)))}
                if matches!(profile.backend,Backend::Wgsl|Backend::Glsl|Backend::Hlsl) && *space!=AddressSpace::Global{return Err(Diagnostic::new(profile,DiagnosticKind::InvalidAddressSpace,Some(&k.name),&arg.name,"shader resource pointer must use global address space",None))}
                if *space==AddressSpace::Private && profile.backend==Backend::IntelOpenCl{return Err(Diagnostic::new(profile,DiagnosticKind::InvalidAddressSpace,Some(&k.name),&arg.name,"OpenCL kernel arguments cannot point to private memory",None))}
                let layout=p.layout_for_backend(inner,profile.backend,*space).map_err(|_|Diagnostic::new(profile,DiagnosticKind::InvalidLayout,Some(&k.name),&arg.name,"cannot compute target buffer-element layout",None))?;
                match profile.limits.max_buffer_element_bytes {
                    Some(max) if layout.size>max=>return Err(Diagnostic::new(profile,DiagnosticKind::InvalidLayout,Some(&k.name),&arg.name,"buffer element exceeds target size limit",None)),
                    None if profile.strict_limits=>return Err(Diagnostic::new(profile,DiagnosticKind::UnknownDeviceLimit,Some(&k.name),&arg.name,"maximum buffer-element bytes was not queried",None)),
                    _=>(),
                }
            }
            _=>(),
        }
    }
    if matches!(profile.backend,Backend::Wgsl|Backend::Hlsl) && k.args.iter().any(|arg|!matches!(arg.ty,DataType::Pointer(_,_,_))) {
        let layout=p.kernel_uniform_layout(&k.name,profile.backend)
            .map_err(|_|error(DiagnosticKind::InvalidLayout,"cannot compute kernel scalar parameter layout"))?;
        match profile.limits.max_uniform_bytes {
            Some(max) if layout.layout.size>max=>return Err(error(DiagnosticKind::InvalidLayout,"uniform parameter block exceeds target limit")),
            None if profile.strict_limits=>return Err(error(DiagnosticKind::UnknownDeviceLimit,"maximum uniform-buffer size was not queried")),
            _=>(),
        }
    }
    Ok(())
}
impl Program {
    pub fn required_features(&self)->Vec<Feature>{
        let mut out=Vec::new();
        for r in &self.records{for f in &r.fields{scan_type(&f.ty,&mut out)}}
        for f in &self.functions{scan_type(&f.result,&mut out);for a in &f.args{scan_type(&a.ty,&mut out)}scan_body(&f.body,&mut out)}
        for k in &self.kernels{for a in &k.args{scan_type(&a.ty,&mut out)}scan_body(&k.body,&mut out)}
        out
    }
    /// Checks device-profile claims, target layout, resource limits, and
    /// workgroup convergence. Does not execute or query any device.
    pub fn validate_for(&self,profile:&TargetProfile)->Result<(),Diagnostic>{
        // Catch record layout failures with their original field path rather
        // than losing the location inside Program::validate().
        for r in &self.records { for field in &r.fields {
            self.layout_for_backend(&field.ty,profile.backend,AddressSpace::Global)
                .map_err(|_|Diagnostic::new(profile,DiagnosticKind::InvalidLayout,None,&format!("record.{}.{}",r.name,field.name),"record field has no valid target layout",None))?;
        }}
        self.validate().map_err(|e|Diagnostic::new(profile,DiagnosticKind::IrValidation,None,"program",&format!("{:?}",e),None))?;
        let good_version=match profile.backend {
            Backend::IntelOpenCl=>profile.language.at_least(1,2),
            Backend::Glsl=>profile.language.at_least(4,5),
            Backend::Hlsl=>profile.language.at_least(5,0),
            Backend::Wgsl|Backend::NvidiaCuda|Backend::AmdHip=>profile.language.major>=1,
        };
        if !good_version{return Err(Diagnostic::new(profile,DiagnosticKind::InvalidProfile,None,"profile","target language profile is older than the generated source dialect",None))}
        for f in self.required_features() {
            if !profile.supports(f) {
                return Err(Diagnostic::new(profile,DiagnosticKind::UnsupportedFeature,None,"types","required feature not confirmed by target device",Some(f)));
            }
        }
        for f in &self.functions { for arg in &f.args {
            validate_argument_space(arg,profile,Some(&f.name))?;
        }}
        for k in &self.kernels {
            validate_kernel_limits(k,self,profile)?;
            let mut env=BTreeMap::new();
            for arg in &k.args {env.insert(arg.name.clone(),Uniformity::Uniform);}
            analyze_block(&k.body,self,profile,&k.name,"body",&mut env,true,0)?;
            verify_async_lifetimes(&k.body,profile,&k.name)?;
        }
        // Track transitive helper invocations: a function called through
        // another helper is checked with the effective invocation context.
        let mut called=Vec::new();
        for k in &self.kernels { called_in_block(&k.body,&mut called); }
        let mut index=0;
        while index<called.len() {
            if let Some(f)=self.functions.iter().find(|f|f.name==called[index]) {
                called_in_block(&f.body,&mut called);
            }
            index+=1;
        }
        for f in &self.functions {
            if called.iter().any(|name|name==&f.name){continue}
            let mut env=BTreeMap::new();for arg in &f.args{env.insert(arg.name.clone(),Uniformity::Varying);}
            analyze_block(&f.body,self,profile,&f.name,"function",&mut env,true,0)?;
        }
        Ok(())
    }
    /// Strict, profile-aware source generation; legacy emit() is kept for
    /// API compatibility and retains its earlier behavior.
    pub fn emit_for(&self,profile:&TargetProfile)->Result<String,Diagnostic>{
        self.validate_for(profile)?;
        self.emit(profile.backend).map_err(|e|Diagnostic::new(profile,DiagnosticKind::SourceGeneration,None,"emit",&format!("{:?}",e),None))
    }
}
