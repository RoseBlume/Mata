# Phase 2 — Target features, device limits, safety diagnostics

## Implemented interfaces

`ir::TargetProfile`, `ir::LanguageVersion`, `ir::DeviceLimits`, `ir::Feature`, `ir::Diagnostic`, `ir::DiagnosticKind`, `ir::AtomicSpec`, `ir::AtomicOrder`, `ir::AtomicScope`, and `ir::AtomicKind`, `ir::MemoryFenceSpec`, and `ir::FenceDomain`.

`Program::required_features()`, `Program::validate_for(&TargetProfile)`, and `Program::emit_for(&TargetProfile)` are **new additive APIs**; the legacy `emit(Backend)` remains available for existing consumers. Source-only offline operation still needs no hardware, and no dependencies have been added. `TargetProfile::offline(backend)` does **not** assume any optional features or device limits are supported. To validate against a particular physical device, the host application must populate capabilities from its device-query API. When `strict_limits = true`, missing required device limit fields are errors, not unbounded permission.

The checker validates baseline IR semantics and record layouts, target source versions (OpenCL C ≥ 1.2, GLSL ≥ 4.5, HLSL ≥ 5.0), optional numeric and address-space types, per-axis and total workgroup-size limits, buffer binding counts, buffer element size, scalar parameter uniform limits, resource pointer access, and group-barrier convergence. It checks helper calls at their invocation sites and catches guards based on local IDs, global IDs, or global loads, as well as early returns that prevent all work-items from reaching a barrier. It reports `Diagnostic { backend, kind, kernel, location, message, required_feature }`, using AST statement paths rather than inventing source line numbers.

The `AtomicSpec::validate()` and `MemoryFenceSpec::validate()` contracts validate load/store order, compare-exchange success/failure orders, integer atomic types, alignment, memory and fence scopes, and additional 64-bit or system-scope device support. **Atomic operations are not yet IR nodes or emitted instructions**: the roadmap schedules complete atomic expression lowering for Phase 7. This phase validates the proposed semantics, without incorrectly claiming full atomic code generation.

## Example

```rust
use mata_gpu_intrinsics::{Backend,ir::*};
let mut program = Program::default();
program.add_kernel(Kernel { name: "safe".into(), args: vec![],
    body: vec![Statement::Barrier], workgroup: [64, 1, 1] })?;
let mut profile = TargetProfile::offline(Backend::IntelOpenCl);
profile.strict_limits = true;
profile.limits.max_workgroup_size = Some(256);
profile.limits.max_workgroup_dimensions = [Some(256),Some(256),Some(64)];
let source = program.emit_for(&profile)?;
```

Note: the sample assumes appropriate `Result` return handling in the surrounding function. Source generation does not perform actual GPU discovery or compilation.

## Known conservative and deferred areas

- The IR does not yet carry explicit source spans or SSA-level memory-dependence information, so diagnostics use AST locations. Profilers and source-to-source diagnostic spans belong to later phases.
- The barrier checker is conservative: all memory reads and unknown helper return values are potentially varying, and barriers inside `while` loops are rejected unless a stronger uniformity proof is implemented. This is a **false-positive-safe restriction**, not an implementation of full formal reconvergence analysis.
- Memory alias and race-freedom cannot be proved from these annotations. Device/driver execution and compiler-validation tests still require actual GPU toolchains and drivers.
- OpenCL 3.0's optional built-ins and language features remain **device-specific**. The feature enum names such as `Fp64`, `Subgroups`, and `DeviceEnqueue` are requirements; the user must confirm them from the OpenCL runtime and language-feature query before enabling them. The `Feature::opencl_feature_macro` method exposes relevant OpenCL C feature macros, not a claim that all targets implement them.
- `validate_for` does not modify `Program::emit` or its interface; programs produced by the legacy method are not guaranteed profile-checked. Use `emit_for` before dispatch.
- Full atomics, flexible source-stage validation, detailed device resource schemas, and dynamic GPU capability probing require the later runtime and intrinsic phases described in the roadmap.

## Validate

```sh
cargo check --all-targets --all-features
cargo test --all-targets
cargo check --all-targets --no-default-features
cargo run --example phase2_checked_kernel
python3 tools/phase0_audit.py --check
```

No Cargo toolchain was available in the authoring environment; these commands are provided for local verification rather than claimed as completed successes.
