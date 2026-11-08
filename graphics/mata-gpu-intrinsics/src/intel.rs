//! Intel GPU OpenCL C source backend.
//!
//! Compilation of this Rust module requires **no** Intel GPU, OpenCL SDK, runtime
//! library, driver, or host FFI. The result is **device-side OpenCL C source**.
//! Applications compile and dispatch it later using an OpenCL implementation;
//! simply generating a kernel does not execute it.
//!
//! The `cl_intel_subgroups` / `cl_intel_subgroups_short` extension names refer
//! to runtime device support, not properties of the machine building the crate.
use alloc::{format, string::{String,ToString}, vec::Vec};
use crate::{Backend,Capability,CapabilitySet,Emitter,Error,Expr,Scalar,Stage,Type,is_identifier};

/// Read/write intent of a global OpenCL buffer kernel argument.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum BufferAccess {
    /// Parameter is a `const __global T*`; writing is rejected by the builder.
    ReadOnly,
    /// Parameter is a `__global T*`; both reads and writes are permitted.
    ReadWrite,
}

/// An OpenCL kernel argument whose elements can be loaded and stored as typed expressions.
///
/// The buffer holds no host allocation and does not upload data. A runtime must
/// supply the matching OpenCL buffer when invoking the kernel.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Buffer {
    name:String,
    element:Type,
    access:BufferAccess,
}
impl Buffer {
    /// Return the parameter identifier used in generated source.
    pub fn name(&self)->&str {&self.name}
    /// Return the scalar or vector element type stored in this global buffer.
    pub fn element_type(&self)->Type {self.element}
    /// Return the read-only/read-write classification.
    pub fn access(&self)->BufferAccess {self.access}
    /// Emit `buffer[index]` without actually reading GPU memory.
    pub fn load(&self,index:&Expr)->Result<Expr,Error>{
        if index.ty()!=Type::U32 && index.ty()!=Type::I32 {return Err(Error::InvalidType)}
        Ok(Expr::from_source_unchecked(format!("{}[{}]",self.name,index.code()),self.element))
    }
    /// Emit a pointer suitable for Intel subgroup block I/O, e.g. `buffer + index`.
    ///
    /// Its type annotation denotes *pointee value* rather than Rust pointer
    /// metadata. All lanes in the subgroup must pass the same pointer; the
    /// device's extension specification requires this for block operations.
    pub fn address_at(&self,index:&Expr)->Result<Expr,Error>{
        if index.ty()!=Type::U32 && index.ty()!=Type::I32{return Err(Error::InvalidType)}
        Ok(Expr::from_source_unchecked(format!("({} + {})",self.name,index.code()),self.element))
    }
}

/// Source-only Intel compute kernel builder with typed global-buffer parameters.
///
/// This is a small safe subset of OpenCL kernel declarations, not a shader
/// compiler or driver wrapper. It produces a complete OpenCL C kernel source
/// string, including enabled extension pragmas and the `__kernel` declaration.
pub struct KernelBuilder {
    name:String,
    caps:CapabilitySet,
    parameters:Vec<String>,
    names:Vec<String>,
    statements:Vec<String>,
}
impl KernelBuilder {
    /// Start a named kernel; validates that the name is an ASCII identifier.
    pub fn new(name:&str,caps:CapabilitySet)->Result<Self,Error>{
        if !is_identifier(name){return Err(Error::InvalidIdentifier)}
        Ok(Self{name:name.into(),caps,parameters:Vec::new(),names:Vec::new(),statements:Vec::new()})
    }
    /// Return the matching CPU-side OpenCL expression emitter.
    pub const fn emitter(&self)->Emitter {Emitter::new(Backend::IntelOpenCl,Stage::Compute,self.caps)}
    fn register(&mut self,name:&str)->Result<(),Error>{
        if !is_identifier(name){return Err(Error::InvalidIdentifier)}
        if self.names.iter().any(|n|n==name){return Err(Error::DuplicateParameter)}
        self.names.push(name.to_string());Ok(())
    }
    fn element_type(ty:Type)->bool{
        matches!(ty,Type::Scalar(Scalar::F32|Scalar::I32|Scalar::U32)
            |Type::Vector(Scalar::F32|Scalar::I32|Scalar::U32,2..=4))
    }
    /// Add a `const __global T*` buffer parameter and return a read-only handle.
    pub fn read_only_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>{
        self.buffer(name,element,BufferAccess::ReadOnly)
    }
    /// Add a `__global T*` buffer parameter and return a writable handle.
    pub fn read_write_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>{
        self.buffer(name,element,BufferAccess::ReadWrite)
    }
    /// Add a typed global-buffer parameter; no allocation occurs on the host.
    pub fn buffer(&mut self,name:&str,element:Type,access:BufferAccess)->Result<Buffer,Error>{
        if !Self::element_type(element){return Err(Error::InvalidType)}
        self.register(name)?;
        let qualifier=if access==BufferAccess::ReadOnly{"const __global"}else{"__global"};
        self.parameters.push(format!("{} {}* {}",qualifier,element.source(Backend::IntelOpenCl),name));
        Ok(Buffer{name:name.to_string(),element,access})
    }
    /// Add a scalar/vector value parameter; returns its typed expression.
    pub fn value(&mut self,name:&str,ty:Type)->Result<Expr,Error>{
        if !Self::element_type(ty){return Err(Error::InvalidType)}
        self.register(name)?;
        self.parameters.push(format!("{} {}",ty.source(Backend::IntelOpenCl),name));
        Expr::variable(name,ty)
    }
    /// Append a local variable holding a GPU operation result.
    pub fn let_value(&mut self,name:&str,expression:&Expr)->Result<Expr,Error>{
        if !Self::element_type(expression.ty()){return Err(Error::InvalidType)}
        self.register(name)?;
        self.statements.push(format!("    {} {} = {};",expression.ty().source(Backend::IntelOpenCl),name,expression.code()));
        Expr::variable(name,expression.ty())
    }
    /// Emit the one-dimensional global work-item ID as a local unsigned integer.
    ///
    /// OpenCL's `size_t` ID is converted to `uint`, so launch ranges must fit in
    /// 32 bits for code produced by this convenience method.
    pub fn index_1d(&mut self,name:&str)->Result<Expr,Error>{
        self.register(name)?;
        self.statements.push(format!("    uint {} = (uint)get_global_id(0);",name));
        Expr::variable(name,Type::U32)
    }
    /// Write a typed expression to a writable buffer element.
    pub fn store(&mut self,buffer:&Buffer,index:&Expr,value:&Expr)->Result<(),Error>{
        if buffer.access!=BufferAccess::ReadWrite{return Err(Error::InvalidType)}
        if buffer.element!=value.ty(){return Err(Error::InvalidType)}
        if index.ty()!=Type::U32 && index.ty()!=Type::I32{return Err(Error::InvalidType)}
        self.statements.push(format!("    {}[{}] = {};",buffer.name,index.code(),value.code()));
        Ok(())
    }
    /// Append a validated expression statement, including a barrier or block store.
    ///
    /// `Expr::from_source_unchecked` is not safe for untrusted shader text.
    pub fn statement(&mut self,expression:&Expr){
        self.statements.push(format!("    {};",expression.code()));
    }
    /// Append a bounds test that returns early from the kernel when out of range.
    pub fn return_if_out_of_bounds(&mut self,index:&Expr,limit:&Expr)->Result<(),Error>{
        if index.ty()!=Type::U32 || limit.ty()!=Type::U32{return Err(Error::InvalidType)}
        self.statements.push(format!("    if ({} >= {}) return;",index.code(),limit.code()));Ok(())
    }
    /// Produce complete Intel OpenCL C source with opt-in extension pragmas.
    ///
    /// Requests for extensions are **not** runtime capability discovery. Check
    /// `CL_DEVICE_EXTENSIONS` and OpenCL version before building the kernel.
    pub fn build(self)->Result<String,Error>{
        if self.caps.contains(Capability::IntelSubgroupsShort)
            && !self.caps.contains(Capability::IntelSubgroups) {
            return Err(Error::FeatureDisabled(Capability::IntelSubgroups));
        }
        let mut code=String::from("// Intel OpenCL C device kernel; compile on the target runtime GPU.\n");
        if self.caps.contains(Capability::IntelSubgroups) {
            code.push_str("#pragma OPENCL EXTENSION cl_intel_subgroups : enable\n");
        }
        if self.caps.contains(Capability::IntelSubgroupsShort){
            code.push_str("#pragma OPENCL EXTENSION cl_intel_subgroups_short : enable\n");
        }
        if self.caps.contains(Capability::Subgroups){
            code.push_str("#pragma OPENCL EXTENSION cl_khr_subgroups : enable\n");
        }
        code.push_str(&format!("\n__kernel void {}({}) {{\n",self.name,self.parameters.join(", ")));
        for line in self.statements {code.push_str(&line);code.push('\n');}
        code.push_str("}\n");Ok(code)
    }
}
