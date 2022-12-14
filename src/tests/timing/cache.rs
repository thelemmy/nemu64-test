use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use core::any::Any;
use core::arch::asm;
use core::mem::transmute;
use core::cmp::{max, min};
use core::ops::RangeInclusive;
use alloc::alloc::{alloc, dealloc, Layout};
use crate::assembler::{Assembler, GPR};
use crate::cop0::RegisterIndex;
use crate::memory_map::MemoryMap;

use crate::tests::{Level, Test};
use crate::tests::soft_asserts::{soft_assert_eq, soft_assert_eq_with_epsilon, soft_assert_range_contained_within_expected};
use crate::tests::timing::ExceptionTimingMode;
use crate::uncached_memory::{UncachedHeapMemory, UncachedHeapMemoryWriter};

// TODO: Time CACHE instruction
// TODO: Time writes, in particular once the write buffer has been filled

/// This tests determines the amount of cache
pub struct CacheSizeTest {

}

impl Test for CacheSizeTest {
    fn name(&self) -> &str { "Timing: Data cache Size" }

    fn level(&self) -> Level { Level::Timing }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

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
            unsafe { dealloc(test_data as *mut u8, layout); }

            result
        }

        soft_assert_eq(true, fits_within_cache(2 * 1024, 16), "2kb should fit within data cache")?;
        soft_assert_eq(true, fits_within_cache(4 * 1024, 16), "4kb should fit within data cache")?;
        soft_assert_eq(true, fits_within_cache(8 * 1024, 16), "8kb should fit within data cache")?;
        soft_assert_eq(false, fits_within_cache(16 * 1024, 16), "16kb should not fit within data cache")?;

        Ok(())
    }
}

#[inline(never)]
fn get_cycles(memory: &mut UncachedHeapMemory<u64>, configure_preconditions: extern "C" fn(), execute: extern "C" fn()) -> u32 {
    let ticks1: u32;
    let ticks2: u32;
    unsafe {
        // A couple of issues this test harness fixes:
        // - ICACHE: The first time the test runs, it is freshly loaded from ROM, introducing
        //   delays. Solve this by running twice
        // - COUNT increments at half-intervals, so when we're measuring cycles, half-cycles
        //   (e.g. 1.5) can be rounded up or down. Solution: Write to COUNT, as that
        //   calibrates.
        // - In order to measure half-cycles we can then add an additional NOP in another run.
        asm!("
            .set noat
            .set noreorder
            // stash RA
            ORI $20, $31, 0

            NOP; NOP

            .align 5 // align so that the loop below fits within the fewest ICACHE cachelines as possible
1:
            ORI $26, $18, 0  // Configure exception handler
            OR $6, $2, $0
            OR $7, $3, $0
            OR $8, $4, $0
            OR $9, $5, $0

            JALR $16
            NOP

            // Calibration step
            MFC0 $19, ${COUNT}
            NOP
            NOP
            MTC0 $19, ${COUNT}
            NOP

            // If loop counter is 2, skip an instruction. This shifts half of a cycle compared to the last round
            ORI $19, $0, 2
            BEQ $22, $19, 2f
            NOP  // delay
            NOP  // an extra instruction that is skipped if $22 == 2
2:

            MFC0 $19, ${COUNT}
            JALR $25
            NOP
            MFC0 $21, ${COUNT}

            // If enabled, don't use the timing value we just got but instead use k1, which is what the exception handler stored
            ORI $17, $0, 1
            BNE $17, $18, 3f
            NOP
            ORI $21, $27, 0
3:
            ORI $23, $24, 0     // stash previous iteration's result in 23
            SUB $24, $21, $19
            ADDIU $22, $22, 0xFFFF

            // Loop a bit - this clears out the write-buffer if it was used by the test
            ORI $19, $0, 70
4:
            BNE $19, $0, 4b
            ADDIU $19, $19, -1 // delay slot

            BNE $22, $0, 1b
            NOP // delay slot
            ORI $31, $20, 0  // restore RA
            ORI $26, $0, 0  // restore normal exception handler behavior
        ",
        COUNT = const RegisterIndex::Count as usize,

        in("$3") MemoryMap::physical_to_cached_mut::<u64>(memory.start_physical()),
        in("$5") MemoryMap::physical_to_uncached_mut::<u64>(memory.start_physical()),

        // These are freely to be used to by test
        out("$6") _,
        out("$7") _,
        out("$8") _,
        out("$9") _,
        out("$10") _,
        out("$11") _,
        out("$12") _,
        out("$13") _,
        out("$14") _,
        out("$15") _,

        // These are for the test infra itself
        in("$16") configure_preconditions,
        out("$17") _,
        in("$18") ExceptionTimingMode::Off as u32,
        out("$19") _,
        out("$20") _,
        out("$21") _,
        inout("$22") 3 => _,
        out("$23") ticks2,
        out("$24") ticks1,
        in("$25") execute,
        options(nostack));
    }

    let effective_ticks =
        // In addition to the code being measured, we need a few extra cycles:
        // - 1 for the JALR
        // - 1 for the JALR's NOP
        // - 1 for the return JR RA
        // - 1 for the delay of the JR RA
        // - 1 for one of the MFC0 COUNT itself
        (ticks1 + ticks2) - 5;

    effective_ticks
}

#[derive(Debug)]
struct AveragedCycles {
    range: RangeInclusive<u32>,
    median: u32,
    average: f32,
}

fn get_averaged_cycles_with_codegen<F: FnOnce(&mut UncachedHeapMemoryWriter<u32>), F2: FnOnce(&mut UncachedHeapMemoryWriter<u32>)>(iterations: usize, configure_preconditions: F, execute: F2) -> AveragedCycles {
    // Dynamically generate both functions
    let mut code_memory = UncachedHeapMemory::<u32>::new_with_align(64, 64);
    let mut writer = UncachedHeapMemoryWriter::new(&mut code_memory);

    configure_preconditions(&mut writer);
    writer.write(Assembler::make_jr(GPR::RA));
    writer.write(Assembler::make_nop());   // delay slot

    let execute_offset = writer.index() << 2;

    execute(&mut writer);
    writer.write(Assembler::make_jr(GPR::RA));
    writer.write(Assembler::make_nop());   // delay slot

    // Turn the pointer into a function pointer
    let preconditions_ptr: extern "C" fn() = unsafe { transmute(MemoryMap::physical_to_cached::<u8>(code_memory.start_physical())) };
    let execute_ptr: extern "C" fn() = unsafe { transmute(MemoryMap::physical_to_cached::<u8>(code_memory.start_physical() + execute_offset)) };

    let mut memory = UncachedHeapMemory::<u64>::new_with_align(16 * 1024, 64);
    let mut sum = 0u64;
    let mut min_value = u32::MAX;
    let mut max_value = 0;
    let mut all_cycles = Vec::new();
    for i in 0..iterations {
        let cycles = get_cycles(&mut memory, preconditions_ptr, execute_ptr);
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
        range : min_value..=max_value,
        median: all_cycles[all_cycles.len() >> 1],
        average : sum as f32 / iterations as f32,
    }
}

fn assert_averaged_cycles_with_codegen<F: FnOnce(&mut UncachedHeapMemoryWriter<u32>), F2: FnOnce(&mut UncachedHeapMemoryWriter<u32>)>(
    expected_range: RangeInclusive<u32>,
    expected_median: u32,
    expected_average: f32,
    average_epsilon: f32,
    configure_preconditions: F,
    execute: F2) -> Result<(), String> {
    let averaged_cycles = get_averaged_cycles_with_codegen(1_000, configure_preconditions, execute);

    // crate::println!("Avg {} Med {} ({}..={})", averaged_cycles.average, averaged_cycles.median, averaged_cycles.range.start(), averaged_cycles.range.end());

    soft_assert_range_contained_within_expected(expected_range, averaged_cycles.range, "Cycle count (min and max)")?;
    soft_assert_eq_with_epsilon(1, averaged_cycles.median, expected_median, "Median cycle count")?;
    soft_assert_eq_with_epsilon(average_epsilon, averaged_cycles.average, expected_average, "Average cycle count")?;

    Ok(())
}

pub struct LoadMissVIEnabled {

}

impl Test for LoadMissVIEnabled {
    fn name(&self) -> &str { "Timing: Load Miss (with VI enabled)" }

    fn level(&self) -> Level { Level::Timing }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            Box::new(true),
            Box::new(false),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<bool>() {
            Some(in_same_bank_as_vi) => {
                let frontbuffer_address = crate::VIDEO.lock().frontbuffer_physical_address();
                let frontbuffer_bank_base = frontbuffer_address & 0x00700000;
                let base_address = if *in_same_bank_as_vi {
                    0x80000000 | frontbuffer_bank_base
                } else {
                    0x80000000 | ((frontbuffer_bank_base + 1 * 1024 * 1024) & (MemoryMap::memory_size() as u32 - 1))
                };
                crate::VIDEO.lock().spinwait_for_vsync();
                assert_averaged_cycles_with_codegen(38..=103, 42, 43.25f32, 1.0f32, |writer| {
                    writer.write(Assembler::make_lui(GPR::T2, (base_address >> 16) as i16));
                    writer.write(Assembler::make_ori(GPR::T2, GPR::T2, base_address as u16));
                    // Load the same cache line in the next 8k block. This guarantees that below we'll have a cache miss
                    writer.write(Assembler::make_lw(GPR::R0, 8 * 1024, GPR::T2));
                }, |writer| {
                    writer.write(Assembler::make_lw(GPR::R0, 0, GPR::T2));
                })
            }
            _ => Err(format!("Unhandled pattern")),
        }
    }
}

pub struct LoadMissVIDisabled {

}

impl Test for LoadMissVIDisabled {
    fn name(&self) -> &str { "Timing: Load Miss (with VI disabled)" }

    fn level(&self) -> Level { Level::Timing }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            Box::new(0x80000000u32 + 0 * 1024 * 1024),
            Box::new(0x80000000u32 + 1 * 1024 * 1024),
            Box::new(0x80000000u32 + 2 * 1024 * 1024),
            Box::new(0x80000000u32 + 3 * 1024 * 1024),
            Box::new(0x80000000u32 + 4 * 1024 * 1024),
            Box::new(0x80000000u32 + 5 * 1024 * 1024),
            Box::new(0x80000000u32 + 6 * 1024 * 1024),
            Box::new(0x80000000u32 + 7 * 1024 * 1024),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let lock = crate::VIDEO.lock();
        let _disable = lock.disable_video();
        match (*value).downcast_ref::<u32>() {
            Some(base_address) => {
                assert_averaged_cycles_with_codegen(41..=103, 41, 42.5f32, 0.5f32, |writer| {
                    writer.write(Assembler::make_lui(GPR::T2, (*base_address >> 16) as i16));
                    writer.write(Assembler::make_ori(GPR::T2, GPR::T2, *base_address as u16));
                    // Load the same cache line in the next 8k block. This guarantees that below we'll have a cache miss
                    writer.write(Assembler::make_lw(GPR::R0, 8 * 1024, GPR::T2));
                }, |writer| {
                    writer.write(Assembler::make_lw(GPR::R0, 0, GPR::T2));
                })
            }
            _ => Err(format!("Unhandled pattern")),
        }
    }
}

pub struct LoadFromUncachedVIEnabled {

}

impl Test for LoadFromUncachedVIEnabled {
    fn name(&self) -> &str { "Timing: Load from uncached (with VI enabled)" }

    fn level(&self) -> Level { Level::Timing }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            // These measurements are all pretty unstable - will need to revise as things are better understood
            Box::new((true, 36u32, 40.7f32)),
            Box::new((false, 32u32, 32.5f32)),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(bool, u32, f32)>() {
            Some((in_same_bank_as_vi, expected_median, expected_average)) => {
                let frontbuffer_address = crate::VIDEO.lock().frontbuffer_physical_address();
                let frontbuffer_bank_base = frontbuffer_address & 0x00700000;
                let base_address = if *in_same_bank_as_vi {
                    0xA0000000 | frontbuffer_bank_base
                } else {
                    0xA0000000 | ((frontbuffer_bank_base + 1 * 1024 * 1024) & (MemoryMap::memory_size() as u32 - 1))
                };
                crate::VIDEO.lock().spinwait_for_vsync();
                assert_averaged_cycles_with_codegen(32..=93, *expected_median, *expected_average, 4.0f32, |writer| {
                    writer.write(Assembler::make_lui(GPR::T2, (base_address >> 16) as i16));
                    writer.write(Assembler::make_ori(GPR::T2, GPR::T2, base_address as u16));
                }, |writer| {
                    writer.write(Assembler::make_lw(GPR::R0, 0, GPR::T2));
                })
            }
            _ => Err(format!("Unhandled pattern")),
        }
    }
}


pub struct LoadFromUncachedVIDisabled {

}

impl Test for LoadFromUncachedVIDisabled {
    fn name(&self) -> &str { "Timing: Load from uncached (with VI disabled)" }

    fn level(&self) -> Level { Level::Timing }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            Box::new(0xA0000000u32 + 0 * 1024 * 1024),
            Box::new(0xA0000000u32 + 1 * 1024 * 1024),
            Box::new(0xA0000000u32 + 2 * 1024 * 1024),
            Box::new(0xA0000000u32 + 3 * 1024 * 1024),
            Box::new(0xA0000000u32 + 4 * 1024 * 1024),
            Box::new(0xA0000000u32 + 5 * 1024 * 1024),
            Box::new(0xA0000000u32 + 6 * 1024 * 1024),
            Box::new(0xA0000000u32 + 7 * 1024 * 1024),
            Box::new(0xA0000000u32 + 7 * 1024 * 1024),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let lock = crate::VIDEO.lock();
        let _disable = lock.disable_video();
        match (*value).downcast_ref::<u32>() {
            Some(base_address) => {
                assert_averaged_cycles_with_codegen(32..=93, 32, 32.54f32, 1.0f32, |writer| {
                    writer.write(Assembler::make_lui(GPR::T2, (*base_address >> 16) as i16));
                    writer.write(Assembler::make_ori(GPR::T2, GPR::T2, *base_address as u16));
                }, |writer| {
                    writer.write(Assembler::make_lw(GPR::R0, 0, GPR::T2));
                })
            }
            _ => Err(format!("Unhandled pattern")),
        }
    }
}


