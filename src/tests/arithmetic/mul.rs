use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use oorandom::Rand64;

use crate::assembler::{Assembler, GPR};
use crate::tests::soft_asserts::soft_assert_eq;
use crate::tests::{Level, Test};

// Tests the various MULT instructions
//  MULT: Takes rt as a 35-bit value and rs as a 64-bit value (both signed). Multiplies into 64 bit. The result is split into Hi/Lo, sign extending the upper 32 bits
//  MULTU: Takes both operands as 32 bit unsigned values, multiplies into 64 bit. The result is split into Hi/Lo, sign extending the upper 32 bits
//  DMULT: Takes both operands as 64 bit signed value, multiplies into 128 bits. The result is written into Hi/Lo
//  DMULTU: Takes both operands as 64 bit unsigned value, multiplies into 128 bits. The result is written into Hi/Lo
fn mult<const INSTRUCTION: u32>(f1: u64, f2: u64) -> u128 {
    let lo: u64;
    let hi: u64;
    unsafe {
        asm!("
            .set noat
            MTLO $0
            MTHI $0
            NOP
            NOP
            .word {INSTRUCTION}
            NOP
            NOP
            MFLO {lo}
            MFHI {hi}
        ",
        INSTRUCTION = const INSTRUCTION,
        in("$2") f1,
        in("$3") f2,
        lo = out(reg) lo,
        hi = out(reg) hi)
    }
    ((hi as u128) << 64) | (lo as u128)
}

pub struct MULT {}

impl Test for MULT {
    fn name(&self) -> &str {
        "MULT"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((
                0x00000000_00000000u64,
                0x00000000_00000000u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x00000000_00000000u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000000u64,
                0x00000000_00000010u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x00000000_00000010u64,
                0x00000000_00000000_00000000_00000100u128,
            )),
            Box::new((
                0x00000000_FFFFFFFFu64,
                0x00000000_00000010u64,
                0x00000000_0000000F_FFFFFFFF_FFFFFFF0u128,
            )),
            Box::new((
                0x000000EE_FFFFFFFFu64,
                0x00000000_00000010u64,
                0x00000000_00000EEF_FFFFFFFF_FFFFFFF0u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x000000EE_FFFFFFFFu64,
                0xFFFFFFFF_FFFFFFEF_FFFFFFFF_FFFFFFF0u128,
            )),
            Box::new((
                0x00000000_12345678u64,
                0x00000000_00012334u64,
                0x00000000_000014B5_00000000_30EBF860u128,
            )),
            Box::new((
                0x00000000_00012334u64,
                0x00000000_12345678u64,
                0x00000000_000014B5_00000000_30EBF860u128,
            )),
            Box::new((
                0x00000FF0_12345678u64,
                0x00000000_00012334u64,
                0x00000000_12212175_00000000_30EBF860u128,
            )),
            Box::new((
                0x00000000_00012334u64,
                0x00000FF0_12345678u64,
                0x00000000_000014B5_00000000_30EBF860u128,
            )),
            Box::new((
                0xFFFFFFFF_FFFFFFFFu64,
                0x00000000_00000010u64,
                0xFFFFFFFF_FFFFFFFF_FFFFFFFF_FFFFFFF0u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0xFFFFFFFF_FFFFFFFF_FFFFFFFF_FFFFFFF0u128,
            )),
            Box::new((
                0x0000000F_70000001u64,
                0x00000000_00000010u64,
                0x00000000_000000F7_00000000_00000010u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x0000000F_70000001u64,
                0xFFFFFFFF_FFFFFFF7_00000000_00000010u128,
            )),
            Box::new((
                0x0000000F_00000000u64,
                0x00000000_00000010u64,
                0x00000000_000000F0_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x0000000F_00000000u64,
                0xFFFFFFFF_FFFFFFF0_00000000_00000000u128,
            )),
            Box::new((
                0x0000000C_00000000u64,
                0x00000000_00000001u64,
                0x00000000_0000000C_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000001u64,
                0x0000000C_00000000u64,
                0xFFFFFFFF_FFFFFFFC_00000000_00000000u128,
            )),
            Box::new((
                0x00000002_00000000u64,
                0x00000000_00000001u64,
                0x00000000_00000002_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000001u64,
                0x00000002_00000000u64,
                0x00000000_00000002_00000000_00000000u128,
            )),
            Box::new((
                0x00000004_00000000u64,
                0x00000000_00000001u64,
                0x00000000_00000004_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000001u64,
                0x00000004_00000000u64,
                0xFFFFFFFF_FFFFFFFC_00000000_00000000u128,
            )),
            Box::new((
                0x00000008_00000000u64,
                0x00000000_00000001u64,
                0x00000000_00000008_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000001u64,
                0x00000008_00000000u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000040_00000000u64,
                0x00000000_00000001u64,
                0x00000000_00000040_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000001u64,
                0x00000040_00000000u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000008_00000000u64,
                0x00000000_00000001u64,
                0x00000000_00000008_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000001u64,
                0x00000008_00000000u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u64, u128)>() {
            Some((f1, f2, expected_result)) => {
                const INSTRUCTION: u32 = Assembler::make_mult(GPR::V1, GPR::V0);
                let result = mult::<INSTRUCTION>(*f1, *f2);
                soft_assert_eq(result, *expected_result, "Result")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct MULTRandomized {}

impl Test for MULTRandomized {
    fn name(&self) -> &str {
        "MULT (randomized)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let mut random = Rand64::new(0);
        for _ in 0..50000 {
            let f1 = random.rand_u64() as i64;
            let f2 = random.rand_u64() as i64;

            let lo: u64;
            let hi: u64;
            unsafe {
                asm!("
                    MULT {f1}, {f2}
                    MFLO {lo}
                    MFHI {hi}
                ",
                f1 = in(reg) f1,
                f2 = in(reg) f2,
                lo = out(reg) lo,
                hi = out(reg) hi);
            }

            // Expected: rt is a 35-bit number, so simulate that here
            // We can use regular Rust multiplication as it will use DMULT. Besides, MULT with upper bits that aren't
            // sign extension is undefined
            let f2 = (f2 << 29) >> 29;
            let expected: i64 = f1 * f2;
            soft_assert_eq(expected as i32 as u64, lo, "MultLo")?;
            soft_assert_eq((expected >> 32) as i32 as u64, hi, "MultHi")?;
        }

        Ok(())
    }
}

pub struct MULTU {}

impl Test for MULTU {
    fn name(&self) -> &str {
        "MULTU"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((
                0x00000000_00000000u64,
                0x00000000_00000000u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x00000000_00000000u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000000u64,
                0x00000000_00000010u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x00000000_00000010u64,
                0x00000000_00000000_00000000_00000100u128,
            )),
            Box::new((
                0x00000000_FFFFFFFFu64,
                0x00000000_00000010u64,
                0x00000000_0000000F_FFFFFFFF_FFFFFFF0u128,
            )),
            Box::new((
                0x000000EE_FFFFFFFFu64,
                0x00000000_00000010u64,
                0x00000000_0000000F_FFFFFFFF_FFFFFFF0u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x000000EE_FFFFFFFFu64,
                0x00000000_0000000F_FFFFFFFF_FFFFFFF0u128,
            )),
            Box::new((
                0x00000000_12345678u64,
                0x00000000_00012334u64,
                0x00000000_000014B5_00000000_30EBF860u128,
            )),
            Box::new((
                0xFFFFFFFF_FFFFFFFFu64,
                0x00000000_00000010u64,
                0x00000000_0000000F_FFFFFFFF_FFFFFFF0u128,
            )),
            Box::new((
                0xFFFFFFFF_7FFFFFF1u64,
                0x00000000_00000010u64,
                0x00000000_00000007_FFFFFFFF_FFFFFF10u128,
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u64, u128)>() {
            Some((f1, f2, expected_result)) => {
                const INSTRUCTION: u32 = Assembler::make_multu(GPR::V0, GPR::V1);
                let result = mult::<INSTRUCTION>(*f1, *f2);
                soft_assert_eq(result, *expected_result, "Result")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct DMULT {}

impl Test for DMULT {
    fn name(&self) -> &str {
        "DMULT"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((
                0x00000000_00000000u64,
                0x00000000_00000000u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x00000000_00000000u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000000u64,
                0x00000000_00000010u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x00000000_00000010u64,
                0x00000000_00000000_00000000_00000100u128,
            )),
            Box::new((
                0x00000000_FFFFFFFFu64,
                0x00000000_00000010u64,
                0x00000000_00000000_0000000F_FFFFFFF0u128,
            )),
            Box::new((
                0x000000EE_FFFFFFFFu64,
                0x00000000_00000010u64,
                0x00000000_00000000_00000EEF_FFFFFFF0u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x000000EE_FFFFFFFFu64,
                0x00000000_00000000_00000EEF_FFFFFFF0u128,
            )),
            Box::new((
                0x00000000_12345678u64,
                0x00000000_00012334u64,
                0x00000000_00000000_000014B5_30EBF860u128,
            )),
            Box::new((
                0xFFFFFFFF_FFFFFFFFu64,
                0x00000000_7FFFFFFFu64,
                0xFFFFFFFF_FFFFFFFF_FFFFFFFF_80000001u128,
            )),
            Box::new((
                0x80000000_00000001u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0x00000000_00000000_7FFFFFFF_FFFFFFFFu128,
            )),
            Box::new((
                0x80000000_00000000u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0x00000000_00000008_0000000_00000000u128,
            )),
            Box::new((
                0x80000000_00000000u64,
                0x80000000_00000000u64,
                0x40000000_00000000_00000000_00000000u128,
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u64, u128)>() {
            Some((f1, f2, expected_result)) => {
                const INSTRUCTION: u32 = Assembler::make_dmult(GPR::V0, GPR::V1);
                let result = mult::<INSTRUCTION>(*f1, *f2);
                soft_assert_eq(result, *expected_result, "Result")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct DMULTU {}

impl Test for DMULTU {
    fn name(&self) -> &str {
        "DMULTU"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((
                0x00000000_00000000u64,
                0x00000000_00000000u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x00000000_00000000u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000000u64,
                0x00000000_00000010u64,
                0x00000000_00000000_00000000_00000000u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x00000000_00000010u64,
                0x00000000_00000000_00000000_00000100u128,
            )),
            Box::new((
                0x00000000_FFFFFFFFu64,
                0x00000000_00000010u64,
                0x00000000_00000000_0000000F_FFFFFFF0u128,
            )),
            Box::new((
                0x000000EE_FFFFFFFFu64,
                0x00000000_00000010u64,
                0x00000000_00000000_00000EEF_FFFFFFF0u128,
            )),
            Box::new((
                0x00000000_00000010u64,
                0x000000EE_FFFFFFFFu64,
                0x00000000_00000000_00000EEF_FFFFFFF0u128,
            )),
            Box::new((
                0x00000000_12345678u64,
                0x00000000_00012334u64,
                0x00000000_00000000_000014B5_30EBF860u128,
            )),
            Box::new((
                0xFFFFFFFF_FFFFFFFFu64,
                0x00000000_7FFFFFFFu64,
                0x00000000_7FFFFFFE_FFFFFFFF_80000001u128,
            )),
            Box::new((
                0x80000000_00000001u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0x80000000_00000000_7FFFFFFF_FFFFFFFFu128,
            )),
            Box::new((
                0x80000000_00000000u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0x7FFFFFFF_FFFFFFFF_80000000_00000000u128,
            )),
            Box::new((
                0x80000000_00000000u64,
                0x80000000_00000000u64,
                0x40000000_00000000_00000000_00000000u128,
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u64, u128)>() {
            Some((f1, f2, expected_result)) => {
                const INSTRUCTION: u32 = Assembler::make_dmultu(GPR::V0, GPR::V1);
                let result = mult::<INSTRUCTION>(*f1, *f2);
                soft_assert_eq(result, *expected_result, "Result")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}
