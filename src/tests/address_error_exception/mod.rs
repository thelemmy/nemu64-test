use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use arbitrary_int::prelude::*;

use crate::assembler::{Assembler, FR, GPR};
use crate::cop0;
use crate::cop0::{cache, preset_cause_to_copindex2, CacheOp, CauseException, Context, XContext};
use crate::exception_handler::{expect_exception, observe_exception};
use crate::tests::soft_asserts::soft_assert_eq;
use crate::tests::{Level, Test};

// TODO: Less copy&paste below to more easily test more cases
// TODO: Test 16 bit
// TODO: Test 64 bit
// TODO: Test unsigned
// TODO: Test delay slot (in that case make sure to skip 2 instructions in the exception handler)
// TODO: Test nested exceptions (address_error_exception access while exception is already happening)

pub struct UnalignedLW {}

impl Test for UnalignedLW {
    fn name(&self) -> &str {
        "Unaligned LW exception"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        preset_cause_to_copindex2()?;

        let a = 0x12345678u32;
        // Make unaligned pointer
        let p = &a as *const u32 as isize + 2;

        unsafe {
            cop0::set_context_64(0);
        }
        unsafe {
            cop0::set_xcontext_64(0);
        }

        let mut fault_address: isize = 0;
        let exception_context = expect_exception(CauseException::AdEL, 1, || {
            unsafe {
                asm!("
                    .set noat
                    DLA {fault_address}, 1f
1:
                    LW $0, 0($3)
                ", in("$3") p, fault_address = out(reg) fault_address)
            }

            Ok(())
        })?;

        soft_assert_eq(
            exception_context.k0_exception_vector,
            0xFFFFFFFF_80000180,
            "Exception Vector",
        )?;
        soft_assert_eq(exception_context.exceptpc, fault_address as u64, "ExceptPC")?;
        soft_assert_eq(
            exception_context.badvaddr,
            p as u64,
            "BadVAddr during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.cause.raw_value(),
            0x10,
            "Cause during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.status,
            0x24000002,
            "Status during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.context,
            Context::from_virtual_address(p as u64),
            "Context during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.xcontext,
            XContext::from_virtual_address(p as u64),
            "XContext during AdEL exception",
        )?;

        Ok(())
    }
}

pub struct UnalignedLW2 {}

impl Test for UnalignedLW2 {
    fn name(&self) -> &str {
        "Unaligned LW exception (with high bits of context set)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let a = 0x12345678u32;
        // Make unaligned pointer
        let p = &a as *const u32 as isize + 3;

        unsafe {
            cop0::set_context_64(0xFFFFFFFF_FFFFFFFF);
        }
        unsafe {
            cop0::set_xcontext_64(0xFFFFFFFF_FFFFFFFF);
        }
        let exception_context = expect_exception(CauseException::AdEL, 1, || {
            unsafe {
                asm!("
                    .set noat
                    LW $0, 0($2)
                ", in("$2") p)
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
            0x8C400000,
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(
            exception_context.badvaddr,
            p as isize as u64,
            "BadVAddr during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.cause.raw_value(),
            0x10,
            "Cause during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.status,
            0x24000002,
            "Status during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.context,
            Context::from_virtual_address(p as u64).with_pte_base(u41::MAX),
            "Context during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.xcontext,
            XContext::from_virtual_address(p as u64).with_pte_base(u31::MAX),
            "XContext during AdEL exception",
        )?;

        Ok(())
    }
}

pub struct UnalignedLWC1 {}

impl Test for UnalignedLWC1 {
    fn name(&self) -> &str {
        "Unaligned LWC1 exception (with high bits of context set)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let a = 0x12345678u32;
        // Make unaligned pointer
        let p = &a as *const u32 as isize + 3;

        unsafe {
            cop0::set_context_64(0xFFFFFFFF_FFFFFFFF);
        }
        unsafe {
            cop0::set_xcontext_64(0xFFFFFFFF_FFFFFFFF);
        }
        let exception_context = expect_exception(CauseException::AdEL, 1, || {
            unsafe {
                asm!("
                    .set noat
                    LWC1 $0, 0($2)
                ", in("$2") p)
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
            Assembler::make_lwc1(FR::F0, 0, GPR::V0),
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(
            exception_context.badvaddr,
            p as isize as u64,
            "BadVAddr during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.cause.raw_value(),
            0x10,
            "Cause during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.status,
            0x24000002,
            "Status during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.context,
            Context::from_virtual_address(p as u64).with_pte_base(u41::MAX),
            "Context during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.xcontext,
            XContext::from_virtual_address(p as u64).with_pte_base(u31::MAX),
            "XContext during AdEL exception",
        )?;

        Ok(())
    }
}

pub struct UnalignedLWDelay {}

impl Test for UnalignedLWDelay {
    fn name(&self) -> &str {
        "Unaligned LW exception (delay slot)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let a = 0x12345678u32;
        // Make unaligned pointer
        let p = &a as *const u32 as isize + 1;

        unsafe {
            cop0::set_context_64(0);
        }
        unsafe {
            cop0::set_xcontext_64(0);
        }
        let exception_context = expect_exception(CauseException::AdEL, 2, || {
            unsafe {
                asm!("
                    .set noat
                    .set noreorder
                    BNE $1, $1, 2f
                    LW $0, 0($2)
                    2:
                    NOP
                    NOP
                ", in("$2") p)
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
            0x8C400000,
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(
            exception_context.badvaddr,
            p as isize as u64,
            "BadVAddr during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.cause.raw_value(),
            0x80000010,
            "Cause during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.status,
            0x24000002,
            "Status during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.context,
            Context::from_virtual_address(p as u64),
            "Context during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.xcontext,
            XContext::from_virtual_address(p as u64),
            "XContext during AdEL exception",
        )?;

        Ok(())
    }
}

pub struct UnalignedSW {}

impl Test for UnalignedSW {
    fn name(&self) -> &str {
        "Unaligned SW exception"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let a = 0x12345678u32;
        // Make unaligned pointer
        let p = &a as *const u32 as isize + 2;

        unsafe {
            cop0::set_context_64(0);
        }
        unsafe {
            cop0::set_xcontext_64(0);
        }
        let exception_context = expect_exception(CauseException::AdES, 1, || {
            unsafe {
                asm!("
                    .set noat
                    SW $0, 0($2)
                ", in("$2") p)
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
            0xAC400000,
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(
            exception_context.badvaddr,
            p as isize as u64,
            "BadVAddr during AdES exception",
        )?;
        soft_assert_eq(
            exception_context.cause.raw_value(),
            0x14,
            "Cause during AdES exception",
        )?;
        soft_assert_eq(
            exception_context.status,
            0x24000002,
            "Status during AdES exception",
        )?;
        soft_assert_eq(
            exception_context.context,
            Context::from_virtual_address(p as u64),
            "Context during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.xcontext,
            XContext::from_virtual_address(p as u64),
            "XContext during AdEL exception",
        )?;

        Ok(())
    }
}

fn test_unaligned_cache<const OP: u8>() -> Result<(), String> {
    preset_cause_to_copindex2()?;

    let a = 0x12345678_89abcdefu64;

    unsafe {
        cop0::set_context_64(0);
    }
    unsafe {
        cop0::set_xcontext_64(0);
    }

    // Aligned to 8 byte is ok
    unsafe {
        cache::<OP, 0>(&a as *const u64 as usize);
    }

    // Aligned to 4 byte is also ok
    unsafe {
        cache::<OP, 0>(&a as *const u64 as usize + 4);
    }

    // Aligned to 2 bytes raises an exception
    let unaligned = &a as *const u64 as isize + 2;
    let exception_context = expect_exception(CauseException::AdEL, 1, || {
        unsafe { asm!("cache {OP}, 0 ($2)", OP = const OP, in("$2") unaligned, options(nostack)) }

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
        Assembler::make_cache(
            CacheOp::new_with_raw_value(u5::new(OP)).unwrap(),
            0,
            GPR::V0,
        ),
        "ExceptPC points to wrong instruction",
    )?;
    soft_assert_eq(
        exception_context.badvaddr,
        unaligned as i32 as u64,
        "BadVAddr during AdEL exception",
    )?;
    soft_assert_eq(
        exception_context.cause.raw_value(),
        0x10,
        "Cause during AdEL exception",
    )?;
    soft_assert_eq(
        exception_context.status,
        0x24000002,
        "Status during AdEL exception",
    )?;
    soft_assert_eq(
        exception_context.context,
        Context::from_virtual_address(unaligned as u64),
        "Context during AdEL exception",
    )?;
    soft_assert_eq(
        exception_context.xcontext,
        XContext::from_virtual_address(unaligned as u64),
        "XContext during AdEL exception",
    )?;

    Ok(())
}

pub struct UnalignedCacheData {}

impl Test for UnalignedCacheData {
    fn name(&self) -> &str {
        "Unaligned CACHE (data) address"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const OP: u8 = CacheOp::DataIndexWriteBackInvalidate.raw_value().value();
        test_unaligned_cache::<OP>()
    }
}

pub struct UnalignedCacheInstruction {}

impl Test for UnalignedCacheInstruction {
    fn name(&self) -> &str {
        "Unaligned CACHE (instruction) address"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const OP: u8 = CacheOp::InstructionIndexInvalidate.raw_value().value();
        test_unaligned_cache::<OP>()
    }
}

pub struct UnalignedJump {}

impl Test for UnalignedJump {
    fn name(&self) -> &str {
        "Unaligned jump exception"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let mut addr = 0u32;
        unsafe {
            cop0::set_context_64(0);
        }
        unsafe {
            cop0::set_xcontext_64(0);
        }
        let exception_context = expect_exception(CauseException::AdEL, 0, || {
            unsafe {
                asm!("
                    .set noat
                    DLA $2, 1f
                    ADDIU $2, 2
                    JR $2
                1:
                    ADDIU $0, $0, 0x1234
                    NOP
                ", out("$2") addr);
            }

            Ok(())
        })?;

        let addr64 = addr as i32 as u64;

        soft_assert_eq(
            exception_context.k0_exception_vector,
            0xFFFFFFFF_80000180,
            "Exception Vector",
        )?;
        soft_assert_eq(exception_context.exceptpc, addr64, "ExceptPC")?;
        soft_assert_eq(
            exception_context.badvaddr,
            addr64,
            "BadVAddr during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.cause.raw_value(),
            0x10,
            "Cause during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.status,
            0x24000002,
            "Status during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.context,
            Context::from_virtual_address(addr64),
            "Context during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.xcontext,
            XContext::from_virtual_address(addr64),
            "XContext during AdEL exception",
        )?;

        Ok(())
    }
}

pub struct UnalignedJumpWithDelaySlot {}

impl Test for UnalignedJumpWithDelaySlot {
    fn name(&self) -> &str {
        "Unaligned jump exception with delay slot"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let mut addr = 0u32;
        let mut val = 0u32;
        unsafe {
            cop0::set_context_64(0);
        }
        unsafe {
            cop0::set_xcontext_64(0);
        }
        let exception_context = expect_exception(CauseException::AdEL, 0, || {
            unsafe {
                asm!("
                    .set noat
                    .set noreorder
                    ORI $3, $0, 0x01
                    DLA $2, 1f
                    ADDIU $2, 1
                    JR $2
                    ORI $3, $3, 0x02
                1:
                    ORI $3, $3, 0x04
                ", out("$2") addr,
                   out("$3") val);
            }

            Ok(())
        })?;

        let addr64 = addr as i32 as u64;

        soft_assert_eq(
            exception_context.k0_exception_vector,
            0xFFFFFFFF_80000180,
            "Exception Vector",
        )?;
        soft_assert_eq(exception_context.exceptpc, addr64, "ExceptPC")?;
        soft_assert_eq(
            exception_context.badvaddr,
            addr64,
            "BadVAddr during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.cause.raw_value(),
            0x10,
            "Cause during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.status,
            0x24000002,
            "Status during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.context,
            Context::from_virtual_address(addr64),
            "Context during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.xcontext,
            XContext::from_virtual_address(addr64),
            "XContext during AdEL exception",
        )?;
        soft_assert_eq(val, 7, "delay slot execution during exception")?;

        Ok(())
    }
}

pub struct LWAddressNotSignExtended {}

impl Test for LWAddressNotSignExtended {
    fn name(&self) -> &str {
        "LW with address not sign extended"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let a = 0x12345678u32;
        let p = &a as *const u32 as u32;
        // Load from 0x00000000_80xxxxxx causes AdEL, as upper bits are 0
        unsafe {
            cop0::set_context_64(0);
        }
        unsafe {
            cop0::set_xcontext_64(0);
        }
        let exception_context = expect_exception(CauseException::AdEL, 1, || {
            unsafe {
                asm!("
                    .set noat
                    // zero out upper bits
                    DSLL32 $2, $2, 0
                    DSRL32 $2, $2, 0
                    LW $0, 0($2)
                ", inout("$2") p => _)
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
            0x8C400000,
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(
            exception_context.badvaddr,
            p as u64 & 0xFFFFFFFF,
            "BadVAddr",
        )?;
        soft_assert_eq(exception_context.cause.raw_value(), 0x10, "Cause")?;
        soft_assert_eq(exception_context.status, 0x24000002, "Status")?;
        soft_assert_eq(
            exception_context.context,
            Context::from_virtual_address(p as u64),
            "Context during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.xcontext,
            XContext::from_virtual_address(p as u64),
            "XContext during AdEL exception",
        )?;

        Ok(())
    }
}

pub struct SWAddressNotSignExtended {}

impl Test for SWAddressNotSignExtended {
    fn name(&self) -> &str {
        "SW with address not sign extended"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // Store into 0x00000000_80xxxxxx causes AdES, as upper bits are 0
        let a = 0x12345678u32;
        let p = &a as *const u32 as u32;
        unsafe {
            cop0::set_context_64(0);
        }
        unsafe {
            cop0::set_xcontext_64(0);
        }
        let exception_context = expect_exception(CauseException::AdES, 1, || {
            unsafe {
                asm!("
                    .set noat
                    // zero out upper bits
                    DSLL32 $2, $2, 0
                    DSRL32 $2, $2, 0
                    SW $0, 0($2)
                ", inout("$2") p => _)
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
            0xAC400000,
            "ExceptPC points to wrong instruction",
        )?;
        soft_assert_eq(
            exception_context.badvaddr,
            p as u64 & 0xFFFFFFFF,
            "BadVAddr",
        )?;
        soft_assert_eq(exception_context.cause.raw_value(), 0x14, "Cause")?;
        soft_assert_eq(exception_context.status, 0x24000002, "Status")?;
        soft_assert_eq(
            exception_context.context,
            Context::from_virtual_address(p as u64),
            "Context during AdEL exception",
        )?;
        soft_assert_eq(
            exception_context.xcontext,
            XContext::from_virtual_address(p as u64),
            "XContext during AdEL exception",
        )?;

        Ok(())
    }
}

/// Low 32 bits of the probed address. The probes combine each of these with many patterns for the
/// upper 32 bits.
const FAMILIES: [&str; 6] = [
    "kuseg 0x00001000",
    "kseg0 (own buffer)",
    "kseg1 (own buffer)",
    "kseg2 0xC0001000",
    "kuseg misaligned 0x00001001",
    "kseg0 misaligned (own buffer + 1)",
];

fn low_word(family: u32, buffer: u32) -> u32 {
    match family {
        0 => 0x0000_1000,
        1 => buffer,
        2 => (buffer & 0x1FFF_FFFF) | 0xA000_0000,
        3 => 0xC000_1000,
        4 => 0x0000_1001,
        _ => buffer + 1,
    }
}

fn upper_patterns() -> Vec<u32> {
    // The two canonical values, two alternating patterns, every single bit set, and all ones with
    // a single bit cleared. The last two sets show which upper bits matter.
    let mut patterns = vec![0x0000_0000, 0xFFFF_FFFF, 0x5555_5555, 0xAAAA_AAAA];
    for bit in 0..32 {
        patterns.push(1u32 << bit);
    }
    for bit in 0..32 {
        patterns.push(!(1u32 << bit));
    }
    patterns
}

/// What we are sure about: Some(outcome) with outcome None = no exception, Some(code) = ExcCode.
/// Outer None = not known, the probe then just reports what it saw.
fn predicted(store: bool, family: u32, upper: u32) -> Option<Option<u32>> {
    let (tlb, address_error) = if store { (3, 5) } else { (2, 4) };
    match (family, upper) {
        // Canonical (sign extended) addresses
        (0, 0x0000_0000) => Some(Some(tlb)), // kuseg is mapped by the TLB, which is empty
        (1, 0xFFFF_FFFF) | (2, 0xFFFF_FFFF) => Some(None), // kseg0 / kseg1: valid
        (3, 0xFFFF_FFFF) => Some(Some(tlb)), // kseg2 is mapped
        (5, 0xFFFF_FFFF) => Some(Some(address_error)), // misaligned
        // Hardware results we already have
        (1, 0x0000_0000) => Some(Some(address_error)), // zero extended kseg0
        (0, 0x0000_0001) => Some(Some(tlb)),           // bit 32 set, bit 31 clear
        (0, _) => Some(Some(tlb)),
        (1, _) => Some(Some(address_error)),
        (2, _) => Some(Some(address_error)),
        (3, _) => Some(Some(address_error)),
        (4, _) => Some(Some(address_error)),
        (5, _) => Some(Some(address_error)),
        _ => None,
    }
}

struct Observed {
    upper: u32,
    code: Option<u32>,
    badvaddr: u64,
    context: u64,
    xcontext: u64,
}

fn code_name(code: Option<u32>) -> String {
    match code {
        None => String::from("no exception"),
        Some(2) => String::from("TLBL"),
        Some(3) => String::from("TLBS"),
        Some(4) => String::from("AdEL"),
        Some(5) => String::from("AdES"),
        Some(other) => format!("ExcCode {}", other),
    }
}

fn observe_access(store: bool, address: u64) -> Result<Observed, String> {
    // Reset what a TLB exception writes, so we can see what each exception really updated
    unsafe {
        cop0::set_context_64(0);
        cop0::set_xcontext_64(0);
    }
    let seen = observe_exception(1, || {
        unsafe {
            if store {
                asm!("
                    .set noat
                    SW $0, 0($2)
                ", in("$2") address);
            } else {
                asm!("
                    .set noat
                    LW $0, 0($2)
                ", in("$2") address);
            }
        }
        Ok(())
    })?;

    match seen {
        None => Ok(Observed {
            upper: (address >> 32) as u32,
            code: None,
            badvaddr: 0,
            context: 0,
            xcontext: 0,
        }),
        Some((context, count)) => {
            if count != 1 {
                return Err(format!("{:#018x}: {} exceptions", address, count));
            }
            Ok(Observed {
                upper: (address >> 32) as u32,
                code: Some((context.cause.raw_value() >> 2) & 0x1F),
                badvaddr: context.badvaddr,
                context: context.context.raw_value() as u64,
                xcontext: context.xcontext.raw_value() as u64,
            })
        }
    }
}

/// Probes LW or SW with a base register whose lower 32 bits are in a given segment and whose
/// upper 32 bits take many values, in 32-bit kernel addressing (KX = 0).
pub struct AddressErrorVsTlbMiss32 {}

impl Test for AddressErrorVsTlbMiss32 {
    fn name(&self) -> &str {
        "Address error vs TLB miss (32 bit addressing, upper address bits)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        let mut result: Vec<Box<dyn Any>> = Vec::new();
        for store in [false, true] {
            for family in 0..FAMILIES.len() as u32 {
                result.push(Box::new((store, family)));
            }
        }
        result
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(bool, u32)>() {
            Some((store, family)) => run_probe(*store, *family),
            _ => Err(String::from("Unexpected pattern")),
        }
    }
}

fn run_probe(store: bool, family: u32) -> Result<(), String> {
    // The runner doesn't necessarily know how to print this value type, so name it ourselves
    let header = format!(
        "{} {}",
        if store { "SW" } else { "LW" },
        FAMILIES[family as usize]
    );

    // Words the probes may end up reading or (if the address is unexpectedly valid) zeroing
    let buffer = [0x1234_5678u32; 4];
    let buffer_address = buffer.as_ptr() as usize as u32;
    // The TLB must not map the kuseg/kseg2 addresses we use
    unsafe {
        cop0::clear_tlb();
    }

    let low = low_word(family, buffer_address);
    let mut mismatches: Vec<String> = Vec::new();
    let mut unknown: BTreeMap<Option<u32>, Vec<Observed>> = BTreeMap::new();

    for upper in upper_patterns() {
        let address = ((upper as u64) << 32) | low as u64;
        let observed = observe_access(store, address).map_err(|e| format!("{}: {}", header, e))?;
        match predicted(store, family, upper) {
            Some(expected) => {
                if expected != observed.code {
                    mismatches.push(format!(
                        "upper {:08x}: expected {}, got {}",
                        upper,
                        code_name(expected),
                        code_name(observed.code)
                    ));
                }
            }
            None => unknown.entry(observed.code).or_default().push(observed),
        }
    }
    // Keep the buffer alive until all probes are done
    core::hint::black_box(&buffer);

    if mismatches.is_empty() && unknown.is_empty() {
        return Ok(());
    }

    let mut report = format!("{}:", header);
    for line in &mismatches {
        report.push_str(&format!("\n  MISMATCH {}", line));
    }
    for (code, group) in &unknown {
        let list: Vec<String> = group.iter().map(|o| format!("{:08x}", o.upper)).collect();
        report.push_str(&format!(
            "\n  {} for upper = {}",
            code_name(*code),
            list.join(" ")
        ));
        let first = &group[0];
        if first.code.is_some() {
            report.push_str(&format!(
                "\n    e.g. upper {:08x}: BadVAddr={:016x} Context={:016x} XContext={:016x}",
                first.upper, first.badvaddr, first.context, first.xcontext
            ));
        }
    }
    Err(report)
}
