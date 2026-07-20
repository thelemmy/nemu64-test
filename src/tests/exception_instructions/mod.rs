use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use crate::cop0::{
    preset_cause_to_copindex2, set_status, status, Cause, CauseException, RegisterIndex, Status,
};
use crate::exception_handler::expect_exception;
use crate::tests::soft_asserts::{soft_assert_eq, soft_assert_range};
use crate::tests::{Level, Test};

pub mod reserved;

pub struct Break {}

impl Test for Break {
    fn name(&self) -> &str {
        "Break"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        preset_cause_to_copindex2()?;

        let exception_context = expect_exception(CauseException::Bp, 1, || {
            unsafe {
                asm!(
                    "
                    .set noat
                    BREAK 0x319
                "
                )
            }

            Ok(())
        })?;

        soft_assert_eq(
            exception_context.k0_exception_vector,
            0xFFFFFFFF_80000180,
            "Exception Vector",
        )?;
        soft_assert_eq(
            exception_context.exceptpc & 0xFFFFFFFF_FF000000,
            0xFFFFFFFF_80000000,
            "ExceptPC",
        )?;
        soft_assert_eq(
            ((unsafe { *(exception_context.exceptpc as *const u32) }) >> 16) & 0x3FF,
            0x319,
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(exception_context.cause.raw_value(), 0x24, "Cause")?;
        soft_assert_eq(exception_context.status, 0x24000002, "Status")?;

        Ok(())
    }
}

pub struct BreakDelay {}

impl Test for BreakDelay {
    fn name(&self) -> &str {
        "Break (delay slot)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let exception_context = expect_exception(CauseException::Bp, 2, || {
            unsafe {
                asm!(
                    "
                    .set noat
                    .set noreorder
                    BEQ $0, $0, 2f
                    BREAK 0x319
                    2:
                    NOP
                    NOP
                "
                )
            }

            Ok(())
        })?;

        soft_assert_eq(
            exception_context.k0_exception_vector,
            0xFFFFFFFF_80000180,
            "Exception Vector",
        )?;
        soft_assert_eq(
            exception_context.exceptpc & 0xFFFFFFFF_FF000000,
            0xFFFFFFFF_80000000,
            "ExceptPC",
        )?;
        soft_assert_eq(
            ((unsafe { *(exception_context.exceptpc as *const u32).add(1) }) >> 16) & 0x3FF,
            0x319,
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(exception_context.cause.raw_value(), 0x80000024, "Cause")?;
        soft_assert_eq(exception_context.status, 0x24000002, "Status")?;

        Ok(())
    }
}

pub struct Syscall {}

impl Test for Syscall {
    fn name(&self) -> &str {
        "Syscall"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        preset_cause_to_copindex2()?;

        let exception_context = expect_exception(CauseException::Sys, 1, || {
            unsafe {
                asm!(
                    "
                    .set noat
                    SYSCALL 0xF123F
                "
                )
            }

            Ok(())
        })?;

        soft_assert_eq(
            exception_context.k0_exception_vector,
            0xFFFFFFFF_80000180,
            "Exception Vector",
        )?;
        soft_assert_eq(
            exception_context.exceptpc & 0xFFFFFFFF_FF000000,
            0xFFFFFFFF_80000000,
            "ExceptPC",
        )?;
        soft_assert_eq(
            ((unsafe { *(exception_context.exceptpc as *const u32) }) >> 6) & 0xFFFFF,
            0xF123F,
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(exception_context.cause.raw_value(), 0x20, "Cause")?;
        soft_assert_eq(exception_context.status, 0x24000002, "Status")?;

        Ok(())
    }
}

pub struct SyscallDelay {}

impl Test for SyscallDelay {
    fn name(&self) -> &str {
        "Syscall (delay slot)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let exception_context = expect_exception(CauseException::Sys, 2, || {
            unsafe {
                asm!(
                    "
                    .set noat
                    .set noreorder
                    BNE $0, $0, 2f
                    SYSCALL 0xF123F
                    2:
                    NOP
                    NOP
                "
                )
            }

            Ok(())
        })?;

        soft_assert_eq(
            exception_context.k0_exception_vector,
            0xFFFFFFFF_80000180,
            "Exception Vector",
        )?;
        soft_assert_eq(
            exception_context.exceptpc & 0xFFFFFFFF_FF000000,
            0xFFFFFFFF_80000000,
            "ExceptPC",
        )?;
        soft_assert_eq(
            ((unsafe { *(exception_context.exceptpc as *const u32).add(1) }) >> 6) & 0xFFFFF,
            0xF123F,
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(exception_context.cause.raw_value(), 0x80000020, "Cause")?;
        soft_assert_eq(exception_context.status, 0x24000002, "Status")?;

        Ok(())
    }
}

/// Instruction 31 doesn't exist. If it is called (Linux calls it for example), we expect a Reserved-Instruction exception
pub struct Reserved31 {}

impl Test for Reserved31 {
    fn name(&self) -> &str {
        "Reserved (31)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        preset_cause_to_copindex2()?;

        let exception_context = expect_exception(CauseException::RI, 1, || {
            unsafe {
                asm!(
                    "
                    .set noat
                    .word 0x7C03E83B
                "
                )
            }

            Ok(())
        })?;

        soft_assert_eq(
            exception_context.k0_exception_vector,
            0xFFFFFFFF_80000180,
            "Exception Vector",
        )?;
        soft_assert_eq(
            exception_context.exceptpc & 0xFFFFFFFF_FF000000,
            0xFFFFFFFF_80000000,
            "ExceptPC",
        )?;
        soft_assert_eq(
            unsafe { *(exception_context.exceptpc as *const u32) },
            0x7C03E83B,
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(exception_context.cause.raw_value(), 0x28, "Cause")?;
        soft_assert_eq(exception_context.status, 0x24000002, "Status")?;

        Ok(())
    }
}

pub struct Reserved31Delay {}

impl Test for Reserved31Delay {
    fn name(&self) -> &str {
        "Reserved (31) (delay slot)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let exception_context = expect_exception(CauseException::RI, 2, || {
            unsafe {
                asm!(
                    "
                    .set noat
                    .set noreorder
                    BNE $0, $0, 2f
                    .word 0x7C03E83B
                    2:
                    NOP
                    NOP
                "
                )
            }

            Ok(())
        })?;

        soft_assert_eq(
            exception_context.k0_exception_vector,
            0xFFFFFFFF_80000180,
            "Exception Vector",
        )?;
        soft_assert_eq(
            exception_context.exceptpc & 0xFFFFFFFF_FF000000,
            0xFFFFFFFF_80000000,
            "ExceptPC",
        )?;
        soft_assert_eq(
            unsafe { *(exception_context.exceptpc as *const u32).add(1) },
            0x7C03E83B,
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(exception_context.cause.raw_value(), 0x80000028, "Cause")?;
        soft_assert_eq(exception_context.status, 0x24000002, "Status")?;

        Ok(())
    }
}

fn test_sw_interrupt(
    status_before: Status,
    cause: Cause,
    status_after: Status,
    fire_position: u32,
    hazard: bool,
) -> Result<(), String> {
    preset_cause_to_copindex2()?;

    let previous_status = status();

    let mut addr0: isize = 0;
    let mut addr1: isize = 0;
    let exception_context = expect_exception(CauseException::Int, 1, || {
        unsafe {
            asm!("
                .set noat
                mtc0 {status_before_value}, ${Status}
                dla {addr0}, 0f
                dla {addr1}, 1f
                mtc0 {cause_value}, ${Cause}
                nop
0:
                mtc0 {status_after_value}, ${Status}
                nop
1:
                nop
            ",
            Cause = const RegisterIndex::Cause as u32,
            Status = const RegisterIndex::Status as u32,
            cause_value = in(reg) cause.raw_value(),
            status_before_value = in(reg) status_before.raw_value(),
            status_after_value = in(reg) status_after.raw_value(),
            addr0 = out(reg) addr0,
            addr1 = out(reg) addr1)
        }

        Ok(())
    })?;

    unsafe {
        set_status(previous_status);
    }

    soft_assert_eq(
        exception_context.k0_exception_vector,
        0xFFFFFFFF_80000180,
        "Exception Vector",
    )?;
    let expected_address = match fire_position {
        0 => addr0,
        1 => addr1,
        _ => panic!("Unexpected fire_position"),
    } as u64;
    soft_assert_eq(exception_context.cause, cause, "Cause")?;
    soft_assert_eq(
        exception_context.status,
        0x2400_0003 | (cause.raw_value() & 0x300),
        "Status",
    )?;

    if hazard {
        soft_assert_eq(exception_context.exceptpc, expected_address, "ExceptPC")?;
    } else {
        // Unless we're doing a COP0 hazard test, we can also allow the PC to be slightly earlier
        soft_assert_range(
            exception_context.exceptpc,
            expected_address - 4,
            expected_address,
            &"ExceptPC",
        )?;
    }

    Ok(())
}

pub struct SoftwareInterrupt1Enabled {}

impl Test for SoftwareInterrupt1Enabled {
    fn name(&self) -> &str {
        "SoftwareInterrupt1 (enabled)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous_status = status();
        test_sw_interrupt(
            previous_status.with_interrupt_mask_sw1(true).with_ie(true),
            Cause::DEFAULT.with_interrupt_sw1(true),
            previous_status,
            0,
            false,
        )
    }
}

pub struct SoftwareInterrupt1Masked {}

impl Test for SoftwareInterrupt1Masked {
    fn name(&self) -> &str {
        "SoftwareInterrupt1 (masked)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous_status = status();
        test_sw_interrupt(
            previous_status.with_interrupt_mask_sw1(false).with_ie(true),
            Cause::DEFAULT.with_interrupt_sw1(true),
            previous_status.with_interrupt_mask_sw1(true).with_ie(true),
            1,
            false,
        )
    }
}

pub struct SoftwareInterrupt1InterruptsDisabled {}

impl Test for SoftwareInterrupt1InterruptsDisabled {
    fn name(&self) -> &str {
        "SoftwareInterrupt1 (interrupts disabled)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous_status = status();
        test_sw_interrupt(
            previous_status.with_interrupt_mask_sw1(true).with_ie(false),
            Cause::DEFAULT.with_interrupt_sw1(true),
            previous_status.with_interrupt_mask_sw1(true).with_ie(true),
            1,
            false,
        )
    }
}

pub struct SoftwareInterrupt2Enabled {}

impl Test for SoftwareInterrupt2Enabled {
    fn name(&self) -> &str {
        "SoftwareInterrupt2 (enabled)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous_status = status();
        test_sw_interrupt(
            previous_status.with_interrupt_mask_sw2(true).with_ie(true),
            Cause::DEFAULT.with_interrupt_sw2(true),
            previous_status,
            0,
            false,
        )
    }
}

pub struct SoftwareInterrupt2Masked {}

impl Test for SoftwareInterrupt2Masked {
    fn name(&self) -> &str {
        "SoftwareInterrupt2 (masked)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous_status = status();
        test_sw_interrupt(
            previous_status.with_interrupt_mask_sw2(false).with_ie(true),
            Cause::DEFAULT.with_interrupt_sw2(true),
            previous_status.with_interrupt_mask_sw2(true).with_ie(true),
            1,
            false,
        )
    }
}

pub struct SoftwareInterrupt2InterruptsDisabled {}

impl Test for SoftwareInterrupt2InterruptsDisabled {
    fn name(&self) -> &str {
        "SoftwareInterrupt2 (interrupts disabled)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous_status = status();
        test_sw_interrupt(
            previous_status.with_interrupt_mask_sw2(true).with_ie(false),
            Cause::DEFAULT.with_interrupt_sw2(true),
            previous_status.with_interrupt_mask_sw2(true).with_ie(true),
            1,
            false,
        )
    }
}

pub struct SoftwareInterrupt12Enabled {}

impl Test for SoftwareInterrupt12Enabled {
    fn name(&self) -> &str {
        "SoftwareInterrupt12 (enabled)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous_status = status();
        test_sw_interrupt(
            previous_status
                .with_interrupt_mask_sw1(true)
                .with_interrupt_mask_sw2(true)
                .with_ie(true),
            Cause::DEFAULT
                .with_interrupt_sw1(true)
                .with_interrupt_sw2(true),
            previous_status,
            0,
            false,
        )
    }
}

pub struct SoftwareInterrupt1EnabledHazard {}

impl Test for SoftwareInterrupt1EnabledHazard {
    fn name(&self) -> &str {
        "SoftwareInterrupt1 (enabled, hazard)"
    }

    fn level(&self) -> Level {
        Level::COP0Hazard
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous_status = status();
        test_sw_interrupt(
            previous_status.with_interrupt_mask_sw1(true).with_ie(true),
            Cause::DEFAULT.with_interrupt_sw1(true),
            previous_status,
            0,
            true,
        )
    }
}

pub struct SoftwareInterrupt1EnableAndDisableInstantly {}

impl Test for SoftwareInterrupt1EnableAndDisableInstantly {
    fn name(&self) -> &str {
        "SoftwareInterrupt1 (enable but disable right away)"
    }

    fn level(&self) -> Level {
        Level::COP0Hazard
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        preset_cause_to_copindex2()?;

        let previous_status = status();

        // Fire exception but clear it before it had a chance to fire. This will not raise an exception
        unsafe {
            asm!("
                .set noat
                mtc0 {status}, ${Status}
                mtc0 {fire}, ${Cause}
                mtc0 {dontfire}, ${Cause}
                nop
            ",
            Cause = const RegisterIndex::Cause as u32,
            Status = const RegisterIndex::Status as u32,
            fire = in(reg) Cause::DEFAULT.with_interrupt_sw1(true).raw_value(),
            dontfire = in(reg) Cause::DEFAULT.raw_value(),
            status = in(reg) previous_status.with_interrupt_mask_sw1(true).with_ie(true).raw_value())
        }

        Ok(())
    }
}

pub struct SoftwareInterrupt1EnableAndDisableAfterOneNop {}

impl Test for SoftwareInterrupt1EnableAndDisableAfterOneNop {
    fn name(&self) -> &str {
        "SoftwareInterrupt12 (enable and disable after one nop)"
    }

    fn level(&self) -> Level {
        Level::COP0Hazard
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        preset_cause_to_copindex2()?;

        let previous_status = status();

        // Fire exception but clear it before it had a chance to fire

        let mut addr0: isize = 0;
        let exception_context = expect_exception(CauseException::Int, 1, || {
            unsafe {
                asm!("
                    .set noat
                    mtc0 {status}, ${Status}
                    dla {addr0}, 0f
                    mtc0 {fire}, ${Cause}
                    nop
0:
                    mtc0 {dontfire}, ${Cause}
                    nop
                ",
                Cause = const RegisterIndex::Cause as u32,
                Status = const RegisterIndex::Status as u32,
                fire = in(reg) Cause::DEFAULT.with_interrupt_sw1(true).raw_value(),
                dontfire = in(reg) Cause::DEFAULT.raw_value(),
                status = in(reg) previous_status.with_interrupt_mask_sw1(true).with_ie(true).raw_value(),
                addr0 = out(reg) addr0)
            }

            Ok(())
        })?;

        unsafe {
            set_status(previous_status);
        }

        soft_assert_eq(
            exception_context.k0_exception_vector,
            0xFFFFFFFF_80000180,
            "Exception Vector",
        )?;
        soft_assert_eq(
            exception_context.cause,
            Cause::DEFAULT.with_interrupt_sw1(true),
            "Cause",
        )?;
        soft_assert_eq(exception_context.status, 0x2400_0103, "Status")?;
        soft_assert_eq(
            exception_context.exceptpc,
            addr0 as u64,
            "ExceptPC points to wrong instruction",
        )?;

        Ok(())
    }
}
