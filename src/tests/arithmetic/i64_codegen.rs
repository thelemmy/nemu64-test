//! Pure-Rust self-checks that must hold regardless of how the compiler lowers them.
//!
//! These exist because i64 values were observed to come back corrupted on console (low word
//! correct, high word holding a stray value) while the identical code was correct on a host - see
//! docs/mips-i64-codegen-bug.md. Nothing here touches hardware: every test computes a value two
//! ways and compares, so a failure means the toolchain, not the N64, is at fault.
//!
//! These reductions currently PASS: they are below the register-pressure threshold that triggers
//! the corruption. They are kept as regression tests and as a record of what was already tried,
//! so nobody re-reduces along the same dead end.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use crate::tests::soft_asserts::{soft_assert_eq, soft_assert_eq2};
use crate::tests::{Level, Test};

/// Values are read through this so the optimizer can't constant-fold the whole computation away.
#[inline(never)]
fn opaque<T>(value: T) -> T {
    unsafe { core::ptr::read_volatile(&value) }
}

/// An edge walker reduced from the RDP triangle rasterizer, in i64.
///
/// Steps two accumulators once per iteration and, on every fourth one, derives a pixel range from
/// them. Returns the sum of all ranges so a single value captures the whole walk.
#[inline(never)]
fn walk_i64(xh: i64, dh: i64, xm: i64, dm: i64, iterations: i32) -> i64 {
    let mut major = xh;
    let mut minor = xm;
    let mut accumulated = 0i64;
    let mut y = 0i32;
    while y < iterations {
        if (y & 3) == 0 && minor >= major {
            let start = (major + (1 << 18) - 1) >> 18;
            let end = ((minor - 8) >> 16) >> 2;
            accumulated += start * 1000 + end;
        }
        major += dh;
        minor += dm;
        y += 1;
    }
    accumulated
}

/// The identical walk in i32. Every intermediate stays well inside i32 for the inputs used, so
/// both functions must return the same number.
#[inline(never)]
fn walk_i32(xh: i32, dh: i32, xm: i32, dm: i32, iterations: i32) -> i64 {
    let mut major = xh;
    let mut minor = xm;
    let mut accumulated = 0i64;
    let mut y = 0i32;
    while y < iterations {
        if (y & 3) == 0 && minor >= major {
            let start = (major + (1 << 18) - 1) >> 18;
            let end = ((minor - 8) >> 16) >> 2;
            accumulated += (start as i64) * 1000 + (end as i64);
        }
        major += dh;
        minor += dm;
        y += 1;
    }
    accumulated
}

pub struct I64EdgeWalk {}

impl Test for I64EdgeWalk {
    fn name(&self) -> &str {
        "i64 edge walk matches the same walk in i32"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        // (xh, dh, xm, dm) in the rasterizer's <<2-shifted 16.16 domain, plus the iteration count
        const CASES: &[(i32, i32, i32, i32, i32)] = &[
            (0x0004_0000, 0, 0x0014_0000, 0, 64),
            (0x0004_0000, 0x0001_0000, 0x0014_0000, 0x0002_0000, 64),
            (0x0020_0000, -0x0001_0000, 0x0060_0000, -0x0002_0000, 48),
            (-0x0008_0000, 0x0000_4000, 0x0008_0000, 0x0000_8000, 96),
        ];
        crate::tests::boxed_values(CASES)
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let (xh, dh, xm, dm, iterations) = *value
            .downcast_ref::<(i32, i32, i32, i32, i32)>()
            .ok_or("Value is not a (i32, i32, i32, i32, i32)")?;

        let wide = walk_i64(
            opaque(xh as i64),
            opaque(dh as i64),
            opaque(xm as i64),
            opaque(dm as i64),
            opaque(iterations),
        );
        let narrow = walk_i32(
            opaque(xh),
            opaque(dh),
            opaque(xm),
            opaque(dm),
            opaque(iterations),
        );

        soft_assert_eq2(wide, narrow, || {
            format!(
                "walk_i64 vs walk_i32 for xh={:#x} dh={:#x} xm={:#x} dm={:#x} n={}",
                xh, dh, xm, dm, iterations
            )
        })?;

        Ok(())
    }
}

/// LLVM lowers i64 locals to MIPS III 64-bit instructions, so a spilled i64 becomes an `sd`/`ld`
/// pair against the stack pointer. Those fault unless the stack stays 8 byte aligned, which makes
/// the alignment of the stack we are handed a correctness prerequisite for all i64 code in this
/// suite - not just a convention.
pub struct StackPointerIs8ByteAligned {}

impl Test for StackPointerIs8ByteAligned {
    fn name(&self) -> &str {
        "Stack pointer is 8 byte aligned (i64 spills need it)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let sp: u64;
        unsafe {
            asm!("move {}, $29", out(reg) sp);
        }

        soft_assert_eq2(sp & 7, 0, || {
            format!("Stack pointer {:#x} must be 8 byte aligned", sp)
        })?;

        // An actual 64-bit spill/reload through the stack, which is what the compiler emits for a
        // spilled i64
        let mut slot = [0u64; 2];
        let read_back: u64;
        unsafe {
            asm!("
                sd {value}, 0({ptr})
                ld {out}, 0({ptr})
            ",
            value = in(reg) 0x0123_4567_89AB_CDEFu64,
            ptr = in(reg) slot.as_mut_ptr(),
            out = out(reg) read_back);
        }
        soft_assert_eq(
            read_back,
            0x0123_4567_89AB_CDEF,
            "sd/ld roundtrip on the stack",
        )?;

        Ok(())
    }
}

/// Byte-addressed memory behind a trait, mirroring how the software rasterizer reaches RAM: the
/// wide accessors are trait default methods that decompose into bounds-checked byte writes.
trait ByteSink {
    fn write_u8(&mut self, address: u32, value: u8);

    fn write_u16(&mut self, address: u32, value: u16) {
        self.write_u8(address, (value >> 8) as u8);
        self.write_u8(address + 1, value as u8);
    }
}

struct SliceSink<'a> {
    base: u32,
    bytes: &'a mut [u8],
}

impl ByteSink for SliceSink<'_> {
    fn write_u8(&mut self, address: u32, value: u8) {
        let index = (address - self.base) as usize;
        self.bytes[index] = value;
    }
}

/// The rasterizer's inner loop, kept structurally intact: state is decoded out of 64-bit command
/// words, the span bounds are computed in i64 and the covered pixels are written through the trait
/// above.
#[inline(never)]
fn walk_and_paint(cmd: &[u64; 4], scissor_right: u32, sink: &mut impl ByteSink, base: u32) {
    let right_major = (cmd[0] >> 55) & 1 != 0;
    let yl = (((cmd[0] >> 32) & 0x3FFF) as u32) as i32;
    let ym = (((cmd[0] >> 16) & 0x3FFF) as u32) as i32;
    let yh = ((cmd[0] & 0x3FFF) as u32) as i32;
    let xl = ((cmd[1] >> 32) as u32 as i32 as i64) << 2;
    let dl = cmd[1] as u32 as i32 as i64;
    let xh = ((cmd[2] >> 32) as u32 as i32 as i64) << 2;
    let dh = cmd[2] as u32 as i32 as i64;
    let xm = ((cmd[3] >> 32) as u32 as i32 as i64) << 2;
    let dm = cmd[3] as u32 as i32 as i64;

    let mut major_x = xh;
    let mut minor_x = xm;
    let mut y = yh;
    let mut section = 0;
    loop {
        let (y_target, minor_inc) = if section == 0 { (ym, dm) } else { (yl, dl) };
        if y >= y_target {
            if section == 1 {
                break;
            }
            section = 1;
            minor_x = xl;
            continue;
        }

        if (y & 3) == 0 {
            let (left, right) = if right_major {
                (major_x, minor_x)
            } else {
                (minor_x, major_x)
            };
            if right >= left {
                let px_start = ((left + (1 << 18) - 1) >> 18).max(0);
                let px_end = ((right - (2 << 2)) >> 16).min(scissor_right as i64 - 1) >> 2;
                let py = (y >> 2) as u32;
                let mut px = px_start;
                while px <= px_end {
                    sink.write_u16(base + py * 64 + (px as u32) * 2, 0xFA43);
                    px += 1;
                }
            }
        }

        major_x += dh;
        minor_x += minor_inc;
        y += 1;
    }
}

/// Reference implementation of [`walk_and_paint`], written as plainly as possible.
fn walk_and_paint_reference(cmd: &[u64; 4], scissor_right: u32, out: &mut [u8], base: u32) {
    let right_major = (cmd[0] >> 55) & 1 != 0;
    let yl = (((cmd[0] >> 32) & 0x3FFF) as u32) as i32;
    let ym = (((cmd[0] >> 16) & 0x3FFF) as u32) as i32;
    let yh = ((cmd[0] & 0x3FFF) as u32) as i32;
    let xl = ((cmd[1] >> 32) as u32 as i32).wrapping_shl(2);
    let dl = cmd[1] as u32 as i32;
    let xh = ((cmd[2] >> 32) as u32 as i32).wrapping_shl(2);
    let dh = cmd[2] as u32 as i32;
    let xm = ((cmd[3] >> 32) as u32 as i32).wrapping_shl(2);
    let dm = cmd[3] as u32 as i32;

    let mut major_x = xh;
    let mut minor_x = xm;
    let mut y = yh;
    let mut section = 0;
    loop {
        let (y_target, minor_inc) = if section == 0 { (ym, dm) } else { (yl, dl) };
        if y >= y_target {
            if section == 1 {
                break;
            }
            section = 1;
            minor_x = xl;
            continue;
        }

        if (y & 3) == 0 {
            let (left, right) = if right_major {
                (major_x, minor_x)
            } else {
                (minor_x, major_x)
            };
            if right >= left {
                let px_start = (left.wrapping_add((1 << 18) - 1) >> 18).max(0);
                let px_end =
                    ((right.wrapping_sub(2 << 2) >> 16).min(scissor_right as i32 - 1)) >> 2;
                let py = (y >> 2) as u32;
                let mut px = px_start;
                while px <= px_end {
                    let index = (base + py * 64 + (px as u32) * 2 - base) as usize;
                    out[index] = 0xFA;
                    out[index + 1] = 0x43;
                    px += 1;
                }
            }
        }

        major_x += dh;
        minor_x += minor_inc;
        y += 1;
    }
}

/// Runs the rasterizer inner loop twice - once in i64, once in i32 - over the same triangle and
/// compares the painted memory. All inputs are chosen so no intermediate leaves i32 range, so the
/// two must agree exactly.
pub struct I64WalkAndPaint {}

impl Test for I64WalkAndPaint {
    fn name(&self) -> &str {
        "i64 rasterizer walk paints the same pixels as i32"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        // Triangle command words, captured from a failing SoftRDP differential run
        const CASES: &[[u64; 4]] = &[
            [
                0x0880_0014_000c_0004,
                0x0003_0000_0000_0000,
                0x0001_0000_0000_0000,
                0x0005_0000_0000_0000,
            ],
            [
                0x0800_0038_001c_0004,
                0x0002_0000_0001_0000,
                0x0002_0000_0000_0000,
                0x0002_0000_0002_0000,
            ],
            [
                0x0880_002d_0013_0005,
                0x0014_0000_ffff_0000,
                0x0018_0000_ffff_8000,
                0x0018_0000_fffe_0000,
            ],
        ];
        crate::tests::boxed_values(CASES)
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let cmd = *value
            .downcast_ref::<[u64; 4]>()
            .ok_or("Value is not a [u64; 4]")?;

        const SIZE: usize = 64 * 24;
        let base = 0x0010_0000u32;

        let mut wide = vec![0u8; SIZE];
        {
            let mut sink = SliceSink {
                base,
                bytes: &mut wide,
            };
            walk_and_paint(&cmd, 128, &mut sink, base);
        }

        let mut narrow = vec![0u8; SIZE];
        walk_and_paint_reference(&cmd, 128, &mut narrow, base);

        for i in 0..SIZE {
            soft_assert_eq2(wide[i], narrow[i], || {
                format!(
                    "Byte {} (pixel {}, row {}) for triangle {:#x?}",
                    i,
                    (i % 64) / 2,
                    i / 64,
                    cmd
                )
            })?;
        }

        Ok(())
    }
}
