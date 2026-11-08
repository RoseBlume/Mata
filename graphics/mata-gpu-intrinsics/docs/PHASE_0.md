# Phase 0 complete: baseline, manifests and compile-only gates

This phase freezes **the 322 pre-existing intrinsic IDs in their original order** without adding or renaming any public Rust operation or introducing dependencies.

## Generated deliverables

- `audit/catalog_names.txt` — immutable Phase 0 snapshot of `Intrinsic` enum variants in numeric order.
- `audit/backend_coverage.csv` — one row per intrinsic and source target (1,932 rows); records family, arity, signature category, capability, shader stage, declared spelling, mapping status and unverified compiler/device flags.
- `audit/source_manifest.json` — hashed source inventory, reference document editions, catalog fingerprint and declared mapping totals.
- `audit/BACKEND_COVERAGE.md` — aggregate coverage table, status interpretation and what validation is still required.
- `tools/phase0_audit.py` — Python-standard-library inventory generator and deterministic `--check` mode.
- `tests/phase0_baseline.rs` — freezes 322 numeric IDs and compile-smokes a representative expression in each of six backends.
- `.github/workflows/phase0.yml` — dependency-free Python validation and Rust check/test/doc matrix.

## Using the audit

```bash
python3 tools/phase0_audit.py --check
python3 tools/validate_catalog.py
cargo check --all-targets --all-features
cargo check --all-targets --no-default-features
cargo test --all-features
cargo test --no-default-features
cargo doc --all-features --no-deps
```

To regenerate intentionally after editing the source mapping, run `python3 tools/phase0_audit.py`, inspect the diff, and explicitly update `audit/catalog_names.txt` **only when an intentional public-API compatibility change is approved**. The Rust test rejects accidental changes to the Phase 0 catalog.

## Status semantics

All entries are either `declared_unverified` (there is a spelling mapping) or `unmapped` (there is none). Even declared entries can fail typed emission, source compiler validation, capability checks or runtime hardware tests; **no status asserts correctness**. Some vendors share CUDA-style names via delegation, but a spelling alone does not establish HIP support. Phase 0 does **not** add a GPU runtime.

## Source limitations

The CUDA 13.4.2 *Programming Guide* documents feature families rather than every native signature; HIP's bundled reference is missing externally generated Doxygen signatures for many runtime modules; OpenCL 3.0's compact quick-reference shows optional APIs but cannot prove feature support of a specific device. No missing declarations were invented to fill these gaps.

## Validation environment

Python-static inventory can run without a Rust toolchain or GPU. Cargo commands and compiler-backed checks should be executed by the included CI workflow or a Rust installation; GPU/compiler/runtime support requires additional tests in later phases. The original `docs/VALIDATION.md` is a historical document and describes an older operation count; `audit/` is the authoritative Phase 0 inventory.
