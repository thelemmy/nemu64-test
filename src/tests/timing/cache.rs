use alloc::alloc::{alloc, dealloc, Layout};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;
use core::cmp::{max, min};
use core::ops::RangeInclusive;

use crate::assembler::{Assembler, GPR};
use crate::cop0::{RegisterIndex, Status};
use crate::memory_map::MemoryMap;
use crate::tests::soft_asserts::{
    soft_assert_eq, soft_assert_eq_with_epsilon, soft_assert_range_contained_within_expected,
};
use crate::tests::timing::{effective_cycles, ExceptionTimingMode, MeasurementProgram};
use crate::tests::{Level, Test};

// TODO: Time CACHE instruction
// TODO: Time writes, in particular once the write buffer has been filled

/// This tests determines the amount of cache
pub struct CacheSizeTest {}

impl Test for CacheSizeTest {
    fn name(&self) -> &str {
        "Timing: Data cache Size"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        fn fits_within_cache(size: usize, pagesize: usize) -> bool {
            assert!(size % pagesize == 0);
            let layout = Layout::from_size_align(size, 1024).unwrap();
            let test_data = unsafe { alloc(layout) } as *mut u32;

            unsafe {
                asm!("
                    .set noat
                    .set noreorder
1:
                    ORI {inner_size}, {size}, 0
                    ORI {inner_pointer}, {test_data}, 0
2:
                    // Make sure to have an even number of instructions in the inner loop
                    MFC0 {count}, ${COUNT}
                    SUBU {inner_size}, {inner_size}, {pagesize}
                    SW {count}, 0({inner_pointer})
                    ADDU {inner_pointer}, {inner_pointer}, {pagesize}
                    BNE {inner_size}, $0, 2b
                    NOP

                    ADDIU {counter}, {counter}, -1
                    BNE {counter}, $0, 1b
                    NOP
                ",
                count = out(reg) _,
                size = in(reg) size,
                inner_size = out(reg) _,
                inner_pointer = out(reg) _,
                test_data = in(reg) test_data,
                counter = inout(reg) 2 => _,
                pagesize = in(reg) pagesize,
                COUNT = const RegisterIndex::Count as usize);
            }

            let mut previous = unsafe { test_data.read() };
            let mut result = true;
            for i in (pagesize..size).step_by(pagesize) {
                let value = unsafe { test_data.add(i >> 2).read() };
                let difference = value - previous;
                previous = value;

                if difference != 3 {
                    result = false;
                    break;
                }
            }
            unsafe {
                dealloc(test_data as *mut u8, layout);
            }

            result
        }

        soft_assert_eq(
            true,
            fits_within_cache(2 * 1024, 16),
            "2kb should fit within data cache",
        )?;
        soft_assert_eq(
            true,
            fits_within_cache(4 * 1024, 16),
            "4kb should fit within data cache",
        )?;
        soft_assert_eq(
            true,
            fits_within_cache(8 * 1024, 16),
            "8kb should fit within data cache",
        )?;
        soft_assert_eq(
            false,
            fits_within_cache(16 * 1024, 16),
            "16kb should not fit within data cache",
        )?;

        Ok(())
    }
}

#[derive(Debug)]
struct AveragedCycles {
    range: RangeInclusive<u32>,
    median: u32,
    average: f32,
}

fn get_averaged_cycles_with_codegen(
    iterations: usize,
    configure_preconditions: &[u32],
    execute: &[u32],
) -> AveragedCycles {
    // Generate the measurement loop, preconditions and execute into a single buffer so their
    // icache placement is fixed relative to each other (see MeasurementProgram)
    let mut program = MeasurementProgram::new(Some(configure_preconditions), execute);

    let mut sum = 0u64;
    let mut min_value = u32::MAX;
    let mut max_value = 0;
    let mut all_cycles = Vec::new();
    for i in 0..iterations {
        let (ticks1, ticks2) = program.run(0, 0, Status::DEFAULT, ExceptionTimingMode::Off);
        let cycles = effective_cycles(ticks1, ticks2, ExceptionTimingMode::Off);
        // If too many cycles, stop pushing. This avoids running out of memory
        if i < 100_000 {
            all_cycles.push(cycles);
        }
        sum += cycles as u64;
        min_value = min(min_value, cycles);
        max_value = max(max_value, cycles);
    }
    all_cycles.sort();

    AveragedCycles {
        range: min_value..=max_value,
        median: all_cycles[all_cycles.len() >> 1],
        average: sum as f32 / iterations as f32,
    }
}

fn assert_averaged_cycles_with_codegen(
    expected_range: RangeInclusive<u32>,
    expected_median: u32,
    expected_average: f32,
    average_epsilon: f32,
    configure_preconditions: &[u32],
    execute: &[u32],
) -> Result<(), String> {
    let averaged_cycles = get_averaged_cycles_with_codegen(1_000, configure_preconditions, execute);

    // crate::println!("Avg {} Med {} ({}..={})", averaged_cycles.average, averaged_cycles.median, averaged_cycles.range.start(), averaged_cycles.range.end());

    soft_assert_range_contained_within_expected(
        expected_range,
        averaged_cycles.range,
        "Cycle count (min and max)",
    )?;
    soft_assert_eq_with_epsilon(
        1,
        averaged_cycles.median,
        expected_median,
        "Median cycle count",
    )?;
    soft_assert_eq_with_epsilon(
        average_epsilon,
        averaged_cycles.average,
        expected_average,
        "Average cycle count",
    )?;

    Ok(())
}

pub struct LoadMissVIEnabled {}

impl Test for LoadMissVIEnabled {
    fn name(&self) -> &str {
        "Timing: Load Miss (with VI enabled)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![Box::new(true), Box::new(false)]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<bool>() {
            Some(in_same_bank_as_vi) => {
                let frontbuffer_address = crate::VIDEO.lock().frontbuffer_physical_address();
                let frontbuffer_bank_base = frontbuffer_address & 0x00700000;
                let base_address = if *in_same_bank_as_vi {
                    0x80000000 | frontbuffer_bank_base
                } else {
                    0x80000000
                        | ((frontbuffer_bank_base + 1 * 1024 * 1024)
                            & (MemoryMap::memory_size() as u32 - 1))
                };
                crate::VIDEO.lock().spinwait_for_vsync();
                assert_averaged_cycles_with_codegen(
                    38..=103,
                    42,
                    43.25f32,
                    1.0f32,
                    &[
                        Assembler::make_lui(GPR::T2, (base_address >> 16) as i16),
                        Assembler::make_ori(GPR::T2, GPR::T2, base_address as u16),
                        // Load the same cache line in the next 8k block. This guarantees that below we'll have a cache miss
                        Assembler::make_lw(GPR::R0, 8 * 1024, GPR::T2),
                    ],
                    &[Assembler::make_lw(GPR::R0, 0, GPR::T2)],
                )
            }
            _ => Err(format!("Unhandled pattern")),
        }
    }
}

pub struct LoadMissVIDisabled {}

impl Test for LoadMissVIDisabled {
    fn name(&self) -> &str {
        "Timing: Load Miss (with VI disabled)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new(0x80000000u32 + 0 * 1024 * 1024),
            Box::new(0x80000000u32 + 1 * 1024 * 1024),
            Box::new(0x80000000u32 + 2 * 1024 * 1024),
            Box::new(0x80000000u32 + 3 * 1024 * 1024),
            Box::new(0x80000000u32 + 4 * 1024 * 1024),
            Box::new(0x80000000u32 + 5 * 1024 * 1024),
            Box::new(0x80000000u32 + 6 * 1024 * 1024),
            Box::new(0x80000000u32 + 7 * 1024 * 1024),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let lock = crate::VIDEO.lock();
        let _disable = lock.disable_video();
        match (*value).downcast_ref::<u32>() {
            Some(base_address) => {
                assert_averaged_cycles_with_codegen(
                    41..=103,
                    41,
                    42.5f32,
                    0.5f32,
                    &[
                        Assembler::make_lui(GPR::T2, (*base_address >> 16) as i16),
                        Assembler::make_ori(GPR::T2, GPR::T2, *base_address as u16),
                        // Load the same cache line in the next 8k block. This guarantees that below we'll have a cache miss
                        Assembler::make_lw(GPR::R0, 8 * 1024, GPR::T2),
                    ],
                    &[Assembler::make_lw(GPR::R0, 0, GPR::T2)],
                )
            }
            _ => Err(format!("Unhandled pattern")),
        }
    }
}

pub struct LoadFromUncachedVIEnabled {}

impl Test for LoadFromUncachedVIEnabled {
    fn name(&self) -> &str {
        "Timing: Load from uncached (with VI enabled)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            // These measurements are all pretty unstable - will need to revise as things are better understood.
            // The in-same-bank average was re-measured (very stably) at ~36.3 after the measurement
            // loop moved into the generated buffer; the wide epsilon stays to capture the uncertainty.
            Box::new((true, 36u32, 36.3f32)),
            Box::new((false, 32u32, 32.5f32)),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(bool, u32, f32)>() {
            Some((in_same_bank_as_vi, expected_median, expected_average)) => {
                let frontbuffer_address = crate::VIDEO.lock().frontbuffer_physical_address();
                let frontbuffer_bank_base = frontbuffer_address & 0x00700000;
                let base_address = if *in_same_bank_as_vi {
                    0xA0000000 | frontbuffer_bank_base
                } else {
                    0xA0000000
                        | ((frontbuffer_bank_base + 1 * 1024 * 1024)
                            & (MemoryMap::memory_size() as u32 - 1))
                };
                crate::VIDEO.lock().spinwait_for_vsync();
                assert_averaged_cycles_with_codegen(
                    32..=93,
                    *expected_median,
                    *expected_average,
                    4.0f32,
                    &[
                        Assembler::make_lui(GPR::T2, (base_address >> 16) as i16),
                        Assembler::make_ori(GPR::T2, GPR::T2, base_address as u16),
                    ],
                    &[Assembler::make_lw(GPR::R0, 0, GPR::T2)],
                )
            }
            _ => Err(format!("Unhandled pattern")),
        }
    }
}

pub struct LoadFromUncachedVIDisabled {}

impl Test for LoadFromUncachedVIDisabled {
    fn name(&self) -> &str {
        "Timing: Load from uncached (with VI disabled)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new(0xA0000000u32 + 0 * 1024 * 1024),
            Box::new(0xA0000000u32 + 1 * 1024 * 1024),
            Box::new(0xA0000000u32 + 2 * 1024 * 1024),
            Box::new(0xA0000000u32 + 3 * 1024 * 1024),
            Box::new(0xA0000000u32 + 4 * 1024 * 1024),
            Box::new(0xA0000000u32 + 5 * 1024 * 1024),
            Box::new(0xA0000000u32 + 6 * 1024 * 1024),
            Box::new(0xA0000000u32 + 7 * 1024 * 1024),
            Box::new(0xA0000000u32 + 7 * 1024 * 1024),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let lock = crate::VIDEO.lock();
        let _disable = lock.disable_video();
        match (*value).downcast_ref::<u32>() {
            Some(base_address) => assert_averaged_cycles_with_codegen(
                32..=93,
                32,
                32.54f32,
                1.0f32,
                &[
                    Assembler::make_lui(GPR::T2, (*base_address >> 16) as i16),
                    Assembler::make_ori(GPR::T2, GPR::T2, *base_address as u16),
                ],
                &[Assembler::make_lw(GPR::R0, 0, GPR::T2)],
            ),
            _ => Err(format!("Unhandled pattern")),
        }
    }
}
