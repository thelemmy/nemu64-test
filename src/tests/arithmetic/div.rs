use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use crate::tests::soft_asserts::soft_assert_eq;
use crate::tests::{Level, Test};

fn test_div(dividend: u64, divisor: u64) -> (u64, u64) {
    let quotient: u64;
    let remainder: u64;
    unsafe {
        asm!("
            .set noat
            .set noreorder

            MFLO {save_lo}
            MFHI {save_hi}
            DIV $0, $2, $3
            MFLO $2
            MFHI $3
            MTLO {save_lo}
            MTHI {save_hi}

        ", inout("$2") dividend => quotient, inout("$3") divisor => remainder, save_lo = out(reg) _, save_hi = out(reg) _)
    }
    (quotient, remainder)
}

fn test_divu(dividend: u64, divisor: u64) -> (u64, u64) {
    let quotient: u64;
    let remainder: u64;
    unsafe {
        asm!("
            .set noat
            .set noreorder

            MFLO {save_lo}
            MFHI {save_hi}
            DIVU $0, $2, $3
            MFLO $2
            MFHI $3
            MTLO {save_lo}
            MTHI {save_hi}

        ", inout("$2") dividend => quotient, inout("$3") divisor => remainder, save_lo = out(reg) _, save_hi = out(reg) _)
    }
    (quotient, remainder)
}

fn test_ddiv(dividend: u64, divisor: u64) -> (u64, u64) {
    let quotient: u64;
    let remainder: u64;
    unsafe {
        asm!("
            .set noat
            .set noreorder

            MFLO {save_lo}
            MFHI {save_hi}
            DDIV $0, $2, $3
            MFLO $2
            MFHI $3
            MTLO {save_lo}
            MTHI {save_hi}

        ", inout("$2") dividend => quotient, inout("$3") divisor => remainder, save_lo = out(reg) _, save_hi = out(reg) _)
    }
    (quotient, remainder)
}

fn test_ddivu(dividend: u64, divisor: u64) -> (u64, u64) {
    let quotient: u64;
    let remainder: u64;
    unsafe {
        asm!("
            .set noat
            .set noreorder

            MFLO {save_lo}
            MFHI {save_hi}
            DDIVU $0, $2, $3
            MFLO $2
            MFHI $3
            MTLO {save_lo}
            MTHI {save_hi}

        ", inout("$2") dividend => quotient, inout("$3") divisor => remainder, save_lo = out(reg) _, save_hi = out(reg) _)
    }
    (quotient, remainder)
}

pub struct DIV {}

impl Test for DIV {
    fn name(&self) -> &str {
        "DIV"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((
                0x00000000_01234567u64,
                0u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0x00000000_01234567u64,
            )),
            Box::new((0u64, 0u64, 0xFFFFFFFF_FFFFFFFFu64, 0u64)),
            Box::new((
                0xFFFFFFFF_F1234567u64,
                0u64,
                0x00000000_00000001u64,
                0xFFFFFFFF_F1234567u64,
            )),
            Box::new((
                0xFFFFFFFF_80000000u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0xFFFFFFFF_80000000u64,
                0u64,
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u64, u64, u64)>() {
            Some((dividend, divisor, expected_quotient, expected_remainder)) => {
                let (quotient, remainder) = test_div(*dividend, *divisor);
                soft_assert_eq(quotient, *expected_quotient, "Quotient")?;
                soft_assert_eq(remainder, *expected_remainder, "Remainder")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct DIVU {}

impl Test for DIVU {
    fn name(&self) -> &str {
        "DIVU"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((
                0x00000000_01234567u64,
                0u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0x00000000_01234567u64,
            )),
            Box::new((0u64, 0u64, 0xFFFFFFFF_FFFFFFFFu64, 0u64)),
            Box::new((
                0xFFFFFFFF_F1234567u64,
                0u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0xFFFFFFFF_F1234567u64,
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u64, u64, u64)>() {
            Some((dividend, divisor, expected_quotient, expected_remainder)) => {
                let (quotient, remainder) = test_divu(*dividend, *divisor);
                soft_assert_eq(quotient, *expected_quotient, "Quotient")?;
                soft_assert_eq(remainder, *expected_remainder, "Remainder")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct DDIV {}

impl Test for DDIV {
    fn name(&self) -> &str {
        "DDIV"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((
                0x01234567_89ABCDEFu64,
                0u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0x01234567_89ABCDEFu64,
            )),
            Box::new((0u64, 0u64, 0xFFFFFFFF_FFFFFFFFu64, 0u64)),
            Box::new((
                0xF1234567_89ABCDEFu64,
                0u64,
                0x00000000_00000001u64,
                0xF1234567_89ABCDEFu64,
            )),
            Box::new((
                0x80000000_00000000u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0x80000000_00000000u64,
                0u64,
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u64, u64, u64)>() {
            Some((dividend, divisor, expected_quotient, expected_remainder)) => {
                let (quotient, remainder) = test_ddiv(*dividend, *divisor);
                soft_assert_eq(quotient, *expected_quotient, "Quotient")?;
                soft_assert_eq(remainder, *expected_remainder, "Remainder")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}

pub struct DDIVU {}

impl Test for DDIVU {
    fn name(&self) -> &str {
        "DDIVU"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((
                0x01234567_89ABCDEFu64,
                0u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0x01234567_89ABCDEFu64,
            )),
            Box::new((0u64, 0u64, 0xFFFFFFFF_FFFFFFFFu64, 0u64)),
            Box::new((
                0xF1234567_89ABCDEFu64,
                0u64,
                0xFFFFFFFF_FFFFFFFFu64,
                0xF1234567_89ABCDEFu64,
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(u64, u64, u64, u64)>() {
            Some((dividend, divisor, expected_quotient, expected_remainder)) => {
                let (quotient, remainder) = test_ddivu(*dividend, *divisor);
                soft_assert_eq(quotient, *expected_quotient, "Quotient")?;
                soft_assert_eq(remainder, *expected_remainder, "Remainder")?;
                Ok(())
            }
            _ => Err("Value is not valid".to_string()),
        }
    }
}
