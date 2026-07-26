//! Differential tests for the soft-RDP (rdp-core): the identical command stream is run through the
//! real DP and through [`SoftRdp`], then the resulting framebuffers are compared pixel-exact.
//!
//! On real hardware this verifies the rdp-core algorithm. Inside an emulator that embeds rdp-core
//! as its RDP, it verifies the emulator's integration (command transport, RDRAM mapping).

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;

use arbitrary_int::prelude::*;
use rdp_core::{SliceRdram, SoftRdp};

use crate::graphics::color::{ARGB8888, RGBA5551};
use crate::rdp::fixedpoint::U10_2;
use crate::rdp::modes::{Blender, CoverageMode, CycleType, Format, Othermode, PixelSize, A, B, PM};
use crate::rdp::rdp::RDP;
use crate::rdp::rdp_assembler::{RDPAssembler, RDPRectangle};
use crate::tests::soft_asserts::{soft_assert_eq, soft_assert_eq2};
use crate::tests::{Level, Test};
use crate::uncached_memory::UncachedHeapMemory;

const WIDTH: usize = 32;
const HEIGHT: usize = 16;
/// Extra rows after the framebuffer, compared as well: fill mode can legitimately write one pixel
/// past the scissor right/bottom, which lands past the nominal framebuffer end.
const GUARD_ROWS: usize = 2;

/// FillRectangle in fill mode on a 16 bpp framebuffer, hardware vs soft-RDP.
pub struct SoftFillRectangle16 {}

impl Test for SoftFillRectangle16 {
    fn name(&self) -> &str {
        "SoftRDP: FillRectangle (fill mode, 16bpp)"
    }

    fn level(&self) -> Level {
        Level::RDPBasic
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        // (left, top, right, bottom) as raw 10.2 subpixel coordinates
        vec![
            Box::new((0u32, 0u32, 31u32 * 4, 15u32 * 4)), // full framebuffer
            Box::new((3u32 * 4, 2u32 * 4, 13u32 * 4, 9u32 * 4)), // interior, odd left edge
            Box::new((4u32 * 4, 1u32 * 4, 10u32 * 4, 1u32 * 4)), // single row, even edges
            Box::new((5u32 * 4, 3u32 * 4, 60u32 * 4, 30u32 * 4)), // clipped by the scissor on both axes
            Box::new((5u32 * 4, 3u32 * 4, 32u32 * 4, 14u32 * 4)), // right exactly at the scissor
            Box::new((5u32 * 4, 3u32 * 4, 60u32 * 4, 14u32 * 4)), // right beyond the scissor only
            Box::new((5u32 * 4, 3u32 * 4, 13u32 * 4, 30u32 * 4)), // bottom beyond the scissor only
            Box::new((3u32 * 4 + 1, 2u32 * 4, 13u32 * 4 + 2, 9u32 * 4)), // fractional left/right
            Box::new((3u32 * 4, 2u32 * 4 + 3, 13u32 * 4, 9u32 * 4 + 1)), // fractional top/bottom
            Box::new((3u32 * 4 + 3, 2u32 * 4 + 1, 13u32 * 4 + 3, 9u32 * 4 + 3)), // all fractional
            Box::new((5u32 * 4, 3u32 * 4, 31u32 * 4 + 3, 14u32 * 4)), // right just inside the scissor
            Box::new((13u32 * 4, 2u32 * 4, 3u32 * 4, 9u32 * 4)), // inverted horizontally (xh > xl)
            Box::new((3u32 * 4, 9u32 * 4, 13u32 * 4, 2u32 * 4)), // inverted vertically (yh > yl)
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let rect = *value
            .downcast_ref::<(u32, u32, u32, u32)>()
            .ok_or("Value is not a (u32, u32, u32, u32)")?;

        fill_rect_differential(
            CycleType::Fill,
            PixelSize::Bits16,
            rect,
            (0, 0, (WIDTH * 4) as u32, (HEIGHT * 4) as u32),
        )
    }
}

/// FillRectangle in fill mode on a 32 bpp framebuffer, hardware vs soft-RDP.
pub struct SoftFillRectangle32 {}

impl Test for SoftFillRectangle32 {
    fn name(&self) -> &str {
        "SoftRDP: FillRectangle (fill mode, 32bpp)"
    }

    fn level(&self) -> Level {
        Level::RDPBasic
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        // (left, top, right, bottom) as raw 10.2 subpixel coordinates
        vec![
            Box::new((0u32, 0u32, 31u32 * 4, 15u32 * 4)), // full framebuffer
            Box::new((3u32 * 4, 2u32 * 4, 13u32 * 4, 9u32 * 4)), // interior, odd left edge
            Box::new((5u32 * 4, 3u32 * 4, 60u32 * 4, 30u32 * 4)), // clipped by the scissor on both axes
            Box::new((3u32 * 4 + 3, 2u32 * 4 + 1, 13u32 * 4 + 3, 9u32 * 4 + 3)), // all fractional
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let rect = *value
            .downcast_ref::<(u32, u32, u32, u32)>()
            .ok_or("Value is not a (u32, u32, u32, u32)")?;

        fill_rect_differential(
            CycleType::Fill,
            PixelSize::Bits32,
            rect,
            (0, 0, (WIDTH * 4) as u32, (HEIGHT * 4) as u32),
        )
    }
}

/// Same differential, but the scissor is the variable and a fixed fractional rectangle overhangs
/// it on the right and bottom.
pub struct SoftFillRectangleScissor16 {}

impl Test for SoftFillRectangleScissor16 {
    fn name(&self) -> &str {
        "SoftRDP: FillRectangle vs fractional scissor (fill mode, 16bpp)"
    }

    fn level(&self) -> Level {
        Level::RDPBasic
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        // (scissor left, top, right, bottom) as raw 10.2 subpixel coordinates; the rectangle is
        // fixed at (0.5, 0.75)..=(60, 30) and overhangs the scissor on all sides
        vec![
            Box::new((0u32, 0u32, 127u32, 64u32)),  // right 31.75
            Box::new((0u32, 0u32, 126u32, 63u32)),  // right 31.5, bottom 15.75
            Box::new((0u32, 0u32, 125u32, 61u32)),  // right 31.25, bottom 15.25
            Box::new((0u32, 0u32, 129u32, 66u32)), // right 32.25 (past the framebuffer), bottom 16.5
            Box::new((1u32, 0u32, 128u32, 64u32)), // left 0.25
            Box::new((3u32, 2u32, 128u32, 64u32)), // left 0.75, top 0.5
            Box::new((9u32, 11u32, 128u32, 64u32)), // left 2.25, top 2.75
            Box::new((8u32, 12u32, 128u32, 64u32)), // left 2, top 3 (integer)
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let scissor = *value
            .downcast_ref::<(u32, u32, u32, u32)>()
            .ok_or("Value is not a (u32, u32, u32, u32)")?;

        fill_rect_differential(
            CycleType::Fill,
            PixelSize::Bits16,
            (2, 3, 60 * 4, 30 * 4),
            scissor,
        )
    }
}

/// FillRectangle in 1-cycle mode: no fill-mode extra pixel, subpixel-exclusive right/bottom.
pub struct SoftOneCycleRectangle16 {}

impl Test for SoftOneCycleRectangle16 {
    fn name(&self) -> &str {
        "SoftRDP: FillRectangle (1-cycle, 16bpp)"
    }

    fn level(&self) -> Level {
        Level::RDPBasic
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        // (left, top, right, bottom) as raw 10.2 subpixel coordinates
        vec![
            Box::new((3u32 * 4, 2u32 * 4, 13u32 * 4, 9u32 * 4)), // integer: right/bottom exclusive?
            Box::new((3u32 * 4 + 2, 2u32 * 4 + 1, 13u32 * 4 + 2, 9u32 * 4 + 3)), // fractional edges
            Box::new((5u32 * 4, 3u32 * 4, 60u32 * 4, 30u32 * 4)), // clipped by the scissor
            Box::new((13u32 * 4, 2u32 * 4, 3u32 * 4, 9u32 * 4)), // inverted horizontally
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let rect = *value
            .downcast_ref::<(u32, u32, u32, u32)>()
            .ok_or("Value is not a (u32, u32, u32, u32)")?;

        fill_rect_differential(
            CycleType::SingleCycle,
            PixelSize::Bits16,
            rect,
            (0, 0, (WIDTH * 4) as u32, (HEIGHT * 4) as u32),
        )
    }
}

/// Like [`SoftOneCycleRectangle16`], on a 32bpp framebuffer (also pins down the memory byte order
/// and the coverage-to-alpha-byte write).
pub struct SoftOneCycleRectangle32 {}

impl Test for SoftOneCycleRectangle32 {
    fn name(&self) -> &str {
        "SoftRDP: FillRectangle (1-cycle, 32bpp)"
    }

    fn level(&self) -> Level {
        Level::RDPBasic
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((3u32 * 4, 2u32 * 4, 13u32 * 4, 9u32 * 4)),
            Box::new((3u32 * 4 + 2, 2u32 * 4 + 1, 13u32 * 4 + 2, 9u32 * 4 + 3)),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let rect = *value
            .downcast_ref::<(u32, u32, u32, u32)>()
            .ok_or("Value is not a (u32, u32, u32, u32)")?;

        fill_rect_differential(
            CycleType::SingleCycle,
            PixelSize::Bits32,
            rect,
            (0, 0, (WIDTH * 4) as u32, (HEIGHT * 4) as u32),
        )
    }
}

fn fill_rect_differential(
    cycle_type: CycleType,
    pixel_size: PixelSize,
    (left, top, right, bottom): (u32, u32, u32, u32),
    (sc_left, sc_top, sc_right, sc_bottom): (u32, u32, u32, u32),
) -> Result<(), String> {
    let bytes_per_pixel = match pixel_size {
        PixelSize::Bits16 => 2,
        PixelSize::Bits32 => 4,
        _ => return Err("Unsupported pixel size".into()),
    };
    let total_bytes = WIDTH * (HEIGHT + GUARD_ROWS) * bytes_per_pixel;

    let mut framebuffer = UncachedHeapMemory::<u32>::new_with_align(total_bytes / 4, 64);
    // Prefill both framebuffers with the same recognizable pattern so untouched pixels are
    // compared as well.
    let mut soft_bytes = vec![0u8; total_bytes];
    for i in 0..total_bytes / 4 {
        let value = 0xA000_0000u32 | (i as u32);
        framebuffer.write(i, value);
        soft_bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_be_bytes());
    }

    let mut assembler = RDPAssembler::new();
    assembler.set_framebuffer_image(
        Format::RGBA,
        pixel_size,
        u12::new((WIDTH - 1).try_into().unwrap()),
        &mut framebuffer,
    );
    assembler.set_scissor(&RDPRectangle::new(
        U10_2::new_with_masked_value(sc_left),
        U10_2::new_with_masked_value(sc_top),
        U10_2::new_with_masked_value(sc_right),
        U10_2::new_with_masked_value(sc_bottom),
    ));
    match cycle_type {
        CycleType::SingleCycle => {
            // The blender/coverage configuration the triangle tests established: output = blend
            // color, coverage zapped to full
            assembler.set_othermode(
                Othermode::DEFAULT
                    .with_cycle_type(CycleType::SingleCycle)
                    .with_coverage_mode(CoverageMode::Zap)
                    .with_blender_0(Blender::new(
                        A::CombineAlpha,
                        PM::BlendColor,
                        B::Zero,
                        PM::MemoryColor,
                    )),
            );
            // Distinct r/g/b bytes
            assembler.set_blendcolor(ARGB8888::new_with_raw_value(0xF848_0800));
        }
        _ => {
            assembler.set_othermode(Othermode::DEFAULT.with_cycle_type(CycleType::Fill));
            match pixel_size {
                // Two different colors so the differential also checks which fill color half lands
                // on even/odd pixels.
                PixelSize::Bits16 => assembler.set_fillcolor16(
                    RGBA5551::new(u5::new(31), u5::new(0), u5::new(0), true),
                    RGBA5551::new(u5::new(0), u5::new(0), u5::new(31), false),
                ),
                // Four distinct bytes to catch any partial write
                _ => assembler.set_fillcolor32(ARGB8888::new_with_raw_value(0x1234_5678)),
            }
        }
    }
    assembler.filled_rectangle(&RDPRectangle::new(
        U10_2::new_with_masked_value(left),
        U10_2::new_with_masked_value(top),
        U10_2::new_with_masked_value(right),
        U10_2::new_with_masked_value(bottom),
    ));
    assembler.sync_full();

    // Bounded wait instead of RDP::run_and_wait: if a hostile input hangs the DP, report it
    // instead of wedging the whole suite
    let end = assembler.end();
    unsafe {
        RDP::start_running(assembler.start(), end);
    }
    let mut done = false;
    for _ in 0..10_000_000 {
        if RDP::current() == end as u32 {
            done = true;
            break;
        }
    }
    if !done {
        return Err(format!(
            "RDP did not finish (hang?). CURRENT=0x{:x} END=0x{:x} STATUS=0x{:x}",
            RDP::current(),
            end,
            RDP::status()
        ));
    }

    // Run the identical command words through the soft-RDP
    let stream: Vec<u64> = (0..assembler.len()).map(|i| assembler.word(i)).collect();
    let mut soft = SoftRdp::new();
    {
        let mut hidden = vec![0u8; total_bytes / 2];
        let mut rdram = SliceRdram::new(
            framebuffer.start_physical() as u32,
            &mut soft_bytes,
            &mut hidden,
        );
        soft.run(&stream, &mut rdram);
    }
    soft_assert_eq(soft.unhandled, 0, "SoftRdp hit unhandled commands")?;

    let words_per_row = WIDTH * bytes_per_pixel / 4;
    for word in 0..total_bytes / 4 {
        let hardware = framebuffer.read(word);
        let soft_value = u32::from_be_bytes(soft_bytes[word * 4..word * 4 + 4].try_into().unwrap());
        if soft_value != hardware {
            let x = (word % words_per_row) * 4 / bytes_per_pixel;
            let y = word / words_per_row;
            // Dump the whole hardware row to make the mismatch pattern visible on screen
            let mut row = String::new();
            for word2 in y * words_per_row..(y + 1) * words_per_row {
                row.push_str(&format!("{:08x} ", framebuffer.read(word2)));
            }
            soft_assert_eq2(soft_value, hardware, || {
                format!(
                    "Rect raw ({}, {})..=({}, {}), scissor ({}, {}, {}, {}): pixel ({}, {}) soft-RDP vs hardware. HW row: {}",
                    left, top, right, bottom, sc_left, sc_top, sc_right, sc_bottom, x, y, row
                )
            })?;
        }
    }

    Ok(())
}
