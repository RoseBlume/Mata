//! Internal performance-oriented codec support primitives.
//!
//! These are kept under `codecs` so the crate's public root remains focused on
//! decoding, demuxing, metadata, playback, errors and SIMD.

#[allow(dead_code)] // Bit reservoirs are currently used by benchmarks and unit tests.
pub(crate) mod bits;
pub(crate) mod buffer;
#[allow(dead_code)] // Sparse seeking infrastructure is retained for codec integration.
pub(crate) mod index;
pub(crate) mod input;
pub(crate) mod instrumentation;
pub(crate) mod memory;
#[allow(dead_code)] // Planar/interleaved output helpers are exercised by tests.
pub(crate) mod output;

#[cfg(feature = "benchmarking")]
pub(crate) use bits::{BitReservoir64, BitReservoir128};
pub(crate) use buffer::{ByteWindow, RefillPolicy};
pub(crate) use input::{TunedBufReader, DEFAULT_IO_CAPACITY};
pub(crate) use instrumentation::StageTimer;
pub(crate) use memory::{AlignedScratch, checked_allocation_len, checked_setup_len};
