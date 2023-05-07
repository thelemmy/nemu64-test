use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::{format, vec};
use core::any::Any;
use core::arch::asm;

use crate::memory_map::MemoryMap;
use crate::tests::soft_asserts::soft_assert_eq;
use crate::tests::{Level, Test};

pub struct Read00 {}

impl Test for Read00 {
    fn name(&self) -> &str {
        "rdram-regs: Read 0x00"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![Box::new(true), Box::new(false)]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let cached = value.downcast_ref::<bool>().unwrap();
        let address = MemoryMap::addr32_to_usize(if *cached { 0x83F00000 } else { 0xA3F00000 });
        let mut result1: u64 = 0xFEDCBA98_76543210;
        let mut result2: u64 = 0xFEDCBA98_76543210;
        let mut result3: u64 = 0xFEDCBA98_76543210;
        let mut result4: u64 = 0xFEDCBA98_76543210;
        let mut result5: u64 = 0xFEDCBA98_76543210;
        let mut result6: u64 = 0xFEDCBA98_76543210;
        unsafe {
            asm!("
                .set noat
                LD {scratch}, 0 ({address})
                SD {scratch}, 0 ({result1})

                LW {scratch}, 0 ({address})
                SD {scratch}, 0 ({result2})

                LH {scratch}, 0 ({address})
                SD {scratch}, 0 ({result3})

                LB {scratch}, 0 ({address})
                SD {scratch}, 0 ({result4})

                LB {scratch}, 1 ({address})
                SD {scratch}, 0 ({result5})

                LD {scratch}, 0 ({result6})
                LWL {scratch}, 1 ({address})
                SD {scratch}, 0 ({result6})
            ",
            address = in(reg) address,
            scratch = out(reg) _,
            result1 = in(reg) &mut result1,
            result2 = in(reg) &mut result2,
            result3 = in(reg) &mut result3,
            result4 = in(reg) &mut result4,
            result5 = in(reg) &mut result5,
            result6 = in(reg) &mut result6,
            )
        }

        soft_assert_eq(result1, 0xb4190010_00000000, "LD from RDRAM REG (Config)")?;
        soft_assert_eq(result2, 0xFFFFFFFF_b4190010, "LW from RDRAM REG (Config)")?;
        soft_assert_eq(result3, 0xFFFFFFFF_FFFFb419, "LH from RDRAM REG (Config)")?;
        soft_assert_eq(result4, 0xFFFFFFFF_FFFFFFb4, "LB from RDRAM REG (Config)")?;
        soft_assert_eq(
            result5,
            0x00000000_00000019,
            "LB from RDRAM REG (Config) (+1)",
        )?;
        soft_assert_eq(
            result6,
            0x00000000_19001010,
            "LDL from RDRAM REG (Config) (+1)",
        )?;

        Ok(())
    }
}

pub struct ReadMore {}

impl Test for ReadMore {
    fn name(&self) -> &str {
        "rdram-regs: Read more"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        fn read_next(p: &mut *const u32) -> u32 {
            unsafe {
                let result = p.read_volatile();
                *p = (*p).add(1);
                result
            }
        }

        fn expect_array(p: &mut *const u32, a: &[u32]) -> Result<(), String> {
            for i in 0..a.len() {
                let value = read_next(p);
                soft_assert_eq(
                    value,
                    a[i],
                    format!("Reading 0x{:x}", (*p) as usize - 4).as_str(),
                )?;
            }
            Ok(())
        }

        // Not sure what these values are but they match what's seen on real hardware (both with
        // and without expansion pack)
        const EXPECTED_A: [u32; 16] = [
            0xb4190010, 0, 0x2b3b1a0b, 0, 0, 0, 0x101c0a04, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let mut p = MemoryMap::addr32_to_usize(0xA3F00000) as *const u32;
        for _ in 0..8 {
            expect_array(&mut p, &EXPECTED_A)?;
        }

        Ok(())
    }
}
