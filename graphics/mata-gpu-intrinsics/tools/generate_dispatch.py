"""Regenerate each backend's ordered, typed intrinsic function-pointer tables.

No code executes on a GPU; this emits Rust source and uses only Python stdlib.
Do not edit the generated function pointer array by hand.
"""
from pathlib import Path
import re

ROOT=Path(__file__).resolve().parents[1]
CAT=(ROOT/'src/catalog.rs').read_text(encoding='utf-8')
matched=re.search(r'pub static ALL_INTRINSICS:&\[Intrinsic\]=&\[(.*?)\];',CAT,re.S)
if not matched: raise SystemExit('catalog array not found')
variants=re.findall(r'Intrinsic::([A-Za-z0-9_]+),',matched.group(1))
if not variants or len(set(variants))!=len(variants):raise SystemExit('catalog missing or duplicate operations')

preamble='''//! Generated backend-specific shader-emission function pointer tables.
//!
//! Each table has one entry per catalog ID; entries are filtered against the
//! configured backend, shader stage and requested capability mask when building
//! a [`FunctionTable`]. They emit **source**, not native CPU or GPU machine code.
//!
//! `tools/generate_dispatch.py` regenerates the backend pointer arrays.
use crate::{Backend, CapabilitySet, Emitter, Error, Expr, Intrinsic,
    IntrinsicInfo, Stage, Type, ALL_INTRINSICS, INTRINSIC_COUNT};

/// One backend-bound shader-emission function pointer. The `Option<Type>` is
/// `None` when an intrinsic has an inferable result and `Some(type)` when the
/// caller wishes to use `Emitter::call_typed`.
///
/// A pointer is bound to the backend it was created for. Passing an emitter
/// configured for another backend returns `Error::BackendMismatch`.
pub type IntrinsicFn = fn(&Emitter, &[Expr], Option<Type>) -> Result<Expr,Error>;

// Generic monomorphization creates a distinct function pointer per backend
// and intrinsic, without dynamic allocation or unsafe casts.
fn invoke<const B: u8, const ID: usize>(
    emitter:&Emitter, args:&[Expr], return_type:Option<Type>,
) -> Result<Expr,Error> {
    let expected=match B {0=>Backend::Wgsl,1=>Backend::Glsl,2=>Backend::Hlsl,
        3=>Backend::IntelOpenCl,4=>Backend::NvidiaCuda,5=>Backend::AmdHip,
        _=>unreachable!("the generated table has an invalid backend ID")};
    if emitter.backend!=expected {
        return Err(Error::BackendMismatch{expected,actual:emitter.backend});
    }
    let op=ALL_INTRINSICS[ID];
    match return_type {
        None=>emitter.call(op,args),
        Some(ty)=>emitter.call_typed(op,args,ty),
    }
}

// A backend table is fixed by the compiler; no GPU lookup or runtime
// function loading takes place here. This is intentionally *not* a driver
// dispatch table such as Vulkan/GL/D3D function pointers.
'''
parts=[preamble]
for i,(name,label) in enumerate([('WGSL','WGSL'),('GLSL','GLSL'),('HLSL','HLSL'),('INTEL_OPENCL','Intel OpenCL C'),('NVIDIA_CUDA','NVIDIA CUDA C++'),('AMD_HIP','AMD HIP C++')]):
    parts.append(f'// {label}: {len(variants)} compile-time handlers, filtered when selected.\n')
    parts.append(f'static {name}_FUNCTIONS:[IntrinsicFn;INTRINSIC_COUNT]=[\n')
    parts.extend(f'    invoke::<{i},{{Intrinsic::{op} as usize}}>,\n' for op in variants)
    parts.append('];\n')
parts.append('''/// Return unfiltered handler pointers for a particular language target.
/// These are backend-specific function pointers, *not* hardware driver entry
/// points. Prefer [`FunctionTable`] for stage/capability-filtered lookup.
pub fn backend_pointers(backend:Backend)->&'static [IntrinsicFn;INTRINSIC_COUNT]{
    match backend {
        Backend::Wgsl=>&WGSL_FUNCTIONS,
        Backend::Glsl=>&GLSL_FUNCTIONS,
        Backend::Hlsl=>&HLSL_FUNCTIONS,
        Backend::IntelOpenCl=>&INTEL_OPENCL_FUNCTIONS,
        Backend::NvidiaCuda=>&NVIDIA_CUDA_FUNCTIONS,
        Backend::AmdHip=>&AMD_HIP_FUNCTIONS,
    }
}

/// A function pointer captured with the backend, stage and capability
/// configuration from its parent [`FunctionTable`].
#[derive(Clone,Copy)]
pub struct BoundFunction<'a> {
    emitter:&'a Emitter,
    function:IntrinsicFn,
    intrinsic:Intrinsic,
}
impl<'a> BoundFunction<'a> {
    /// Emit an intrinsic using type inference; returns an error for opaque
    /// texture, atomic or vendor operations requiring an explicit result type.
    pub fn call(&self,args:&[Expr])->Result<Expr,Error>{
        (self.function)(self.emitter,args,None)
    }
    /// Emit an intrinsic with an explicit result type. For inferred signatures
    /// the type must still match the result of the operation.
    pub fn call_typed(&self,args:&[Expr],return_type:Type)->Result<Expr,Error>{
        (self.function)(self.emitter,args,Some(return_type))
    }
    /// Return the intrinsic that this bound pointer emits.
    pub const fn intrinsic(&self)->Intrinsic{self.intrinsic}
    /// Return the underlying function pointer. Calling it directly still
    /// requires a matching emitter, and validates its stage/capabilities.
    pub const fn pointer(&self)->IntrinsicFn{self.function}
    /// Borrow the emitter this bound function uses.
    pub const fn emitter(&self)->&'a Emitter{self.emitter}
}

/// An intrinsic and its catalog metadata, together with a supported function
/// pointer when the backend, stage and requested capabilities allow it.
#[derive(Clone,Copy)]
pub struct TableEntry<'a> {
    /// The catalog identifier, stable within this crate version.
    pub intrinsic:Intrinsic,
    /// The intrinsic's static documentation/arity/capability information.
    pub info:&'static IntrinsicInfo,
    /// `None` when this backend/stage/capability set cannot emit the intrinsic.
    pub function:Option<BoundFunction<'a>>,
}

/// Full backend-specific function-pointer dispatch table for all catalog
/// intrinsics. Created on the CPU without a GPU, driver, SDK or allocations.
///
/// Unsupported slots are `None`; all supported slots point to separately
/// instantiated functions bound to this table's backend. Type and argument
/// validation still runs in the existing `Emitter` implementation.
#[derive(Clone)]
pub struct FunctionTable {
    emitter:Emitter,
    functions:[Option<IntrinsicFn>; INTRINSIC_COUNT],
    available:usize,
}
impl FunctionTable {
    /// Create the function table for any supported backend and shader stage.
    /// Capabilities are requested, not probed on the local GPU.
    pub fn new(backend:Backend, stage:Stage, capabilities:CapabilitySet)->Self {
        Self::from_emitter(Emitter::new(backend,stage,capabilities))
    }
    /// Create the function table using an existing emitter configuration.
    pub fn from_emitter(emitter:Emitter)->Self {
        let pointers=backend_pointers(emitter.backend);
        let functions=core::array::from_fn(|i| {
            if emitter.supports(ALL_INTRINSICS[i]) {Some(pointers[i])} else {None}
        });
        let available=functions.iter().filter(|f|f.is_some()).count();
        Self{emitter,functions,available}
    }
    /// Create a WGSL table; valid in vertex, fragment or compute stages.
    pub fn wgsl(stage:Stage,capabilities:CapabilitySet)->Self {
        Self::new(Backend::Wgsl,stage,capabilities)
    }
    /// Create a GLSL table; valid in vertex, fragment or compute stages.
    pub fn glsl(stage:Stage,capabilities:CapabilitySet)->Self {
        Self::new(Backend::Glsl,stage,capabilities)
    }
    /// Create an HLSL table; valid in vertex, fragment or compute stages.
    pub fn hlsl(stage:Stage,capabilities:CapabilitySet)->Self {
        Self::new(Backend::Hlsl,stage,capabilities)
    }
    /// Create an Intel OpenCL C compute table.
    pub fn intel(capabilities:CapabilitySet)->Self {
        Self::new(Backend::IntelOpenCl,Stage::Compute,capabilities)
    }
    /// Create an NVIDIA CUDA C++ compute table.
    pub fn nvidia(capabilities:CapabilitySet)->Self {
        Self::new(Backend::NvidiaCuda,Stage::Compute,capabilities)
    }
    /// Create an AMD HIP C++ compute table.
    pub fn amd(capabilities:CapabilitySet)->Self {
        Self::new(Backend::AmdHip,Stage::Compute,capabilities)
    }
    /// Borrow the original emitter configuration.
    pub const fn emitter(&self)->&Emitter{&self.emitter}
    /// Return the target language of this dispatch table.
    pub const fn backend(&self)->Backend{self.emitter.backend}
    /// Return the configured shader stage.
    pub const fn stage(&self)->Stage{self.emitter.stage}
    /// Return the requested shader capability mask.
    pub const fn capabilities(&self)->CapabilitySet{self.emitter.capabilities}
    /// Number of available functions after backend/stage/capability filtering.
    pub const fn available_count(&self)->usize{self.available}
    /// Number of entries in the complete intrinsic catalog.
    pub const fn catalog_count(&self)->usize{INTRINSIC_COUNT}
    /// Whether a function pointer is available with this configuration.
    pub fn contains(&self,op:Intrinsic)->bool{self.functions[op as usize].is_some()}
    /// Borrow a bound callable for one intrinsic, or `None` if unavailable.
    pub fn get(&self,op:Intrinsic)->Option<BoundFunction<'_>>{
        self.functions[op as usize].map(|function|BoundFunction{
            emitter:&self.emitter,function,intrinsic:op,
        })
    }
    /// Look up a bound function by its catalog name, such as `"sin"` or
    /// `"nvidiaShuffleSync"`. Names are case sensitive.
    pub fn get_named(&self,name:&str)->Option<BoundFunction<'_>>{
        self.get(Intrinsic::from_name(name)?)
    }
    /// Return an unbound backend-specific function pointer, if available.
    /// Use [`Self::emitter`] to supply a matching emitter when invoking it.
    pub fn pointer(&self,op:Intrinsic)->Option<IntrinsicFn>{self.functions[op as usize]}
    /// Invoke a supported pointer with inferred return type. When unavailable,
    /// delegate to the emitter to retain precise stage/capability errors.
    pub fn call(&self,op:Intrinsic,args:&[Expr])->Result<Expr,Error>{
        match self.get(op){Some(f)=>f.call(args),None=>self.emitter.call(op,args)}
    }
    /// Invoke an intrinsic with an explicit result type, including special
    /// atomic, texture and subgroup operations.
    pub fn call_typed(&self,op:Intrinsic,args:&[Expr],ty:Type)->Result<Expr,Error>{
        match self.get(op){Some(f)=>f.call_typed(args,ty),None=>self.emitter.call_typed(op,args,ty)}
    }
    /// Invoke an intrinsic by its canonical catalog name. Unknown names
    /// return `Error::UnknownIntrinsicName`.
    pub fn call_named(&self,name:&str,args:&[Expr])->Result<Expr,Error>{
        let op=Intrinsic::from_name(name).ok_or(Error::UnknownIntrinsicName)?;
        self.call(op,args)
    }
    /// Invoke an intrinsic by name with an explicit result type.
    pub fn call_named_typed(&self,name:&str,args:&[Expr],ty:Type)->Result<Expr,Error>{
        let op=Intrinsic::from_name(name).ok_or(Error::UnknownIntrinsicName)?;
        self.call_typed(op,args,ty)
    }
    /// Return the metadata and available function pointer for one entry.
    pub fn entry(&self,op:Intrinsic)->TableEntry<'_>{TableEntry{
        intrinsic:op,info:op.info(),function:self.get(op),
    }}
    /// Iterate over every catalog entry in stable order, including unsupported
    /// entries whose function pointers are `None`.
    pub fn entries(&self)->impl Iterator<Item=TableEntry<'_>> + '_ {
        ALL_INTRINSICS.iter().copied().map(|op|self.entry(op))
    }
}
''')
path=ROOT/'src/dispatch.rs'
path.write_text(''.join(parts),encoding='utf-8')
print('generated',path, 'ops',len(variants),'backend pointers',len(variants)*6)
