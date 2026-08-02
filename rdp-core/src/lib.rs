//! A software implementation of the N64 RDP, verified against real hardware.
//!
//! # How this is developed
//!
//! This crate is grown test-by-test inside n64-systemtest: every behavior lands together with a
//! differential test that runs the identical command byte stream through both the real DP and
//! [`SoftRdp`] and compares the resulting memory. On real hardware that verifies the algorithm;
//! inside an emulator that embeds this crate as its RDP, the same ROM verifies the emulator's
//! integration (command transport, RDRAM mapping, hidden bits). The test ROM is the testsuite -
//! there is no separate one.
//!
//! Everything here is derived from our own reverse engineering on real hardware. Existing software
//! implementations are deliberately not used as a reference. Behavior that is implemented but not
//! yet pinned down by a hardware test is marked with "provisional" in the comments. The findings
//! are collected in REFERENCE.md next to this crate.
//!
//! # Design rules
//!
//! - `no_std`, no allocator, no dependencies: usable from the test ROM (MIPS) and from emulators.
//! - Integer-only: the RDP is fixed-point hardware; bit-exactness must not depend on the host FPU.
//! - Commands are consumed as raw `u64` words (the same bytes the DP fetches), so command decoding
//!   itself is under test - there is no parallel "assembler-level" API to hide decode bugs.
//! - Memory access goes through the [`Rdram`] trait, including the hidden 9th bits, so an emulator
//!   can hand in its own RDRAM.
//! - Written to vectorize later: rasterization is span-based, per-primitive state is resolved to
//!   plain integers before the span loop (no muxing branches per pixel), and the pixel pipeline
//!   operates on fixed-size row buffers that a SIMD backend can process in lanes. The scalar code
//!   is the specification; a SIMD backend must match it bit-exactly.

#![no_std]

pub mod cmd;
mod raster;
pub mod rdram;
mod soft;
pub mod state;

pub use raster::{COMBINE_PASSTHROUGH_SHADE, COMBINE_PASSTHROUGH_TEXEL0};
pub use rdram::{Rdram, SliceRdram};
pub use soft::SoftRdp;
