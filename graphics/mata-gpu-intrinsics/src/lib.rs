//! Portable, **device-independent** GPU intrinsic definitions.
//!
//! This crate describes GPU operations and emits shader expressions; it does
//! not execute GPU code or discover graphics drivers during compilation.
//! Its caller (e.g. a D3D11, Vulkan, Metal, or WebGPU renderer) compiles the
//! resulting shader on the **runtime** GPU and owns the actual GPU resources.
//!
//! A CPU reference module is available with the default `std` feature.
//! The [`ops`] module contains a named Rust function for each catalog operation.
//!
//! ```
//! use mata_gpu_intrinsics::{Backend, CapabilitySet, Emitter, Expr, Intrinsic, Scalar, Stage, Type};
//! let emitter = Emitter::new(Backend::Wgsl, Stage::Compute, CapabilitySet::CORE);
//! let x = Expr::variable("x", Type::Scalar(Scalar::F32)).unwrap();
//! let y = emitter.call(Intrinsic::Sin, &[x]).unwrap();
//! assert_eq!(y.code(), "sin(x)");
//! ```
#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

pub mod ir;
pub mod collectives;
mod catalog;
mod emit;
/// Hardware-independent, backend-specific intrinsic function-pointer dispatch.
pub mod dispatch;
/// Intel GPU OpenCL C kernel-source generation (requires no GPU when compiling Rust).
pub mod intel;
/// NVIDIA CUDA device-kernel source generation without CUDA build requirements.
pub mod nvidia;
/// AMD HIP C++ compute-kernel source generation; never probes GPUs at build time.
pub mod amd;
/// Named Rust wrappers for every operation in [`Intrinsic`].
pub mod ops;
mod types;
#[cfg(feature="std")]
pub mod cpu;
pub mod precision;
pub mod resources;
pub mod vendor;

pub use catalog::{Intrinsic, IntrinsicInfo, Family, Signature, Arity, ALL_INTRINSICS, INTRINSIC_COUNT};
pub use emit::{Emitter, Expr, Error, FunctionBuilder, is_identifier};
pub use dispatch::{FunctionTable, BoundFunction, IntrinsicFn, TableEntry, backend_pointers};
pub use types::{Backend, Stage, Scalar, Type, Capability, CapabilitySet};
