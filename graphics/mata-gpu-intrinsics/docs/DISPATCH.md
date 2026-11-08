# Function-Pointer Dispatch — All Six GPU Backends

The `dispatch` module adds a **function pointer table for each backend**,
covering the original **322 intrinsic IDs**. Every target uses the same indexed
operation names, call conventions, type checking and error reporting.

This is a GPU-**shader source generator**: the pointers invoke **host Rust
functions** that emit source expressions. They do **not** point to GPU machine
code or dynamically load device drivers. They are safe to compile on a server
without a GPU or driver installed.

## Backend table selection

| Target | Constructor | Stage |
|---|---|---|
| WGSL | `FunctionTable::wgsl(stage, caps)` | Vertex, fragment or compute |
| GLSL | `FunctionTable::glsl(stage, caps)` | Vertex, fragment or compute |
| HLSL | `FunctionTable::hlsl(stage, caps)` | Vertex, fragment or compute |
| Intel OpenCL C | `FunctionTable::intel(caps)` | Compute only |
| NVIDIA CUDA C++ | `FunctionTable::nvidia(caps)` | Compute only |
| AMD HIP C++ | `FunctionTable::amd(caps)` | Compute only |

`FunctionTable::new(backend,stage,caps)` is equivalent to the convenience
constructors. Use `FunctionTable::from_emitter(emitter)` to reuse an existing
configuration. The stage and capabilities are declarative metadata; no device
feature probing occurs.

## Lookups and invocation

```rust
use mata_gpu_intrinsics::{CapabilitySet, Expr, FunctionTable, Intrinsic, Type};

let table=FunctionTable::amd(CapabilitySet::CORE);
let x=Expr::variable("x",Type::F32)?;

// Option<BoundFunction<'_>>: no per-call dispatch by backend.
if let Some(sin)=table.get(Intrinsic::Sin) {
    let expression=sin.call(&[x.clone()])?;
    assert_eq!(expression.code(),"sinf(x)");
}

// Name lookup: uses canonical catalog identifiers, with case sensitivity.
let same=table.call_named("sin", &[x])?;

// Dynamic identifier and arguments:
let op=Intrinsic::Sin;
let result=table.call(op, &[Expr::float(0.5)?])?;

// For opaque GPU operations (atomics, texture loads, subgroups), use
// table.call_typed(op,&args,return_type) or
// table.get(op).unwrap().call_typed(&args,return_type).
# Ok::<(), mata_gpu_intrinsics::Error>(())
```

### Raw pointers and metadata

`table.pointer(op)` returns `Option<IntrinsicFn>`. The signature is:

```rust
fn(&Emitter, &[Expr], Option<Type>) -> Result<Expr, Error>
```

Pass `table.emitter()` as the first argument. A pointer from a different
backend rejects an incompatible emitter with `Error::BackendMismatch`.
`None` as `return_type` uses ordinary type inference; `Some(ty)` uses explicit
result validation. `backend_pointers(backend)` returns the unfiltered,
compile-time pointer table. Prefer `FunctionTable`, whose unsupported slots are
`None`.

`table.entries()` iterates in catalog order and returns `TableEntry` with:

- `intrinsic`: original `Intrinsic` ID
- `info`: `&'static IntrinsicInfo` with family, arity, stage, capability and native spellings
- `function`: `Option<BoundFunction<'_>>`, based on backend/stage/capability

`table.available_count()` counts active entries, while
`table.catalog_count()` always reports the total catalog size.

## Explicit return types

```rust
use mata_gpu_intrinsics::{Backend, Capability, CapabilitySet,
    Expr, FunctionTable, Intrinsic, Stage, Type};
let caps=CapabilitySet::CORE.with(Capability::Textures);
let t=FunctionTable::new(Backend::Wgsl,Stage::Fragment,caps);
let args=[Expr::variable("tex",Type::Texture2D)?,
          Expr::variable("sampler",Type::Sampler)?,
          Expr::variable("uv",Type::VEC2)?];
let result=t.call_typed(Intrinsic::TextureSample,&args,Type::VEC4)?;
assert_eq!(result.code(),"textureSample(tex, sampler, uv)");
# Ok::<(), mata_gpu_intrinsics::Error>(())
```

## Error behavior

- Invalid catalog names return `Error::UnknownIntrinsicName` for named calls,
  or `None` for `get_named`.
- Known operations that are excluded by the backend/stage/capability mask
  have `None` in the table. Calling them through `table.call()` or
  `table.call_typed()` delegates to the existing `Emitter` for precise
  `Unsupported`, `WrongStage` or `FeatureDisabled` errors.
- `IntrinsicFn` rejects mismatched backends, even when called directly.
- Argument number and types are checked by the same existing emitter methods;
  the function pointer bypasses no validation.
- `CapabilitySet::ALL` represents requested features, not verified GPU
  support. Shader compilers and device runtimes still perform final checks.

## Integration and performance

The function pointer table is built once for each configuration with a fixed
array of `Option<fn>`. Lookups by enum index are O(1), and each bound callable
uses direct function-pointer dispatch. `get_named` performs catalog name
matching (O(number of catalog entries)); cache the handle for hot paths.

Every source-emitting backend has 322 precompiled Rust entry points; unsupported
operations are removed from the **active** table at initialization, not from
the static array. Pointer tables increase binary/code size but avoid backend
selection logic at each lookup. Shader expression generation is typically far
less costly than GPU pipeline compilation and runtime execution.

No dependencies, unsafe blocks, GPU detection, runtime libraries, or native
GPU SDKs are added. The function pointers are ordinary Rust function pointers.

To regenerate the tables after adding new intrinsics:

```bash
python3 tools/generate_catalog.py
python3 tools/generate_wrappers.py
python3 tools/generate_dispatch.py
python3 tools/generate_docs.py
```

To run offline tests:

```bash
cargo test --test dispatch
cargo test
cargo check --no-default-features
cargo run --example function_table
```
