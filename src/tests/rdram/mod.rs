pub mod rdram_regs;

use alloc::boxed::Box;
use alloc::{format, vec};
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;
use crate::assembler::{Assembler, GPR};
use crate::memory_map::MemoryMap;
use crate::tests::{Level, Test};
use crate::tests::soft_asserts::soft_assert_eq;
use crate::uncached_memory::UncachedHeapMemory;

const DATA: [u64; 3] = [0x01234567_89ABCDEF, 0x21436587_99BADCFE, 0xA9887766_55443322];

pub struct LWL {}

impl Test for LWL {
    fn name(&self) -> &str { "rdram: LWL" }

    fn level(&self) -> Level { Level::BasicFunctionality }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec!(
            Box::new(true),
            Box::new(false)
        )
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let cached = value.downcast_ref::<bool>().unwrap();
        const EXPECTED: [u64; 8] = [
            0x01234567,
            0x23456710,
            0x45673210,
            0x67543210,
            0xFFFFFFFF_89abcdef,
            0xFFFFFFFF_abcdef10,
            0xFFFFFFFF_cdef3210,
            0xFFFFFFFF_ef543210,
        ];
        let address = if *cached { &DATA[0] as *const u64 as usize } else { MemoryMap::uncached(&DATA[0] as *const u64) as usize };
        for i in 0..8 {
            let mut result: u64 = 0xFEDCBA98_76543210;
            unsafe {
                asm!("
                    LD {scratch}, 0 ({result})
                    LWL {scratch}, 0 ({address})
                    SD {scratch}, 0 ({result})
                ",
                address = in(reg) address + i,
                scratch = out(reg) _,
                result = in(reg) &mut result
                )
            }

            soft_assert_eq(result, EXPECTED[i], format!("LWL result with offset {}", i).as_str())?;
        }
        Ok(())
    }
}

pub struct LWR {}

impl Test for LWR {
    fn name(&self) -> &str { "rdram: LWR" }

    fn level(&self) -> Level { Level::BasicFunctionality }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec!(
            Box::new(true),
            Box::new(false)
        )
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let cached = value.downcast_ref::<bool>().unwrap();
        const EXPECTED: [u64; 8] = [
            0xFEDCBA98_76543201,
            0xFEDCBA98_76540123,
            0xFEDCBA98_76012345,
            0x01234567,
            0xFEDCBA98_76543289,
            0xFEDCBA98_765489ab,
            0xFEDCBA98_7689abcd,
            0xFFFFFFFF_89abcdef,
        ];
        let address = if *cached { &DATA[0] as *const u64 as usize } else { MemoryMap::uncached(&DATA[0] as *const u64) as usize };
        for i in 0..8 {
            let mut result: u64 = 0xFEDCBA98_76543210;
            unsafe {
                asm!("
                    LD {scratch}, 0 ({result})
                    LWR {scratch}, 0 ({address})
                    SD {scratch}, 0 ({result})
                ",
                address = in(reg) address + i,
                scratch = out(reg) _,
                result = in(reg) &mut result
                )
            }

            //ate::println!("0x{:x},", result);
            soft_assert_eq(result, EXPECTED[i], format!("LWR result with offset {}", i).as_str())?;
        }
        Ok(())
    }
}

pub struct LDL {}

impl Test for LDL {
    fn name(&self) -> &str { "rdram: LDL" }

    fn level(&self) -> Level { Level::BasicFunctionality }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec!(
            Box::new(true),
            Box::new(false)
        )
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let cached = value.downcast_ref::<bool>().unwrap();
        const EXPECTED: [u64; 8] = [
            0x01234567_89abcdef,
            0x23456789_abcdef10,
            0x456789ab_cdef3210,
            0x6789abcd_ef543210,
            0x89abcdef_76543210,
            0xabcdef98_76543210,
            0xcdefba98_76543210,
            0xefdcba98_76543210,
        ];
        let address = if *cached { &DATA[0] as *const u64 as usize } else { MemoryMap::uncached(&DATA[0] as *const u64) as usize };
        for i in 0..8 {
            let mut result: u64 = 0xFEDCBA98_76543210;
            unsafe {
                asm!("
                    LD {scratch}, 0 ({result})
                    LDL {scratch}, 0 ({address})
                    SD {scratch}, 0 ({result})
                ",
                address = in(reg) address + i,
                scratch = out(reg) _,
                result = in(reg) &mut result
                )
            }

            soft_assert_eq(result, EXPECTED[i], format!("LDL result with offset {}", i).as_str())?;
        }
        Ok(())
    }
}

pub struct LDR {}

impl Test for LDR {
    fn name(&self) -> &str { "rdram: LDR" }

    fn level(&self) -> Level { Level::BasicFunctionality }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec!(
            Box::new(true),
            Box::new(false)
        )
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let cached = value.downcast_ref::<bool>().unwrap();
        const EXPECTED: [u64; 8] = [
            0xfedcba98_76543201,
            0xfedcba98_76540123,
            0xfedcba98_76012345,
            0xfedcba98_01234567,
            0xfedcba01_23456789,
            0xfedc0123_456789ab,
            0xfe012345_6789abcd,
            0x01234567_89abcdef,
        ];
        let address = if *cached { &DATA[0] as *const u64 as usize } else { MemoryMap::uncached(&DATA[0] as *const u64) as usize };
        for i in 0..8 {
            let mut result: u64 = 0xFEDCBA98_76543210;
            unsafe {
                asm!("
                    LD {scratch}, 0 ({result})
                    LDR {scratch}, 0 ({address})
                    SD {scratch}, 0 ({result})
                ",
                address = in(reg) address + i,
                scratch = out(reg) _,
                result = in(reg) &mut result
                )
            }

            soft_assert_eq(result, EXPECTED[i], format!("LDR result with offset {}", i).as_str())?;
        }
        Ok(())
    }
}

fn test_unaligned_store<const INSTRUCTION: u32>(cached: bool, expected: [u64; 8]) -> Result<(), String> {
    let mut data = UncachedHeapMemory::<u32>::new_with_align((16 * 1024) >> 2, 8 * 1024);
    for i in 0..8 {
        data.write(4, 0x01234567);
        data.write(5, 0x89ABCDEF);
        // Also write some data 8kb later...that happens to be the next cache line
        data.write(4 + 2 * 1024, 0xCBBCCBBC);
        data.write(5 + 2 * 1024, 0xBAABBAAB);
        let physical = data.start_physical() + 16;
        let cached_address = MemoryMap::physical_to_cached_mut::<u8>(physical) as usize;
        let address = if cached { cached_address } else { MemoryMap::physical_to_uncached_mut::<u8>(physical) as usize };
        let mut value_and_result: u64 = 0xFEDCBA98_76543210;
        unsafe {
            asm!("
                LD $3, 0 ({value_and_result})

                // Load next cache line
                LD $0, 8 * 1024 ({address_aligned_cached})

                // Perform the store and get result
                .word {INSTRUCTION}

                // Move cache line again. For uncached access, this ensures that things actually get written
                LD $0, 8 * 1024 ({address_aligned_cached})

                LD $3, 0 ({address_aligned_cached})
                SD $3, 0 ({value_and_result})
            ",
            address_aligned_cached = in(reg) cached_address,
            value_and_result = in(reg) &mut value_and_result,
            INSTRUCTION = const INSTRUCTION,
            out("$3") _,
            in("$4") address + i,
            )
        }

        soft_assert_eq(value_and_result, expected[i], format!("Result with offset {}", i).as_str())?;
    }
    Ok(())
}

pub struct SWL {}

impl Test for SWL {
    fn name(&self) -> &str { "rdram: SWL" }

    fn level(&self) -> Level { Level::BasicFunctionality }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec!(
            Box::new(true),
            Box::new(false)
        )
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let cached = *value.downcast_ref::<bool>().unwrap();
        const EXPECTED: [u64; 8] = [
            0x76543210_89abcdef,
            0x01765432_89abcdef,
            0x01237654_89abcdef,
            0x01234576_89abcdef,
            0x01234567_76543210,
            0x01234567_89765432,
            0x01234567_89ab7654,
            0x01234567_89abcd76,
        ];
        const INSTRUCTION: u32 = Assembler::make_swl(GPR::V1, 0, GPR::A0);
        test_unaligned_store::<INSTRUCTION>(cached, EXPECTED)
    }
}

pub struct SWR {}

impl Test for SWR {
    fn name(&self) -> &str { "rdram: SWR" }

    fn level(&self) -> Level { Level::BasicFunctionality }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec!(
            Box::new(true),
            Box::new(false)
        )
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let cached = *value.downcast_ref::<bool>().unwrap();
        const EXPECTED: [u64; 8] = [
            0x10234567_89abcdef,
            0x32104567_89abcdef,
            0x54321067_89abcdef,
            0x76543210_89abcdef,
            0x01234567_10abcdef,
            0x01234567_3210cdef,
            0x01234567_543210ef,
            0x01234567_76543210,
        ];
        const INSTRUCTION: u32 = Assembler::make_swr(GPR::V1, 0, GPR::A0);
        test_unaligned_store::<INSTRUCTION>(cached, EXPECTED)
    }
}

pub struct SDL {}

impl Test for SDL {
    fn name(&self) -> &str { "rdram: SDL" }

    fn level(&self) -> Level { Level::BasicFunctionality }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec!(
            Box::new(true),
            Box::new(false)
        )
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let cached = *value.downcast_ref::<bool>().unwrap();
        const EXPECTED: [u64; 8] = [
            0xfedcba98_76543210,
            0x01fedcba_98765432,
            0x0123fedc_ba987654,
            0x012345fe_dcba9876,
            0x01234567_fedcba98,
            0x01234567_89fedcba,
            0x01234567_89abfedc,
            0x01234567_89abcdfe,
        ];
        const INSTRUCTION: u32 = Assembler::make_sdl(GPR::V1, 0, GPR::A0);
        test_unaligned_store::<INSTRUCTION>(cached, EXPECTED)
    }
}

pub struct SDR {}

impl Test for SDR {
    fn name(&self) -> &str { "rdram: SDR" }

    fn level(&self) -> Level { Level::BasicFunctionality }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec!(
            Box::new(true),
            Box::new(false)
        )
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let cached = *value.downcast_ref::<bool>().unwrap();
        const EXPECTED: [u64; 8] = [
            0x10234567_89abcdef,
            0x32104567_89abcdef,
            0x54321067_89abcdef,
            0x76543210_89abcdef,
            0x98765432_10abcdef,
            0xba987654_3210cdef,
            0xdcba9876_543210ef,
            0xfedcba98_76543210,
        ];
        const INSTRUCTION: u32 = Assembler::make_sdr(GPR::V1, 0, GPR::A0);
        test_unaligned_store::<INSTRUCTION>(cached, EXPECTED)
    }
}

