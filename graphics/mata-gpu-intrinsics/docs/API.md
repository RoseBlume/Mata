# Rust API Reference — every exported function

This document is generated from the crate source by `python3 tools/generate_docs.py`. Each entry lists its actual Rust signature and its source-level documentation. For the `Intrinsic` variants, see [INTRINSICS.md](INTRINSICS.md).

## Public types

| Type | Responsibility |
|---|---|
| `Backend` | Select WGSL, GLSL, HLSL, Intel OpenCL C, NVIDIA CUDA C++, or AMD HIP C++ syntax |
| `Stage` | Vertex, fragment, compute validity context |
| `Scalar`, `Type` | Shader scalar/vector/matrix/resource type metadata |
| `Capability`, `CapabilitySet` | Shader features requested by the caller, not GPU detection |
| `Intrinsic`, `IntrinsicInfo`, `Family`, `Signature`, `Arity` | Operation registry and native language spellings |
| `Expr` | Typed shader-source expression |
| `Emitter` | Validated expression emitter |
| `FunctionTable`, `BoundFunction`, `IntrinsicFn`, `TableEntry` | Per-backend, capability-filtered source-emission function-pointer dispatch |
| `FunctionBuilder` | Shader helper-function serialization |
| `Error` | Invalid identifier/type/literal, stage, capability, arity or backend |
| `nvidia::KernelBuilder` | Generate standalone CUDA device-kernel source without linking a CUDA SDK |
| `amd::KernelBuilder` | Generate HIP device kernel source without linking ROCm or HIP |
| `amd::Buffer` | Typed HIP global buffer parameter and atomic addresses |
| `nvidia::Buffer` | Typed CUDA device buffer parameter and atomic pointer expressions |
| `cpu::Float4` | Four-lane portable CPU reference vector (`std` feature) |

## Public functions and methods (including every named intrinsic wrapper)

### `src/catalog.rs` — 3 exported functions

#### `spelling`

```rust
pub fn spelling(&self, backend:crate::Backend)->Option<&'static str>
```

Return the target builtin spelling or `None` when unsupported.

#### `info`

```rust
pub fn info(self)->&'static IntrinsicInfo
```

Retrieve metadata without querying the GPU.

#### `from_name`

```rust
pub fn from_name(name:&str)->Option<Self>
```

Look up a catalog operation by its stable camel-case identifier.

### `src/types.rs` — 12 exported functions

#### `is_float`

```rust
pub fn is_float(self) -> bool
```

Return whether the type is a floating scalar or vector.

#### `is_integer`

```rust
pub fn is_integer(self) -> bool
```

Return whether the type is a signed or unsigned integer scalar/vector.

#### `is_numeric`

```rust
pub fn is_numeric(self) -> bool
```

Return whether the type is a supported numeric scalar/vector.

#### `is_vector`

```rust
pub fn is_vector(self) -> bool
```

Return whether the type is a 2-, 3-, or 4-lane vector.

#### `is_bool`

```rust
pub fn is_bool(self) -> bool
```

Return whether the type is a Boolean scalar/vector.

#### `is_texture`

```rust
pub fn is_texture(self) -> bool
```

Return whether this is a sampled or storage texture.

#### `is_atomic`

```rust
pub fn is_atomic(self) -> bool
```

Return whether this is an integer atomic type.

#### `valid`

```rust
pub fn valid(self) -> bool
```

Check the lane dimensions and atomic element-type constraints.

#### `source`

```rust
pub fn source(self, backend: Backend) -> String
```

Return the target shader-language type spelling; GLSL samplers are combined.

#### `bit`

```rust
pub const fn bit(self) -> u32
```

Return the capability bit mask; Core is always enabled.

#### `with`

```rust
pub const fn with(self, c:Capability)->Self
```

Return a new capability set with this capability enabled.

#### `contains`

```rust
pub const fn contains(self,c:Capability)->bool
```

Check whether a capability is present.

### `src/emit.rs` — 22 exported functions

#### `is_identifier`

```rust
pub fn is_identifier(s:&str)->bool
```

Check the ASCII syntax for an unqualified shader identifier. Does not check target-language reserved words.

#### `variable`

```rust
pub fn variable(name:&str,ty:Type)->Result<Self,Error>
```

Construct a shader variable reference using a simple ASCII identifier.

#### `from_source_unchecked`

```rust
pub fn from_source_unchecked(source:impl Into<String>,ty:Type)->Self
```

Caller is responsible for shader-source safety and type correctness. Use for backend-specific variables, indexed values and struct members.

#### `float`

```rust
pub fn float(v:f32)->Result<Self,Error>
```

Create a finite 32-bit floating literal.

#### `int`

```rust
pub fn int(v:i32)->Self
```

Create a 32-bit signed integer literal.

#### `uint`

```rust
pub fn uint(v:u32)->Self
```

Create a 32-bit unsigned integer literal.

#### `uint64`

```rust
pub fn uint64(v:u64)->Self
```

Create a 64-bit unsigned integer literal for HIP wave64 ballot masks.

#### `boolean`

```rust
pub fn boolean(v:bool)->Self
```

Create a Boolean literal.

#### `code`

```rust
pub fn code(&self)->&str
```

Borrow the shader-source expression.

#### `ty`

```rust
pub const fn ty(&self)->Type
```

Return the emitter-side type annotation.

#### `new`

```rust
pub const fn new(backend:Backend,stage:Stage,capabilities:CapabilitySet)->Self
```

Construct an emitter without probing any graphics hardware.

#### `supports`

```rust
pub fn supports(&self,op:Intrinsic)->bool
```

Check catalog spelling, declared shader capability, and stage. Does not validate the physical device or shader compiler version.

#### `call`

```rust
pub fn call(&self,op:Intrinsic,args:&[Expr])->Result<Expr,Error>
```

Result type is inferred for operations with safe, portable signatures. Opaque operations (textures, atomics, subgroup-specific return types) deliberately require `call_typed`, instead of guessing a type. Emit a built-in call and infer types for supported simple signatures. Returns an error instead of guessing special texture/atomic results.

#### `call_typed`

```rust
pub fn call_typed(&self,op:Intrinsic,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Special operations use an explicitly declared return type. This is needed for texture formats, atomics and backend-specific subgroup ops. Backends will still perform their own shader-type validation at runtime. Emit a built-in expression with an explicit result type. Use this for texture, atomic, subgroup and backend-specific calls.

#### `construct`

```rust
pub fn construct(&self,ty:Type,args:&[Expr])->Result<Expr,Error>
```

Produce a scalar/vector/matrix constructor expression in target syntax. The shader compiler ultimately checks constructor overloads.

#### `new`

```rust
pub fn new(emitter:Emitter,name:&str,returns:Type)->Result<Self,Error>
```

Begin a shader helper-function declaration.

#### `parameter`

```rust
pub fn parameter(&mut self,name:&str,ty:Type)->Result<Expr,Error>
```

Append a typed parameter; duplicate identifiers are rejected.

#### `let_value`

```rust
pub fn let_value(&mut self,name:&str,expression:&Expr)->Result<Expr,Error>
```

Declare an immutable local expression (WGSL `let`, HLSL/GLSL initialized variable).

#### `statement`

```rust
pub fn statement(&mut self,expression:&Expr)
```

Append one expression statement, including void-valued barrier/atomic calls.

#### `return_expr`

```rust
pub fn return_expr(&mut self,expression:&Expr)->Result<(),Error>
```

Append a typed return statement.

#### `discard`

```rust
pub fn discard(&mut self)->Result<(),Error>
```

Append the target language's fragment-stage `discard` statement. This is a statement rather than a call expression in shader languages.

#### `build`

```rust
pub fn build(self)->Result<String,Error>
```

Serialize the helper-function declaration to shader source.

### `src/dispatch.rs` — 30 exported functions

#### `backend_pointers`

```rust
pub fn backend_pointers(backend:Backend)->&'static [IntrinsicFn;INTRINSIC_COUNT]
```

Return unfiltered handler pointers for a particular language target. These are backend-specific function pointers, *not* hardware driver entry points. Prefer [`FunctionTable`] for stage/capability-filtered lookup.

#### `call`

```rust
pub fn call(&self,args:&[Expr])->Result<Expr,Error>
```

Emit an intrinsic using type inference; returns an error for opaque texture, atomic or vendor operations requiring an explicit result type.

#### `call_typed`

```rust
pub fn call_typed(&self,args:&[Expr],return_type:Type)->Result<Expr,Error>
```

Emit an intrinsic with an explicit result type. For inferred signatures the type must still match the result of the operation.

#### `intrinsic`

```rust
pub const fn intrinsic(&self)->Intrinsic
```

Return the intrinsic that this bound pointer emits.

#### `pointer`

```rust
pub const fn pointer(&self)->IntrinsicFn
```

Return the underlying function pointer. Calling it directly still requires a matching emitter, and validates its stage/capabilities.

#### `emitter`

```rust
pub const fn emitter(&self)->&'a Emitter
```

Borrow the emitter this bound function uses.

#### `new`

```rust
pub fn new(backend:Backend, stage:Stage, capabilities:CapabilitySet)->Self
```

Create the function table for any supported backend and shader stage. Capabilities are requested, not probed on the local GPU.

#### `from_emitter`

```rust
pub fn from_emitter(emitter:Emitter)->Self
```

Create the function table using an existing emitter configuration.

#### `wgsl`

```rust
pub fn wgsl(stage:Stage,capabilities:CapabilitySet)->Self
```

Create a WGSL table; valid in vertex, fragment or compute stages.

#### `glsl`

```rust
pub fn glsl(stage:Stage,capabilities:CapabilitySet)->Self
```

Create a GLSL table; valid in vertex, fragment or compute stages.

#### `hlsl`

```rust
pub fn hlsl(stage:Stage,capabilities:CapabilitySet)->Self
```

Create an HLSL table; valid in vertex, fragment or compute stages.

#### `intel`

```rust
pub fn intel(capabilities:CapabilitySet)->Self
```

Create an Intel OpenCL C compute table.

#### `nvidia`

```rust
pub fn nvidia(capabilities:CapabilitySet)->Self
```

Create an NVIDIA CUDA C++ compute table.

#### `amd`

```rust
pub fn amd(capabilities:CapabilitySet)->Self
```

Create an AMD HIP C++ compute table.

#### `emitter`

```rust
pub const fn emitter(&self)->&Emitter
```

Borrow the original emitter configuration.

#### `backend`

```rust
pub const fn backend(&self)->Backend
```

Return the target language of this dispatch table.

#### `stage`

```rust
pub const fn stage(&self)->Stage
```

Return the configured shader stage.

#### `capabilities`

```rust
pub const fn capabilities(&self)->CapabilitySet
```

Return the requested shader capability mask.

#### `available_count`

```rust
pub const fn available_count(&self)->usize
```

Number of available functions after backend/stage/capability filtering.

#### `catalog_count`

```rust
pub const fn catalog_count(&self)->usize
```

Number of entries in the complete intrinsic catalog.

#### `contains`

```rust
pub fn contains(&self,op:Intrinsic)->bool
```

Whether a function pointer is available with this configuration.

#### `get`

```rust
pub fn get(&self,op:Intrinsic)->Option<BoundFunction<'_>>
```

Borrow a bound callable for one intrinsic, or `None` if unavailable.

#### `get_named`

```rust
pub fn get_named(&self,name:&str)->Option<BoundFunction<'_>>
```

Look up a bound function by its catalog name, such as `"sin"` or `"nvidiaShuffleSync"`. Names are case sensitive.

#### `pointer`

```rust
pub fn pointer(&self,op:Intrinsic)->Option<IntrinsicFn>
```

Return an unbound backend-specific function pointer, if available. Use [`Self::emitter`] to supply a matching emitter when invoking it.

#### `call`

```rust
pub fn call(&self,op:Intrinsic,args:&[Expr])->Result<Expr,Error>
```

Invoke a supported pointer with inferred return type. When unavailable, delegate to the emitter to retain precise stage/capability errors.

#### `call_typed`

```rust
pub fn call_typed(&self,op:Intrinsic,args:&[Expr],ty:Type)->Result<Expr,Error>
```

Invoke an intrinsic with an explicit result type, including special atomic, texture and subgroup operations.

#### `call_named`

```rust
pub fn call_named(&self,name:&str,args:&[Expr])->Result<Expr,Error>
```

Invoke an intrinsic by its canonical catalog name. Unknown names return `Error::UnknownIntrinsicName`.

#### `call_named_typed`

```rust
pub fn call_named_typed(&self,name:&str,args:&[Expr],ty:Type)->Result<Expr,Error>
```

Invoke an intrinsic by name with an explicit result type.

#### `entry`

```rust
pub fn entry(&self,op:Intrinsic)->TableEntry<'_>
```

Return the metadata and available function pointer for one entry.

#### `entries`

```rust
pub fn entries(&self)->impl Iterator<Item=TableEntry<'_>> + '_
```

Iterate over every catalog entry in stable order, including unsupported entries whose function pointers are `None`.

### `src/intel.rs` — 17 exported functions

#### `name`

```rust
pub fn name(&self)->&str
```

Return the parameter identifier used in generated source.

#### `element_type`

```rust
pub fn element_type(&self)->Type
```

Return the scalar or vector element type stored in this global buffer.

#### `access`

```rust
pub fn access(&self)->BufferAccess
```

Return the read-only/read-write classification.

#### `load`

```rust
pub fn load(&self,index:&Expr)->Result<Expr,Error>
```

Emit `buffer[index]` without actually reading GPU memory.

#### `address_at`

```rust
pub fn address_at(&self,index:&Expr)->Result<Expr,Error>
```

Emit a pointer suitable for Intel subgroup block I/O, e.g. `buffer + index`.  Its type annotation denotes *pointee value* rather than Rust pointer metadata. All lanes in the subgroup must pass the same pointer; the device's extension specification requires this for block operations.

#### `new`

```rust
pub fn new(name:&str,caps:CapabilitySet)->Result<Self,Error>
```

Start a named kernel; validates that the name is an ASCII identifier.

#### `emitter`

```rust
pub const fn emitter(&self)->Emitter
```

Return the matching CPU-side OpenCL expression emitter.

#### `read_only_buffer`

```rust
pub fn read_only_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>
```

Add a `const __global T*` buffer parameter and return a read-only handle.

#### `read_write_buffer`

```rust
pub fn read_write_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>
```

Add a `__global T*` buffer parameter and return a writable handle.

#### `buffer`

```rust
pub fn buffer(&mut self,name:&str,element:Type,access:BufferAccess)->Result<Buffer,Error>
```

Add a typed global-buffer parameter; no allocation occurs on the host.

#### `value`

```rust
pub fn value(&mut self,name:&str,ty:Type)->Result<Expr,Error>
```

Add a scalar/vector value parameter; returns its typed expression.

#### `let_value`

```rust
pub fn let_value(&mut self,name:&str,expression:&Expr)->Result<Expr,Error>
```

Append a local variable holding a GPU operation result.

#### `index_1d`

```rust
pub fn index_1d(&mut self,name:&str)->Result<Expr,Error>
```

Emit the one-dimensional global work-item ID as a local unsigned integer.  OpenCL's `size_t` ID is converted to `uint`, so launch ranges must fit in 32 bits for code produced by this convenience method.

#### `store`

```rust
pub fn store(&mut self,buffer:&Buffer,index:&Expr,value:&Expr)->Result<(),Error>
```

Write a typed expression to a writable buffer element.

#### `statement`

```rust
pub fn statement(&mut self,expression:&Expr)
```

Append a validated expression statement, including a barrier or block store.  `Expr::from_source_unchecked` is not safe for untrusted shader text.

#### `return_if_out_of_bounds`

```rust
pub fn return_if_out_of_bounds(&mut self,index:&Expr,limit:&Expr)->Result<(),Error>
```

Append a bounds test that returns early from the kernel when out of range.

#### `build`

```rust
pub fn build(self)->Result<String,Error>
```

Produce complete Intel OpenCL C source with opt-in extension pragmas.  Requests for extensions are **not** runtime capability discovery. Check `CL_DEVICE_EXTENSIONS` and OpenCL version before building the kernel.

### `src/nvidia.rs` — 17 exported functions

#### `spelling`

```rust
pub fn spelling(op:Intrinsic)->Option<&'static str>
```

NVIDIA CUDA-C++ mapping for operations with compatible semantics and signatures.  A missing mapping is deliberate: CUDA float vectors are structs (unlike shader SIMD values), and texture/sampler or generic subgroup intrinsics do not share a universally equivalent CUDA expression form.

#### `name`

```rust
pub fn name(&self)->&str
```

Name of the C++ kernel parameter.

#### `element_type`

```rust
pub const fn element_type(&self)->Type
```

Element type of the device allocation.

#### `access`

```rust
pub const fn access(&self)->BufferAccess
```

Whether the kernel can write this buffer through the handle.

#### `load`

```rust
pub fn load(&self,index:&Expr)->Result<Expr,Error>
```

Generate a typed indexed load, without accessing host/GPU memory.

#### `atomic_address_at`

```rust
pub fn atomic_address_at(&self,index:&Expr)->Result<Expr,Error>
```

Generate an atomic-pointer expression suitable for CUDA's `atomicAdd` etc.  Caller must ensure alignment, valid pointer scope and device support.

#### `new`

```rust
pub fn new(name:&str,caps:CapabilitySet)->Result<Self,Error>
```

Create a CUDA kernel declaration using a valid unqualified identifier.

#### `emitter`

```rust
pub const fn emitter(&self)->Emitter
```

Obtain a matching expression emitter.

#### `read_only_buffer`

```rust
pub fn read_only_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>
```

Add read-only global buffer argument.

#### `read_write_buffer`

```rust
pub fn read_write_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>
```

Add writable global buffer argument.

#### `value`

```rust
pub fn value(&mut self,name:&str,ty:Type)->Result<Expr,Error>
```

Add a scalar by-value kernel argument.

#### `let_value`

```rust
pub fn let_value(&mut self,name:&str,expr:&Expr)->Result<Expr,Error>
```

Declare a typed local variable from an expression.

#### `index_1d`

```rust
pub fn index_1d(&mut self,name:&str)->Result<Expr,Error>
```

Generate 1D global index `blockIdx.x * blockDim.x + threadIdx.x`.  Arithmetic uses 32-bit unsigned integers; launches must avoid overflow.

#### `store`

```rust
pub fn store(&mut self,buffer:&Buffer,index:&Expr,value:&Expr)->Result<(),Error>
```

Add an indexed global-memory store, validating access and element type.

#### `statement`

```rust
pub fn statement(&mut self,expression:&Expr)
```

Emit a statement using the source of a previously validated expression.

#### `return_if_out_of_bounds`

```rust
pub fn return_if_out_of_bounds(&mut self,index:&Expr,limit:&Expr)->Result<(),Error>
```

Return from a kernel if `index >= limit` (uniformity must be considered by the caller before inserting workgroup or warp barriers).

#### `build`

```rust
pub fn build(self)->Result<String,Error>
```

Return a fully formed CUDA C++ `extern "C" __global__` function definition.  This text does not include CUDA runtime registration code, device memory allocation, an NVRTC compiler call, or a launch configuration.

### `src/amd.rs` — 17 exported functions

#### `spelling`

```rust
pub fn spelling(op:Intrinsic)->Option<&'static str>
```

Native HIP name for a supported operation, including AMD extensions. Returns `None` for operations without a proven HIP lowering.  Most scalar math and integer atomics are shared with the CUDA-compatible HIP API. NVIDIA-only operations, textures, and unmodeled vector overloads are excluded deliberately.

#### `name`

```rust
pub fn name(&self)->&str
```

Get the emitted parameter identifier.

#### `element_type`

```rust
pub const fn element_type(&self)->Type
```

Get the buffer's element type.

#### `access`

```rust
pub const fn access(&self)->BufferAccess
```

Get whether the device kernel may write through this handle.

#### `load`

```rust
pub fn load(&self,index:&Expr)->Result<Expr,Error>
```

Emit indexed load, without touching host or device memory.

#### `atomic_address_at`

```rust
pub fn atomic_address_at(&self,index:&Expr)->Result<Expr,Error>
```

Emit a writable integer-atomic address expression for HIP's atomic API.

#### `new`

```rust
pub fn new(name:&str,caps:CapabilitySet)->Result<Self,Error>
```

Construct a HIP kernel with a valid unqualified name.

#### `emitter`

```rust
pub const fn emitter(&self)->Emitter
```

Return a configured compute-stage AMD HIP expression emitter.

#### `read_only_buffer`

```rust
pub fn read_only_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>
```

Declare a read-only pointer-to-device-buffer argument.

#### `read_write_buffer`

```rust
pub fn read_write_buffer(&mut self,name:&str,element:Type)->Result<Buffer,Error>
```

Declare a writable pointer-to-device-buffer argument.

#### `value`

```rust
pub fn value(&mut self,name:&str,ty:Type)->Result<Expr,Error>
```

Declare scalar kernel parameter with the requested type.

#### `let_value`

```rust
pub fn let_value(&mut self,name:&str,expr:&Expr)->Result<Expr,Error>
```

Declare a local scalar with a validated typed initializer.

#### `index_1d`

```rust
pub fn index_1d(&mut self,name:&str)->Result<Expr,Error>
```

Generate a 1D global thread index (`blockIdx.x * blockDim.x + threadIdx.x`).

#### `store`

```rust
pub fn store(&mut self,buffer:&Buffer,index:&Expr,value:&Expr)->Result<(),Error>
```

Emit indexed store after validating access permissions and element type.

#### `statement`

```rust
pub fn statement(&mut self,expr:&Expr)
```

Append a prevalidated expression as a HIP statement.

#### `return_if_out_of_bounds`

```rust
pub fn return_if_out_of_bounds(&mut self,index:&Expr,limit:&Expr)->Result<(),Error>
```

Emit early-return guard (`if index >= limit`) for bounds checking.  Do not place group or wave barriers after divergent early returns.

#### `build`

```rust
pub fn build(self)->Result<String,Error>
```

Return HIP C++ kernel source with the HIP runtime header included.  Compiles independently of the host GPU, but compiling this returned *device source* needs a compatible HIP compiler/toolchain at runtime.

### `src/ops.rs` — 322 exported functions

#### `abs`

```rust
pub fn abs(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Abs` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `sign`

```rust
pub fn sign(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Sign` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `sin`

```rust
pub fn sin(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Sin` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `cos`

```rust
pub fn cos(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Cos` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `tan`

```rust
pub fn tan(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Tan` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `asin`

```rust
pub fn asin(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Asin` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `acos`

```rust
pub fn acos(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Acos` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atan`

```rust
pub fn atan(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Atan` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `sinh`

```rust
pub fn sinh(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Sinh` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `cosh`

```rust
pub fn cosh(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Cosh` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `tanh`

```rust
pub fn tanh(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Tanh` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `asinh`

```rust
pub fn asinh(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Asinh` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `acosh`

```rust
pub fn acosh(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Acosh` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atanh`

```rust
pub fn atanh(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Atanh` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `exp`

```rust
pub fn exp(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Exp` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `exp2`

```rust
pub fn exp2(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Exp2` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `log`

```rust
pub fn log(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Log` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `log2`

```rust
pub fn log2(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Log2` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `sqrt`

```rust
pub fn sqrt(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Sqrt` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `inverse_sqrt`

```rust
pub fn inverse_sqrt(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `InverseSqrt` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `floor`

```rust
pub fn floor(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Floor` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `ceil`

```rust
pub fn ceil(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Ceil` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `trunc`

```rust
pub fn trunc(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Trunc` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `round`

```rust
pub fn round(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Round` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `fract`

```rust
pub fn fract(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Fract` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `degrees`

```rust
pub fn degrees(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Degrees` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `radians`

```rust
pub fn radians(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Radians` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pow`

```rust
pub fn pow(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pow` (Math family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atan2`

```rust
pub fn atan2(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Atan2` (Math family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `min`

```rust
pub fn min(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Min` (Math family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `max`

```rust
pub fn max(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Max` (Math family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `clamp`

```rust
pub fn clamp(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Clamp` (Math family; 3–3 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `fma`

```rust
pub fn fma(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Fma` (Math family; 3–3 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `saturate`

```rust
pub fn saturate(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Saturate` (Math family; 1–1 arguments). Shader targets with a native mapping: HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `mix`

```rust
pub fn mix(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Mix` (Math family; 3–3 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `step`

```rust
pub fn step(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Step` (Math family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `smoothstep`

```rust
pub fn smoothstep(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Smoothstep` (Math family; 3–3 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `modf`

```rust
pub fn modf(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `Modf` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `frexp`

```rust
pub fn frexp(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `Frexp` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `ldexp`

```rust
pub fn ldexp(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `Ldexp` (Math family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `quantize_to_f16`

```rust
pub fn quantize_to_f16(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `QuantizeToF16` (Math family; 1–1 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `length`

```rust
pub fn length(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Length` (Geometry family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `distance`

```rust
pub fn distance(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Distance` (Geometry family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `dot`

```rust
pub fn dot(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Dot` (Geometry family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `cross`

```rust
pub fn cross(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Cross` (Geometry family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `normalize`

```rust
pub fn normalize(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Normalize` (Geometry family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `reflect`

```rust
pub fn reflect(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Reflect` (Geometry family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `refract`

```rust
pub fn refract(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Refract` (Geometry family; 3–3 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `face_forward`

```rust
pub fn face_forward(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `FaceForward` (Geometry family; 3–3 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `transpose`

```rust
pub fn transpose(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Transpose` (Matrix family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `determinant`

```rust
pub fn determinant(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Determinant` (Matrix family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `inverse`

```rust
pub fn inverse(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Inverse` (Matrix family; 1–1 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `add`

```rust
pub fn add(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Add` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `sub`

```rust
pub fn sub(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Sub` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `multiply`

```rust
pub fn multiply(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Multiply` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `divide`

```rust
pub fn divide(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Divide` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `modulo`

```rust
pub fn modulo(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Modulo` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `negate`

```rust
pub fn negate(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Negate` (Operator family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `equal`

```rust
pub fn equal(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Equal` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `not_equal`

```rust
pub fn not_equal(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `NotEqual` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `less`

```rust
pub fn less(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Less` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `less_equal`

```rust
pub fn less_equal(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `LessEqual` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `greater`

```rust
pub fn greater(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Greater` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `greater_equal`

```rust
pub fn greater_equal(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `GreaterEqual` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `logical_and`

```rust
pub fn logical_and(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `LogicalAnd` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `logical_or`

```rust
pub fn logical_or(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `LogicalOr` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `logical_not`

```rust
pub fn logical_not(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `LogicalNot` (Operator family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `bit_and`

```rust
pub fn bit_and(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `BitAnd` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `bit_or`

```rust
pub fn bit_or(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `BitOr` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `bit_xor`

```rust
pub fn bit_xor(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `BitXor` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `shift_left`

```rust
pub fn shift_left(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `ShiftLeft` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `shift_right`

```rust
pub fn shift_right(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `ShiftRight` (Operator family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `bit_not`

```rust
pub fn bit_not(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `BitNot` (Operator family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `select`

```rust
pub fn select(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Select` (Selection family; 3–3 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `all`

```rust
pub fn all(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `All` (Comparison family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `any`

```rust
pub fn any(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Any` (Comparison family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `count_one_bits`

```rust
pub fn count_one_bits(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `CountOneBits` (Bits family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `reverse_bits`

```rust
pub fn reverse_bits(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `ReverseBits` (Bits family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `first_leading_bit`

```rust
pub fn first_leading_bit(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `FirstLeadingBit` (Bits family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `first_trailing_bit`

```rust
pub fn first_trailing_bit(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `FirstTrailingBit` (Bits family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `count_leading_zeros`

```rust
pub fn count_leading_zeros(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `CountLeadingZeros` (Bits family; 1–1 arguments). Shader targets with a native mapping: WGSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `count_trailing_zeros`

```rust
pub fn count_trailing_zeros(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `CountTrailingZeros` (Bits family; 1–1 arguments). Shader targets with a native mapping: WGSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `find_lsb`

```rust
pub fn find_lsb(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `FindLsb` (Bits family; 1–1 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `find_msb`

```rust
pub fn find_msb(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `FindMsb` (Bits family; 1–1 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `extract_bits`

```rust
pub fn extract_bits(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `ExtractBits` (Bits family; 3–3 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `insert_bits`

```rust
pub fn insert_bits(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `InsertBits` (Bits family; 4–4 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `rotate_left`

```rust
pub fn rotate_left(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `RotateLeft` (Bits family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `rotate_right`

```rust
pub fn rotate_right(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `RotateRight` (Bits family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `bitfield_mask`

```rust
pub fn bitfield_mask(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `BitfieldMask` (Bits family; 2–2 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `bitfield_reverse`

```rust
pub fn bitfield_reverse(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `BitfieldReverse` (Bits family; 1–1 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `bitfield_extract_signed`

```rust
pub fn bitfield_extract_signed(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `BitfieldExtractSigned` (Bits family; 3–4 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `bitfield_extract_unsigned`

```rust
pub fn bitfield_extract_unsigned(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `BitfieldExtractUnsigned` (Bits family; 3–4 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `bitfield_insert_signed`

```rust
pub fn bitfield_insert_signed(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `BitfieldInsertSigned` (Bits family; 3–4 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `bitfield_count`

```rust
pub fn bitfield_count(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `BitfieldCount` (Bits family; 1–1 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `find_first_set`

```rust
pub fn find_first_set(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `FindFirstSet` (Bits family; 1–1 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `find_last_set`

```rust
pub fn find_last_set(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `FindLastSet` (Bits family; 1–1 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack4x8_unorm`

```rust
pub fn pack4x8_unorm(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pack4x8Unorm` (Packing family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack4x8_snorm`

```rust
pub fn pack4x8_snorm(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pack4x8Snorm` (Packing family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack2x16_unorm`

```rust
pub fn pack2x16_unorm(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pack2x16Unorm` (Packing family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack2x16_snorm`

```rust
pub fn pack2x16_snorm(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pack2x16Snorm` (Packing family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack2x16_float`

```rust
pub fn pack2x16_float(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pack2x16Float` (Packing family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack4x_i8`

```rust
pub fn pack4x_i8(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pack4xI8` (Packing family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack4x_u8`

```rust
pub fn pack4x_u8(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pack4xU8` (Packing family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack4x_i8_clamp`

```rust
pub fn pack4x_i8_clamp(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pack4xI8Clamp` (Packing family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack4x_u8_clamp`

```rust
pub fn pack4x_u8_clamp(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pack4xU8Clamp` (Packing family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack4x_i8_saturate`

```rust
pub fn pack4x_i8_saturate(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pack4xI8Saturate` (Packing family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack4x_u8_saturate`

```rust
pub fn pack4x_u8_saturate(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Pack4xU8Saturate` (Packing family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack4x8_unorm`

```rust
pub fn unpack4x8_unorm(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Unpack4x8Unorm` (Packing family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack4x8_snorm`

```rust
pub fn unpack4x8_snorm(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Unpack4x8Snorm` (Packing family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack2x16_unorm`

```rust
pub fn unpack2x16_unorm(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Unpack2x16Unorm` (Packing family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack2x16_snorm`

```rust
pub fn unpack2x16_snorm(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Unpack2x16Snorm` (Packing family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack2x16_float`

```rust
pub fn unpack2x16_float(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Unpack2x16Float` (Packing family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack4x_i8`

```rust
pub fn unpack4x_i8(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Unpack4xI8` (Packing family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack4x_u8`

```rust
pub fn unpack4x_u8(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Unpack4xU8` (Packing family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack_snorm2x16`

```rust
pub fn pack_snorm2x16(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `PackSnorm2x16` (Packing family; 1–1 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack_unorm2x16`

```rust
pub fn pack_unorm2x16(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `PackUnorm2x16` (Packing family; 1–1 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack_half2x16`

```rust
pub fn pack_half2x16(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `PackHalf2x16` (Packing family; 1–1 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack_snorm4x8`

```rust
pub fn pack_snorm4x8(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `PackSnorm4x8` (Packing family; 1–1 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `pack_unorm4x8`

```rust
pub fn pack_unorm4x8(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `PackUnorm4x8` (Packing family; 1–1 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack_snorm2x16`

```rust
pub fn unpack_snorm2x16(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `UnpackSnorm2x16` (Packing family; 1–1 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack_unorm2x16`

```rust
pub fn unpack_unorm2x16(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `UnpackUnorm2x16` (Packing family; 1–1 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack_half2x16`

```rust
pub fn unpack_half2x16(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `UnpackHalf2x16` (Packing family; 1–1 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack_snorm4x8`

```rust
pub fn unpack_snorm4x8(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `UnpackSnorm4x8` (Packing family; 1–1 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `unpack_unorm4x8`

```rust
pub fn unpack_unorm4x8(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `UnpackUnorm4x8` (Packing family; 1–1 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `dpdx`

```rust
pub fn dpdx(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Dpdx` (Derivative family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `dpdy`

```rust
pub fn dpdy(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Dpdy` (Derivative family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `fwidth`

```rust
pub fn fwidth(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `Fwidth` (Derivative family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `dpdx_fine`

```rust
pub fn dpdx_fine(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `DpdxFine` (Derivative family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `dpdy_fine`

```rust
pub fn dpdy_fine(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `DpdyFine` (Derivative family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `fwidth_fine`

```rust
pub fn fwidth_fine(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `FwidthFine` (Derivative family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `dpdx_coarse`

```rust
pub fn dpdx_coarse(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `DpdxCoarse` (Derivative family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `dpdy_coarse`

```rust
pub fn dpdy_coarse(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `DpdyCoarse` (Derivative family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `fwidth_coarse`

```rust
pub fn fwidth_coarse(emitter:&Emitter,args:&[Expr])->Result<Expr,Error>
```

Emit `FwidthCoarse` (Derivative family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `interpolate_at_centroid`

```rust
pub fn interpolate_at_centroid(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `InterpolateAtCentroid` (Derivative family; 1–2 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `interpolate_at_sample`

```rust
pub fn interpolate_at_sample(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `InterpolateAtSample` (Derivative family; 1–2 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `interpolate_at_offset`

```rust
pub fn interpolate_at_offset(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `InterpolateAtOffset` (Derivative family; 1–2 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_dimensions`

```rust
pub fn texture_dimensions(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureDimensions` (Texture family; 1–2 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_num_layers`

```rust
pub fn texture_num_layers(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureNumLayers` (Texture family; 1–1 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_num_levels`

```rust
pub fn texture_num_levels(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureNumLevels` (Texture family; 1–1 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_num_samples`

```rust
pub fn texture_num_samples(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureNumSamples` (Texture family; 1–1 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_load`

```rust
pub fn texture_load(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureLoad` (Texture family; 2–4 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_store`

```rust
pub fn texture_store(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureStore` (Texture family; 3–4 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_sample`

```rust
pub fn texture_sample(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureSample` (Texture family; 3–5 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_sample_bias`

```rust
pub fn texture_sample_bias(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureSampleBias` (Texture family; 4–6 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_sample_level`

```rust
pub fn texture_sample_level(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureSampleLevel` (Texture family; 4–6 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_sample_grad`

```rust
pub fn texture_sample_grad(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureSampleGrad` (Texture family; 5–7 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_sample_compare`

```rust
pub fn texture_sample_compare(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureSampleCompare` (Texture family; 4–6 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_sample_compare_level`

```rust
pub fn texture_sample_compare_level(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureSampleCompareLevel` (Texture family; 4–6 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_gather`

```rust
pub fn texture_gather(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureGather` (Texture family; 3–5 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_gather_compare`

```rust
pub fn texture_gather_compare(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureGatherCompare` (Texture family; 4–6 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_sample_base_clamp_to_edge`

```rust
pub fn texture_sample_base_clamp_to_edge(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureSampleBaseClampToEdge` (Texture family; 3–3 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_sample_compare_bias`

```rust
pub fn texture_sample_compare_bias(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureSampleCompareBias` (Texture family; 1–6 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_sample_compare_grad`

```rust
pub fn texture_sample_compare_grad(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureSampleCompareGrad` (Texture family; 1–6 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_gather_red`

```rust
pub fn texture_gather_red(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureGatherRed` (Texture family; 1–6 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_gather_green`

```rust
pub fn texture_gather_green(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureGatherGreen` (Texture family; 1–6 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_gather_blue`

```rust
pub fn texture_gather_blue(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureGatherBlue` (Texture family; 1–6 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_gather_alpha`

```rust
pub fn texture_gather_alpha(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureGatherAlpha` (Texture family; 1–6 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_fetch`

```rust
pub fn texture_fetch(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureFetch` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_fetch_offset`

```rust
pub fn texture_fetch_offset(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureFetchOffset` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_fetch_lod`

```rust
pub fn texture_fetch_lod(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureFetchLod` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_fetch_lod_offset`

```rust
pub fn texture_fetch_lod_offset(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureFetchLodOffset` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_query_lod`

```rust
pub fn texture_query_lod(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureQueryLod` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_query_levels`

```rust
pub fn texture_query_levels(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureQueryLevels` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_query_samples`

```rust
pub fn texture_query_samples(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureQuerySamples` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_size`

```rust
pub fn texture_size(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureSize` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_samples`

```rust
pub fn texture_samples(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureSamples` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_offset`

```rust
pub fn texture_offset(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureOffset` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_proj`

```rust
pub fn texture_proj(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureProj` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_proj_lod`

```rust
pub fn texture_proj_lod(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureProjLod` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_proj_grad`

```rust
pub fn texture_proj_grad(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureProjGrad` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_grad`

```rust
pub fn texture_grad(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureGrad` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_lod`

```rust
pub fn texture_lod(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureLod` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_gather_offset`

```rust
pub fn texture_gather_offset(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureGatherOffset` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_gather_offsets`

```rust
pub fn texture_gather_offsets(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureGatherOffsets` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_gather_compare_offset`

```rust
pub fn texture_gather_compare_offset(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureGatherCompareOffset` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_gather_compare_offsets`

```rust
pub fn texture_gather_compare_offsets(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `TextureGatherCompareOffsets` (Texture family; 1–6 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_load`

```rust
pub fn atomic_load(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicLoad` (Atomic family; 1–1 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_store`

```rust
pub fn atomic_store(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicStore` (Atomic family; 2–2 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_add`

```rust
pub fn atomic_add(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicAdd` (Atomic family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_sub`

```rust
pub fn atomic_sub(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicSub` (Atomic family; 2–2 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_max`

```rust
pub fn atomic_max(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicMax` (Atomic family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_min`

```rust
pub fn atomic_min(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicMin` (Atomic family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_and`

```rust
pub fn atomic_and(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicAnd` (Atomic family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_or`

```rust
pub fn atomic_or(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicOr` (Atomic family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_xor`

```rust
pub fn atomic_xor(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicXor` (Atomic family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_exchange`

```rust
pub fn atomic_exchange(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicExchange` (Atomic family; 2–2 arguments). Shader targets with a native mapping: WGSL, GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_compare_exchange_weak`

```rust
pub fn atomic_compare_exchange_weak(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicCompareExchangeWeak` (Atomic family; 3–3 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_comp_swap`

```rust
pub fn atomic_comp_swap(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicCompSwap` (Atomic family; 1–4 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_inc`

```rust
pub fn atomic_inc(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicInc` (Atomic family; 1–4 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_dec`

```rust
pub fn atomic_dec(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicDec` (Atomic family; 1–4 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_counter`

```rust
pub fn atomic_counter(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicCounter` (Atomic family; 1–4 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_counter_increment`

```rust
pub fn atomic_counter_increment(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicCounterIncrement` (Atomic family; 1–4 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `atomic_counter_decrement`

```rust
pub fn atomic_counter_decrement(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AtomicCounterDecrement` (Atomic family; 1–4 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `workgroup_barrier`

```rust
pub fn workgroup_barrier(emitter:&Emitter)->Result<Expr,Error>
```

Emit `WorkgroupBarrier` (Barrier family; 0–0 arguments). Shader targets with a native mapping: WGSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `storage_barrier`

```rust
pub fn storage_barrier(emitter:&Emitter)->Result<Expr,Error>
```

Emit `StorageBarrier` (Barrier family; 0–0 arguments). Shader targets with a native mapping: WGSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `texture_barrier`

```rust
pub fn texture_barrier(emitter:&Emitter)->Result<Expr,Error>
```

Emit `TextureBarrier` (Barrier family; 0–0 arguments). Shader targets with a native mapping: WGSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `barrier`

```rust
pub fn barrier(emitter:&Emitter)->Result<Expr,Error>
```

Emit `Barrier` (Barrier family; 0–0 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `memory_barrier`

```rust
pub fn memory_barrier(emitter:&Emitter)->Result<Expr,Error>
```

Emit `MemoryBarrier` (Barrier family; 0–0 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `memory_barrier_buffer`

```rust
pub fn memory_barrier_buffer(emitter:&Emitter)->Result<Expr,Error>
```

Emit `MemoryBarrierBuffer` (Barrier family; 0–0 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `memory_barrier_image`

```rust
pub fn memory_barrier_image(emitter:&Emitter)->Result<Expr,Error>
```

Emit `MemoryBarrierImage` (Barrier family; 0–0 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `group_memory_barrier`

```rust
pub fn group_memory_barrier(emitter:&Emitter)->Result<Expr,Error>
```

Emit `GroupMemoryBarrier` (Barrier family; 0–0 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `device_memory_barrier`

```rust
pub fn device_memory_barrier(emitter:&Emitter)->Result<Expr,Error>
```

Emit `DeviceMemoryBarrier` (Barrier family; 0–0 arguments). Shader targets with a native mapping: HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `all_memory_barrier`

```rust
pub fn all_memory_barrier(emitter:&Emitter)->Result<Expr,Error>
```

Emit `AllMemoryBarrier` (Barrier family; 0–0 arguments). Shader targets with a native mapping: HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_size`

```rust
pub fn subgroup_size(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupSize` (Subgroup family; 0–0 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_invocation_id`

```rust
pub fn subgroup_invocation_id(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupInvocationId` (Subgroup family; 0–0 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_elect`

```rust
pub fn subgroup_elect(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupElect` (Subgroup family; 0–0 arguments). Shader targets with a native mapping: GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_broadcast`

```rust
pub fn subgroup_broadcast(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupBroadcast` (Subgroup family; 2–2 arguments). Shader targets with a native mapping: GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_broadcast_first`

```rust
pub fn subgroup_broadcast_first(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupBroadcastFirst` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_shuffle`

```rust
pub fn subgroup_shuffle(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupShuffle` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_shuffle_xor`

```rust
pub fn subgroup_shuffle_xor(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupShuffleXor` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_shuffle_up`

```rust
pub fn subgroup_shuffle_up(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupShuffleUp` (Subgroup family; 3–3 arguments). Shader targets with a native mapping: GLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_shuffle_down`

```rust
pub fn subgroup_shuffle_down(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupShuffleDown` (Subgroup family; 3–3 arguments). Shader targets with a native mapping: GLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_all`

```rust
pub fn subgroup_all(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupAll` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_any`

```rust
pub fn subgroup_any(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupAny` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_ballot`

```rust
pub fn subgroup_ballot(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupBallot` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_all_equal`

```rust
pub fn subgroup_all_equal(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupAllEqual` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_add`

```rust
pub fn subgroup_add(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupAdd` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_mul`

```rust
pub fn subgroup_mul(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupMul` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_min`

```rust
pub fn subgroup_min(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupMin` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_max`

```rust
pub fn subgroup_max(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupMax` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_and`

```rust
pub fn subgroup_and(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupAnd` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_or`

```rust
pub fn subgroup_or(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupOr` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_xor`

```rust
pub fn subgroup_xor(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupXor` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_inclusive_add`

```rust
pub fn subgroup_inclusive_add(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupInclusiveAdd` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_inclusive_mul`

```rust
pub fn subgroup_inclusive_mul(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupInclusiveMul` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_inclusive_min`

```rust
pub fn subgroup_inclusive_min(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupInclusiveMin` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_inclusive_max`

```rust
pub fn subgroup_inclusive_max(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupInclusiveMax` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_exclusive_add`

```rust
pub fn subgroup_exclusive_add(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupExclusiveAdd` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_exclusive_mul`

```rust
pub fn subgroup_exclusive_mul(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupExclusiveMul` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_exclusive_min`

```rust
pub fn subgroup_exclusive_min(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupExclusiveMin` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_exclusive_max`

```rust
pub fn subgroup_exclusive_max(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupExclusiveMax` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_quad_broadcast`

```rust
pub fn subgroup_quad_broadcast(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupQuadBroadcast` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL, HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_quad_swap_x`

```rust
pub fn subgroup_quad_swap_x(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupQuadSwapX` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_quad_swap_y`

```rust
pub fn subgroup_quad_swap_y(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupQuadSwapY` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_quad_swap_diagonal`

```rust
pub fn subgroup_quad_swap_diagonal(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupQuadSwapDiagonal` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_clustered_add`

```rust
pub fn subgroup_clustered_add(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupClusteredAdd` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_clustered_mul`

```rust
pub fn subgroup_clustered_mul(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupClusteredMul` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_clustered_min`

```rust
pub fn subgroup_clustered_min(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupClusteredMin` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_clustered_max`

```rust
pub fn subgroup_clustered_max(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupClusteredMax` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: GLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_partitioned_add`

```rust
pub fn subgroup_partitioned_add(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupPartitionedAdd` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_partitioned_inclusive_add`

```rust
pub fn subgroup_partitioned_inclusive_add(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupPartitionedInclusiveAdd` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `subgroup_partitioned_exclusive_add`

```rust
pub fn subgroup_partitioned_exclusive_add(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SubgroupPartitionedExclusiveAdd` (Subgroup family; 0–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `discard`

```rust
pub fn discard(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `Discard` (Utility family; 1–4 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `clip`

```rust
pub fn clip(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `Clip` (Utility family; 1–1 arguments). Shader targets with a native mapping: HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `sin_cos`

```rust
pub fn sin_cos(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `SinCos` (Utility family; 1–4 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `ms_ad4`

```rust
pub fn ms_ad4(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `MsAd4` (Vendor family; 1–4 arguments). Shader targets with a native mapping: HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `dot4_add_i8_packed`

```rust
pub fn dot4_add_i8_packed(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `Dot4AddI8Packed` (Vendor family; 3–3 arguments). Shader targets with a native mapping: HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `dot4_add_u8_packed`

```rust
pub fn dot4_add_u8_packed(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `Dot4AddU8Packed` (Vendor family; 3–3 arguments). Shader targets with a native mapping: HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `non_uniform_index`

```rust
pub fn non_uniform_index(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NonUniformIndex` (Utility family; 1–1 arguments). Shader targets with a native mapping: HLSL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `as_float`

```rust
pub fn as_float(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AsFloat` (Utility family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `as_u_int`

```rust
pub fn as_u_int(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AsUInt` (Utility family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `as_int`

```rust
pub fn as_int(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AsInt` (Utility family; 1–1 arguments). Shader targets with a native mapping: WGSL, GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `is_nan`

```rust
pub fn is_nan(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IsNan` (Utility family; 1–1 arguments). Shader targets with a native mapping: GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `is_inf`

```rust
pub fn is_inf(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IsInf` (Utility family; 1–1 arguments). Shader targets with a native mapping: GLSL, HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `is_finite`

```rust
pub fn is_finite(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IsFinite` (Utility family; 1–1 arguments). Shader targets with a native mapping: HLSL, Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_read`

```rust
pub fn intel_sub_group_block_read(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockRead` (Vendor family; 1–1 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_read2`

```rust
pub fn intel_sub_group_block_read2(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockRead2` (Vendor family; 1–1 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_read4`

```rust
pub fn intel_sub_group_block_read4(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockRead4` (Vendor family; 1–1 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_read8`

```rust
pub fn intel_sub_group_block_read8(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockRead8` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_write`

```rust
pub fn intel_sub_group_block_write(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockWrite` (Vendor family; 2–2 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_write2`

```rust
pub fn intel_sub_group_block_write2(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockWrite2` (Vendor family; 2–2 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_write4`

```rust
pub fn intel_sub_group_block_write4(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockWrite4` (Vendor family; 2–2 arguments). Shader targets with a native mapping: Intel OpenCL. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_write8`

```rust
pub fn intel_sub_group_block_write8(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockWrite8` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_read_us`

```rust
pub fn intel_sub_group_block_read_us(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockReadUs` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_read_us2`

```rust
pub fn intel_sub_group_block_read_us2(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockReadUs2` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_read_us4`

```rust
pub fn intel_sub_group_block_read_us4(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockReadUs4` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_read_us8`

```rust
pub fn intel_sub_group_block_read_us8(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockReadUs8` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_write_us`

```rust
pub fn intel_sub_group_block_write_us(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockWriteUs` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_write_us2`

```rust
pub fn intel_sub_group_block_write_us2(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockWriteUs2` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_write_us4`

```rust
pub fn intel_sub_group_block_write_us4(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockWriteUs4` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `intel_sub_group_block_write_us8`

```rust
pub fn intel_sub_group_block_write_us8(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `IntelSubGroupBlockWriteUs8` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_active_mask`

```rust
pub fn nvidia_active_mask(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaActiveMask` (Vendor family; 0–0 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_ballot_sync`

```rust
pub fn nvidia_ballot_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaBallotSync` (Vendor family; 2–2 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_any_sync`

```rust
pub fn nvidia_any_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaAnySync` (Vendor family; 2–2 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_all_sync`

```rust
pub fn nvidia_all_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaAllSync` (Vendor family; 2–2 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_shuffle_sync`

```rust
pub fn nvidia_shuffle_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaShuffleSync` (Vendor family; 3–4 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_shuffle_up_sync`

```rust
pub fn nvidia_shuffle_up_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaShuffleUpSync` (Vendor family; 3–4 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_shuffle_down_sync`

```rust
pub fn nvidia_shuffle_down_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaShuffleDownSync` (Vendor family; 3–4 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_shuffle_xor_sync`

```rust
pub fn nvidia_shuffle_xor_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaShuffleXorSync` (Vendor family; 3–4 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_sync_warp`

```rust
pub fn nvidia_sync_warp(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaSyncWarp` (Vendor family; 1–1 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_match_any_sync`

```rust
pub fn nvidia_match_any_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaMatchAnySync` (Vendor family; 2–2 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_reduce_add_sync`

```rust
pub fn nvidia_reduce_add_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaReduceAddSync` (Vendor family; 2–2 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_reduce_min_sync`

```rust
pub fn nvidia_reduce_min_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaReduceMinSync` (Vendor family; 2–2 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_reduce_max_sync`

```rust
pub fn nvidia_reduce_max_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaReduceMaxSync` (Vendor family; 2–2 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_reduce_and_sync`

```rust
pub fn nvidia_reduce_and_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaReduceAndSync` (Vendor family; 2–2 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_reduce_or_sync`

```rust
pub fn nvidia_reduce_or_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaReduceOrSync` (Vendor family; 2–2 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `nvidia_reduce_xor_sync`

```rust
pub fn nvidia_reduce_xor_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `NvidiaReduceXorSync` (Vendor family; 2–2 arguments). Shader targets with a native mapping: NVIDIA CUDA. Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_wave_size`

```rust
pub fn amd_wave_size(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdWaveSize` (Vendor family; 0–0 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_lane_id`

```rust
pub fn amd_lane_id(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdLaneId` (Vendor family; 0–0 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_shuffle`

```rust
pub fn amd_shuffle(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdShuffle` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_shuffle_xor`

```rust
pub fn amd_shuffle_xor(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdShuffleXor` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_shuffle_up`

```rust
pub fn amd_shuffle_up(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdShuffleUp` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_shuffle_down`

```rust
pub fn amd_shuffle_down(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdShuffleDown` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_ballot`

```rust
pub fn amd_ballot(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdBallot` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_active_mask`

```rust
pub fn amd_active_mask(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdActiveMask` (Vendor family; 0–0 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_any`

```rust
pub fn amd_any(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdAny` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_all`

```rust
pub fn amd_all(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdAll` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_wave_sync`

```rust
pub fn amd_wave_sync(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdWaveSync` (Vendor family; 0–0 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_read_first_lane`

```rust
pub fn amd_read_first_lane(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdReadFirstLane` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_read_lane`

```rust
pub fn amd_read_lane(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdReadLane` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_write_lane`

```rust
pub fn amd_write_lane(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdWriteLane` (Vendor family; 3–3 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_ds_bpermute`

```rust
pub fn amd_ds_bpermute(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdDsBpermute` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_ds_permute`

```rust
pub fn amd_ds_permute(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdDsPermute` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_fast_sin`

```rust
pub fn amd_fast_sin(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdFastSin` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_fast_cos`

```rust
pub fn amd_fast_cos(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdFastCos` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_fast_exp`

```rust
pub fn amd_fast_exp(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdFastExp` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_fast_log`

```rust
pub fn amd_fast_log(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdFastLog` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_fast_log2`

```rust
pub fn amd_fast_log2(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdFastLog2` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_fast_pow`

```rust
pub fn amd_fast_pow(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdFastPow` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_fast_rcp`

```rust
pub fn amd_fast_rcp(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdFastRcp` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_byte_perm`

```rust
pub fn amd_byte_perm(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdBytePerm` (Vendor family; 3–3 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_funnel_shift_left`

```rust
pub fn amd_funnel_shift_left(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdFunnelShiftLeft` (Vendor family; 3–3 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_funnel_shift_right`

```rust
pub fn amd_funnel_shift_right(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdFunnelShiftRight` (Vendor family; 3–3 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_mul24`

```rust
pub fn amd_mul24(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdMul24` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_u_mul24`

```rust
pub fn amd_u_mul24(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdUMul24` (Vendor family; 2–2 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_sad`

```rust
pub fn amd_sad(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdSad` (Vendor family; 3–3 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_u_sad`

```rust
pub fn amd_u_sad(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdUSad` (Vendor family; 3–3 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_popcount64`

```rust
pub fn amd_popcount64(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdPopcount64` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_reverse_bits64`

```rust
pub fn amd_reverse_bits64(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdReverseBits64` (Vendor family; 1–1 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_thread_fence_block`

```rust
pub fn amd_thread_fence_block(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdThreadFenceBlock` (Vendor family; 0–0 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_thread_fence_device`

```rust
pub fn amd_thread_fence_device(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdThreadFenceDevice` (Vendor family; 0–0 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

#### `amd_thread_fence_system`

```rust
pub fn amd_thread_fence_system(emitter:&Emitter,args:&[Expr],return_ty:Type)->Result<Expr,Error>
```

Emit `AmdThreadFenceSystem` (Vendor family; 0–0 arguments). Shader targets with a native mapping: none (metadata only). Returns an error when the selected backend, stage, feature or argument signature is unsupported.

### `src/cpu.rs` — 40 exported functions

#### `new`

```rust
pub const fn new(x:f32,y:f32,z:f32,w:f32)->Self
```

Construct four lanes from individual components.

#### `splat`

```rust
pub const fn splat(x:f32)->Self
```

Broadcast one scalar to each lane.

#### `map`

```rust
pub fn map(self,mut f:impl FnMut(f32)->f32)->Self
```

Transform each lane independently.

#### `zip_map`

```rust
pub fn zip_map(self,b:Self,mut f:impl FnMut(f32,f32)->f32)->Self
```

Combine two vectors lane-by-lane.

#### `dot`

```rust
pub fn dot(self,b:Self)->f32
```

Sum the four pairwise products.

#### `length`

```rust
pub fn length(self)->f32
```

Return the Euclidean norm of four components.

#### `normalize_or_zero`

```rust
pub fn normalize_or_zero(self)->Self
```

Divide by length, or return zero for a zero-length input.

#### `clamp`

```rust
pub fn clamp(self,min:Self,max:Self)->Self
```

Clamp every lane between corresponding minimum and maximum lanes.

#### `abs4`

```rust
pub fn abs4(x:Float4)->Float4
```

Return the absolute values of all lanes.

#### `floor4`

```rust
pub fn floor4(x:Float4)->Float4
```

Return the floor of every lane.

#### `ceil4`

```rust
pub fn ceil4(x:Float4)->Float4
```

Return the ceiling of every lane.

#### `trunc4`

```rust
pub fn trunc4(x:Float4)->Float4
```

Truncate every lane toward zero.

#### `sin4`

```rust
pub fn sin4(x:Float4)->Float4
```

Compute sine on each lane.

#### `cos4`

```rust
pub fn cos4(x:Float4)->Float4
```

Compute cosine on each lane.

#### `tan4`

```rust
pub fn tan4(x:Float4)->Float4
```

Compute tangent on each lane.

#### `exp4`

```rust
pub fn exp4(x:Float4)->Float4
```

Compute base-e exponential on each lane.

#### `log4`

```rust
pub fn log4(x:Float4)->Float4
```

Compute the natural logarithm on each lane.

#### `sqrt4`

```rust
pub fn sqrt4(x:Float4)->Float4
```

Compute square root on each lane.

#### `rsqrt4`

```rust
pub fn rsqrt4(x:Float4)->Float4
```

Compute reciprocal square root on each lane.

#### `fract4`

```rust
pub fn fract4(x:Float4)->Float4
```

Return the fractional part, `x - floor(x)`.

#### `mix4`

```rust
pub fn mix4(a:Float4,b:Float4,t:Float4)->Float4
```

Interpolate lanes using `a * (1-t) + b * t`.

#### `saturate4`

```rust
pub fn saturate4(x:Float4)->Float4
```

Clamp four values to the inclusive interval [0, 1].

#### `smoothstep4`

```rust
pub fn smoothstep4(edge0:Float4,edge1:Float4,x:Float4)->Option<Float4>
```

Apply the smoothstep polynomial in each lane; equal edges are rejected.

#### `cross3`

```rust
pub fn cross3(a:Float4,b:Float4)->Float4
```

Return the cross product of the XYZ components and zero in W.

#### `reflect4`

```rust
pub fn reflect4(i:Float4,n:Float4)->Float4
```

Reflect incident vector `i` around a unit normal `n`.

#### `refract4`

```rust
pub fn refract4(i:Float4,n:Float4,eta:f32)->Float4
```

Refract incident vector using refraction ratio `eta`; zero on total internal reflection.

#### `pack_unorm8`

```rust
pub fn pack_unorm8(x:f32)->u8
```

Saturating conversion of an `f32` to an 8-bit unsigned normalized value.

#### `unpack_unorm8`

```rust
pub fn unpack_unorm8(x:u8)->f32
```

Interpret an unsigned normalized byte in the range [0,1].

#### `pack4x8_unorm`

```rust
pub fn pack4x8_unorm(x:Float4)->u32
```

Pack four normalized floats into a 32-bit word, lowest byte first.

#### `unpack4x8_unorm`

```rust
pub fn unpack4x8_unorm(x:u32)->Float4
```

Unpack a 32-bit word as four unsigned-normalized float channels.

#### `reverse_bits`

```rust
pub fn reverse_bits(x:u32)->u32
```

Reverse all 32 bits of an unsigned integer.

#### `count_one_bits`

```rust
pub fn count_one_bits(x:u32)->u32
```

Count one-bits in an unsigned integer.

#### `count_leading_zeros`

```rust
pub fn count_leading_zeros(x:u32)->u32
```

Count leading zero bits in an unsigned integer.

#### `count_trailing_zeros`

```rust
pub fn count_trailing_zeros(x:u32)->u32
```

Count trailing zero bits in an unsigned integer.

#### `rotate_left`

```rust
pub fn rotate_left(x:u32,n:u32)->u32
```

Circularly rotate unsigned bits left; shifts are implicitly masked to 0..31.

#### `rotate_right`

```rust
pub fn rotate_right(x:u32,n:u32)->u32
```

Circularly rotate unsigned bits right; shifts are implicitly masked to 0..31.

#### `extract_bits`

```rust
pub fn extract_bits(x:u32,offset:u32,count:u32)->u32
```

Extract up to 32 consecutive bits from an unsigned word. Out-of-range bits are returned as zeros.

#### `insert_bits`

```rust
pub fn insert_bits(base:u32,insert:u32,offset:u32,count:u32)->u32
```

Insert low `count` bits of `insert` into `base` beginning at `offset`. Bits outside the 32-bit word are ignored.

#### `transpose4`

```rust
pub fn transpose4(columns:[Float4;4])->[Float4;4]
```

Transpose an array of four four-component vectors.

#### `mul_mat4_vec4`

```rust
pub fn mul_mat4_vec4(columns:[Float4;4],v:Float4)->Float4
```

Matrix-vector product, interpreting the matrix as four column vectors.

## Trait method behavior

`cpu::Float4` implements `Add`, `Sub`, `Mul` and `Div`, each performing four component-wise scalar operations. These are Rust operator trait methods, not exported inherent functions. Their internal method names are `add`, `sub`, `mul` and `div`.

## Errors and guarantees

- Shader expression emission is CPU-only. No GPU is required for `cargo check`, `cargo test`, `cargo doc`, or building the library.
- `Emitter::call` validates argument count and well-understood type families; `Emitter::call_typed` explicitly specifies a return type for opaque operations. Neither replaces shader compiler validation.
- `Expr::from_source_unchecked` is deliberately unchecked; only use it for trusted input.
- `FunctionBuilder` creates helper functions. Constructing full shader modules, resource bindings and pipeline declarations is the caller’s job.
- `CapabilitySet::ALL` enables catalog checking but **does not** guarantee runtime hardware or driver capabilities.

## Generate Rustdoc

```bash
cargo doc --all-features --no-deps --open
```
