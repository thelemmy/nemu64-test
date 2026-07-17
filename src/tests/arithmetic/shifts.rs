use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use crate::tests::soft_asserts::soft_assert_eq;
use crate::tests::{Level, Test};

fn test_sra<const SHIFT_AMOUNT: u32>(source_value: u64) -> u64 {
    let result: u64;
    unsafe {
        asm!("SRA $3, $2, {SHIFT_AMOUNT}", SHIFT_AMOUNT = const SHIFT_AMOUNT, in("$2") source_value, out("$3") result)
    }
    result
}

fn test_srl<const SHIFT_AMOUNT: u32>(source_value: u64) -> u64 {
    let result: u64;
    unsafe {
        asm!("SRL $3, $2, {SHIFT_AMOUNT}", SHIFT_AMOUNT = const SHIFT_AMOUNT, in("$2") source_value, out("$3") result)
    }
    result
}

fn test_sll<const SHIFT_AMOUNT: u32>(source_value: u64) -> u64 {
    let result: u64;
    unsafe {
        asm!("SLL $3, $2, {SHIFT_AMOUNT}", SHIFT_AMOUNT = const SHIFT_AMOUNT, in("$2") source_value, out("$3") result)
    }
    result
}

fn test_dsra32<const SHIFT_AMOUNT: u32>(source_value: u64) -> u64 {
    let result: u64;
    unsafe {
        asm!("DSRA32 $3, $2, {SHIFT_AMOUNT}", SHIFT_AMOUNT = const SHIFT_AMOUNT, in("$2") source_value, out("$3") result)
    }
    result
}

fn test_dsrl32<const SHIFT_AMOUNT: u32>(source_value: u64) -> u64 {
    let result: u64;
    unsafe {
        asm!("DSRL32 $3, $2, {SHIFT_AMOUNT}", SHIFT_AMOUNT = const SHIFT_AMOUNT, in("$2") source_value, out("$3") result)
    }
    result
}

fn test_dsll32<const SHIFT_AMOUNT: u32>(source_value: u64) -> u64 {
    let result: u64;
    unsafe {
        asm!("DSLL32 $3, $2, {SHIFT_AMOUNT}", SHIFT_AMOUNT = const SHIFT_AMOUNT, in("$2") source_value, out("$3") result)
    }
    result
}

pub struct SRA {}

impl Test for SRA {
    fn name(&self) -> &str {
        "SRA"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((0x00000000_12345678u64, 4u32, 0x00000000_01234567u64)),
            Box::new((0x00000000_82345678u64, 0u32, 0xFFFFFFFF_82345678u64)),
            Box::new((0x01234567_89ABCDEFu64, 4u32, 0x00000000_789ABCDEu64)),
            Box::new((0x00000008_789ABCDEu64, 4u32, 0xFFFFFFFF_8789ABCDu64)),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u32, u64)>() {
            Some((source_value, shift_amount, expected_value)) => {
                let result = match shift_amount {
                    0 => test_sra::<0>(*source_value),
                    4 => test_sra::<4>(*source_value),
                    _ => panic!(),
                };
                soft_assert_eq(result, *expected_value, "Result")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct SRL {}

impl Test for SRL {
    fn name(&self) -> &str {
        "SRL"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((0x00000000_12345678u64, 4u32, 0x00000000_01234567u64)),
            Box::new((0x00000000_82345678u64, 0u32, 0xFFFFFFFF_82345678u64)),
            Box::new((0x01234567_89ABCDEFu64, 4u32, 0x00000000_089ABCDEu64)),
            Box::new((0x00000008_789ABCDEu64, 4u32, 0x00000000_0789ABCDu64)),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u32, u64)>() {
            Some((source_value, shift_amount, expected_value)) => {
                let result = match shift_amount {
                    0 => test_srl::<0>(*source_value),
                    4 => test_srl::<4>(*source_value),
                    _ => panic!(),
                };
                soft_assert_eq(result, *expected_value, "Result")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct SLL {}

impl Test for SLL {
    fn name(&self) -> &str {
        "SLL"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((0x00000000_12345678u64, 4u32, 0x00000000_23456780u64)),
            Box::new((0x00000000_82345678u64, 0u32, 0xFFFFFFFF_82345678u64)),
            Box::new((0x12345678_789ABCDEu64, 4u32, 0xFFFFFFFF_89ABCDE0u64)),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u32, u64)>() {
            Some((source_value, shift_amount, expected_value)) => {
                let result = match shift_amount {
                    0 => test_sll::<0>(*source_value),
                    4 => test_sll::<4>(*source_value),
                    _ => panic!(),
                };
                soft_assert_eq(result, *expected_value, "Result")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct DSRA32 {}

impl Test for DSRA32 {
    fn name(&self) -> &str {
        "DSRA32"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((0x12345678_12345678u64, 0u32, 0x00000000_12345678u64)),
            Box::new((0x82345678_12345678u64, 0u32, 0xFFFFFFFF_82345678u64)),
            Box::new((0x12345678_12345678u64, 4u32, 0x00000000_01234567u64)),
            Box::new((0x82345678_12345678u64, 4u32, 0xFFFFFFFF_F8234567u64)),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u32, u64)>() {
            Some((source_value, shift_amount, expected_value)) => {
                let result = match shift_amount {
                    0 => test_dsra32::<0>(*source_value),
                    4 => test_dsra32::<4>(*source_value),
                    _ => panic!(),
                };
                soft_assert_eq(result, *expected_value, "Result")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct DSRL32 {}

impl Test for DSRL32 {
    fn name(&self) -> &str {
        "DSRL32"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((0x12345678_12345678u64, 0u32, 0x00000000_12345678u64)),
            Box::new((0x82345678_12345678u64, 0u32, 0x00000000_82345678u64)),
            Box::new((0x12345678_12345678u64, 4u32, 0x00000000_01234567u64)),
            Box::new((0x82345678_12345678u64, 4u32, 0x00000000_08234567u64)),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u32, u64)>() {
            Some((source_value, shift_amount, expected_value)) => {
                let result = match shift_amount {
                    0 => test_dsrl32::<0>(*source_value),
                    4 => test_dsrl32::<4>(*source_value),
                    _ => panic!(),
                };
                soft_assert_eq(result, *expected_value, "Result")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct DSLL32 {}

impl Test for DSLL32 {
    fn name(&self) -> &str {
        "DSRL32"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((0x12345678_12345678u64, 0u32, 0x12345678_00000000u64)),
            Box::new((0x82345678_82345678u64, 0u32, 0x82345678_00000000u64)),
            Box::new((0x12345678_12345678u64, 4u32, 0x23456780_00000000u64)),
            Box::new((0x82345678_82345678u64, 4u32, 0x23456780_00000000u64)),
            Box::new((0x82345678_82345678u64, 31u32, 0x00000000_00000000u64)),
            Box::new((0x82345678_82345679u64, 31u32, 0x80000000_00000000u64)),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u32, u64)>() {
            Some((source_value, shift_amount, expected_value)) => {
                let result = match shift_amount {
                    0 => test_dsll32::<0>(*source_value),
                    4 => test_dsll32::<4>(*source_value),
                    31 => test_dsll32::<31>(*source_value),
                    _ => panic!(),
                };
                soft_assert_eq(result, *expected_value, "Result")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct ShiftsIntoR0 {}

impl Test for ShiftsIntoR0 {
    fn name(&self) -> &str {
        "ShiftsIntoR0"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![]
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let mut sll: u64 = 0xDECAF15BADC0FFEE;
        let mut srl: u64 = 0xDECAF15BADC0FFEE;
        let mut sra: u64 = 0xDECAF15BADC0FFEE;

        unsafe {
            asm!("
                LUI $2, 0x1234
                SLL $0, $2, 1
                DADDU {z0}, $0, $0
                SRL $0, $2, 1
                DADDU {z1}, $0, $0
                SRA $0, $2, 1
                DADDU {z2}, $0, $0
            ", out("$2") _, z0 = inout(reg) sll, z1 = inout(reg) srl, z2 = inout(reg) sra)
        }

        soft_assert_eq(sll, 0, "SLL into R0")?;
        soft_assert_eq(srl, 0, "SRL into R0")?;
        soft_assert_eq(sra, 0, "SRA into R0")?;

        Ok(())
    }
}
