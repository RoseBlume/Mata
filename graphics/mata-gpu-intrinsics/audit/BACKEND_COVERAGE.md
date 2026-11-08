# Phase 0 — Static backend coverage (baseline)

**322 frozen enum slots × 6 targets = 1,932 rows.** Every row is `declared_unverified` or `unmapped`. This is intentionally conservative: a name mapping is not a successful emitted call, compiler acceptance, supported overload, or hardware capability.

| Target | Declared mapping (unverified) | Unmapped |
|---|---:|---:|
| `wgsl` | 135 | 187 |
| `glsl` | 185 | 137 |
| `hlsl` | 124 | 198 |
| `intel_opencl` | 102 | 220 |
| `nvidia_cuda` | 89 | 233 |
| `amd_hip` | 108 | 214 |

## Interpret the statuses

- `declared_unverified`: a target spelling is declared by the original registry/native spelling function; it may still fail type checking, render, shader compilation, or hardware validation.
- `unmapped`: no recognized static spelling; may be unsupported or require specialized lowering.
- There is no `verified` status in Phase 0; that requires representative argument types, expression rendering, compiler testing, and device capability checks in later phases.

The CSV includes the operation family, exact signature strategy, arity, required capability/stage, source spelling, and target name for each operation. Native HIP spelling is inferred from its explicit mappings or documented CUDA-spelling delegation in the implementation.

## Follow-up verification

Perform per-signature emitter invocation tests, compiler syntax tests for each target version, and hardware conformance tests; record evidence separately rather than promoting mappings to `verified` based on name alone.

## Frozen catalog fingerprint

`0fd51ed64fdacc7a2ae7b96ded04a010f37b56bb17e7fbb65e9342b88bef6a3b` (SHA-256 of the 322 stable variant identifiers joined by newlines).
