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

use crate::graphics::color::RGBA5551;
use crate::rdp::fixedpoint::U10_2;
use crate::rdp::modes::{CycleType, Format, Othermode, PixelSize};
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
const TOTAL: usize = WIDTH * (HEIGHT + GUARD_ROWS);

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
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let (left, top, right, bottom) = *value
            .downcast_ref::<(u32, u32, u32, u32)>()
            .ok_or("Value is not a (u32, u32, u32, u32)")?;

        let mut framebuffer = UncachedHeapMemory::<u16>::new_with_align(TOTAL, 64);
        // Prefill both framebuffers with the same recognizable pattern so untouched pixels are
        // compared as well.
        let mut soft_bytes = vec![0u8; TOTAL * 2];
        for i in 0..TOTAL {
            let value = 0x0100u16.wrapping_add(i as u16);
            framebuffer.write(i, value);
            soft_bytes[i * 2] = (value >> 8) as u8;
            soft_bytes[i * 2 + 1] = value as u8;
        }

        // Two different colors so the differential also checks which fill color half lands on
        // even/odd pixels.
        let color_even = RGBA5551::new(u5::new(31), u5::new(0), u5::new(0), true);
        let color_odd = RGBA5551::new(u5::new(0), u5::new(0), u5::new(31), false);

        let mut assembler = RDPAssembler::new();
        assembler.set_framebuffer_image(
            Format::RGBA,
            PixelSize::Bits16,
            u12::new((WIDTH - 1).try_into().unwrap()),
            &mut framebuffer,
        );
        assembler.set_scissor(&RDPRectangle::new(
            U10_2::from_u32(0),
            U10_2::from_u32(0),
            U10_2::from_usize(WIDTH),
            U10_2::from_usize(HEIGHT),
        ));
        assembler.set_othermode(Othermode::DEFAULT.with_cycle_type(CycleType::Fill));
        assembler.set_fillcolor16(color_even, color_odd);
        assembler.filled_rectangle(&RDPRectangle::new(
            U10_2::new_with_masked_value(left),
            U10_2::new_with_masked_value(top),
            U10_2::new_with_masked_value(right),
            U10_2::new_with_masked_value(bottom),
        ));
        assembler.sync_full();

        RDP::run_and_wait(&mut assembler);

        // Run the identical command words through the soft-RDP
        let stream: Vec<u64> = (0..assembler.len()).map(|i| assembler.word(i)).collect();
        let mut soft = SoftRdp::new();
        {
            let mut hidden = vec![0u8; TOTAL];
            let mut rdram = SliceRdram::new(
                framebuffer.start_physical() as u32,
                &mut soft_bytes,
                &mut hidden,
            );
            soft.run(&stream, &mut rdram);
        }
        soft_assert_eq(soft.unhandled, 0, "SoftRdp hit unhandled commands")?;

        for y in 0..HEIGHT + GUARD_ROWS {
            for x in 0..WIDTH {
                let i = y * WIDTH + x;
                let hardware = framebuffer.read(i);
                let soft_value = ((soft_bytes[i * 2] as u16) << 8) | (soft_bytes[i * 2 + 1] as u16);
                if soft_value != hardware {
                    // Dump the whole hardware row to make the mismatch pattern visible on screen
                    let mut row = String::new();
                    for x2 in 0..WIDTH {
                        let value = framebuffer.read(y * WIDTH + x2);
                        row.push_str(&format!("{:04x} ", value));
                    }
                    soft_assert_eq2(soft_value, hardware, || {
                        format!(
                            "Rect raw ({}, {})..=({}, {}): pixel ({}, {}) soft-RDP vs hardware. HW row: {}",
                            left, top, right, bottom, x, y, row
                        )
                    })?;
                }
            }
        }

        Ok(())
    }
}
