# Phase 2 verification report

**Source baseline:** `mata-gpu-intrinsics-phase1-complete.zip`.

## Tests executed in authoring environment

| Check | Result |
|---|---|
| `python3 tools/phase0_audit.py --check` | PASS — 322 original intrinsic variants, 1,932 backend audit mappings, original source manifest current |
| `python3 tools/phase2_audit.py` | PASS — additive safety module present, 21 integration tests, original catalog identifiers preserved, no new Cargo dependencies |
| `python3 tools/validate_catalog.py` | PASS — original 322 wrappers and mappings match generated definitions |
| `python3 tools/validate_dispatch.py` | PASS — original backend pointer audit |
| `python3 tools/validate_nvidia.py` | PASS — original CUDA source audit |
| `python3 tools/validate_amd.py` | PASS — original HIP source audit |
| Python `compileall` on `tools/` | PASS |
| Lexical delimiter check on new Rust source, tests, and example | PASS — structural only, not parsing/compilation |

## Checks not executable here

| Check | Status |
|---|---|
| `cargo check --all-targets --all-features` | NOT RUN: cargo/rustc unavailable |
| `cargo test --all-targets` | NOT RUN: cargo/rustc unavailable |
| `cargo check --all-targets --no-default-features` | NOT RUN: cargo/rustc unavailable |
| Native device compilation on CUDA/HIP/OpenCL | NOT RUN: compiler toolchains/devices unavailable |

This distinction is intentional: the source modifications and tests have been delivered, but a static audit is **not evidence of compilation or complete device conformance**. The GitHub Actions workflow runs Cargo checks and tests on a Rust-enabled runner.

## Phase 2 scope

Completed in source: profile and optional feature descriptors, target-language minimum version checking, explicit device limits with unknown-value handling, optional OpenCL C feature macro names, target memory/resource layout checks, early invalid access checks, workgroup barrier convergence analysis including nested calls, early returns and loop checks, typed diagnostics, atomic and fence semantic contracts, negative regression tests and CI integration.

Not part of this phase: actual host GPU probing, runtime allocation and execution, complete device atomic instruction lowering, advanced subgroup rendezvous, and proof of data-race freedom. Those appear in later roadmap phases. Legacy `Program::emit(Backend)` is retained and bypasses the new profile checks; new users should prefer `Program::emit_for(&TargetProfile)`.
