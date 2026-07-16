pub mod compare;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use crate::cop0;
use crate::cop0::{
    count, count32_64, count64, read_cop0_64, set_count, set_count32_64, set_count64,
    write_cop0_32_64, RegisterIndex, Status,
};
use crate::tests::soft_asserts::{soft_assert_eq, soft_assert_range};
use crate::tests::{Level, Test};

pub struct IndexMasking;

impl Test for IndexMasking {
    fn name(&self) -> &str {
        "Index (masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous = cop0::index();

        unsafe { cop0::set_index(0xFFFFFFFF) }
        let readback = cop0::index();
        soft_assert_eq(readback, 0x8000003F, "Index written with 0xFFFFFFFF")?;

        unsafe { cop0::set_index(0x00000000) }
        let readback = cop0::index();
        soft_assert_eq(readback, 0x00000000, "Index written with 0x00000000")?; // verifies bit 31 is mutable

        unsafe { cop0::set_index(previous) }

        Ok(())
    }
}

/// Tests the behavior of the COP0 Random register after writing to Wired and executing any other instructions.
///
/// When writing any value to Wired (which hardware will mask to a value of 0-63 inclusive), the
/// Random register is automatically set to 31. For each instruction after, the Random register is
/// decremented by 1, and follows the following behavior (pseudo-code):
///
/// ```
/// instruction_cycle() {
///     if random == wired {
///         random = 31;
///     } else {
///         random = (random - 1) & 63;
///     }
/// }
/// ```
pub struct RandomDecrement;

impl Test for RandomDecrement {
    fn name(&self) -> &str {
        "Random (decrement)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        fn simulate(cycles: u32, wired: u32) -> u32 {
            let mut random = 31;
            for _ in 0..cycles {
                if random == wired {
                    random = 31;
                } else {
                    random = (random - 1) & 63;
                }
            }

            random
        }

        // Note that when mfc0 is used after mtc0, extra cycles are required. That combined with
        // the nature of the test, assembly is used to ensure timing accuracy.
        fn perform(wired: u32) -> Result<(), String> {
            let after1: u32;
            let after16: u32;
            let after31: u32;
            let after100: u32;

            unsafe {
                asm!("
                    .set noat
                    nop
                    mtc0 {gpr_in}, ${WIRED}
                    nop
                    nop
                    mfc0 {gpr_after1}, ${RANDOM}
                    
                    nop; nop; nop; nop; nop; nop; nop; nop; nop; nop;
                    nop; nop; nop; nop;
                    mfc0 {gpr_after16}, ${RANDOM}
                    
                    nop; nop; nop; nop; nop; nop; nop; nop; nop; nop;
                    nop; nop; nop; nop;
                    mfc0 {gpr_after31}, ${RANDOM}
                    
                    nop; nop; nop; nop; nop; nop; nop; nop; nop; nop;
                    nop; nop; nop; nop; nop; nop; nop; nop; nop; nop;
                    nop; nop; nop; nop; nop; nop; nop; nop; nop; nop;
                    nop; nop; nop; nop; nop; nop; nop; nop; nop; nop;
                    nop; nop; nop; nop; nop; nop; nop; nop; nop; nop;
                    nop; nop; nop; nop; nop; nop; nop; nop; nop; nop;
                    nop; nop; nop; nop; nop;
                    nop; nop; nop;
                    mfc0 {gpr_after100}, ${RANDOM}
                ",
                    gpr_in = in(reg) wired,
                    gpr_after1 = out(reg) after1,
                    gpr_after16 = out(reg) after16,
                    gpr_after31 = out(reg) after31,
                    gpr_after100 = out(reg) after100,
                    WIRED = const RegisterIndex::Wired as usize,
                    RANDOM = const RegisterIndex::Random as usize,
                )
            }

            soft_assert_eq(
                after1,
                simulate(1, wired),
                &format!("Random, 1 instruction after setting Wired = {}", wired),
            )?;
            soft_assert_eq(
                after16,
                simulate(16, wired),
                &format!("Random, 16 instructions after setting Wired = {}", wired),
            )?;
            soft_assert_eq(
                after31,
                simulate(31, wired),
                &format!("Random, 31 instructions after setting Wired = {}", wired),
            )?;
            soft_assert_eq(
                after100,
                simulate(100, wired),
                &format!("Random, 100 instructions after setting Wired = {}", wired),
            )?;

            Ok(())
        }

        // Values above 63 are technically valid, but hardware will ignore any numbers larger than 63.
        // There is separate test for register masking.
        for i in 0..=63 {
            perform(i)?;
        }

        Ok(())
    }
}

/// Tests if read/write masking is correct for the COP0 Random register.
///
/// This register is read-only. Writes are ignored. In order to test this, we need to know what value
/// Random is supposed to contain after attempting a write. This requires writing to Wired, and the
/// use of assembly code to ensure instruction-timing accuracy.
///
/// This test relies on similar behavior as [RandomDecrement], so if decrement fails, this likely will too.
pub struct RandomMasking;

impl Test for RandomMasking {
    fn name(&self) -> &str {
        "Random (masking)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let readback: u32;
        unsafe {
            asm!("
               .set noat
                nop
                mtc0 {gpr_wired}, ${WIRED}
                nop
                nop
                mtc0 {gpr_test}, ${RANDOM}
                nop
                nop
                mfc0 {gpr_readback}, ${RANDOM}
            ",
                gpr_wired = in(reg) 0u32,
                gpr_test = in(reg) 0xFFFFFFFFu32,
                gpr_readback = out(reg) readback,
                WIRED = const RegisterIndex::Wired as usize,
                RANDOM = const RegisterIndex::Random as usize,
            )
        }
        soft_assert_eq(readback, 27, "Random was written as 0xFFFFFFFF, Wired written as 32, expecting Random write to be ignored")?;

        Ok(())
    }
}

/// Tests mfc0 behavior in relation to Random being set by hardware when Wired is written to.
///
/// Due to CPU hazards, when the mfc0 instruction is used after mtc0, there is supposed to be two
/// instructions inbetween them. However, it is technically possible to use mtc0 earlier than
/// recommended. Compilers and assemblers typically will deal with this automatically but not always.
///
/// If a read from Random occurs immediately after a write to Wired, the read value will be as if
/// the write to Wired was replaced with any other instruction (e.g. nop).
///
/// If the read from Random occurs 1 instruction later, then the value will be 31.
pub struct RandomReadEarly;

impl Test for RandomReadEarly {
    fn name(&self) -> &str {
        "Random (read early)"
    }

    fn level(&self) -> Level {
        Level::COP0Hazard
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // test immediate readback
        let start: u32;
        let readback: u32;
        unsafe {
            asm!("
                .set noat
                nop
                mtc0 {gpr_init_wired}, ${WIRED}
                nop;
                nop; nop; nop; nop; nop;
                nop; nop; nop; nop; nop;
                
                mfc0 {gpr_start}, ${RANDOM}
                nop
                nop
                mtc0 {gpr_wired}, ${WIRED}
                mfc0 {gpr_readback}, ${RANDOM}
                nop
                nop
            ",
                gpr_init_wired = in(reg) 20u32,
                gpr_start = out(reg) start,
                gpr_wired = in(reg) 5u32, // value here shouldn't matter
                gpr_readback = out(reg) readback,
                WIRED = const RegisterIndex::Wired as usize,
                RANDOM = const RegisterIndex::Random as usize,
            )
        }

        soft_assert_eq(start, 21, "Wired set to 20, Random decremented 10 times")?;
        soft_assert_eq(
            readback,
            29,
            "Random expected to wrap at previously set Wired bound",
        )?;

        // test delayed readback by 1 instruction cycle
        let start: u32;
        let readback: u32;
        unsafe {
            asm!("
                .set noat
                nop
                mtc0 {gpr_init_wired}, ${WIRED}
                nop;
                nop; nop; nop; nop; nop;
                nop; nop; nop; nop; nop;
                
                mfc0 {gpr_start}, ${RANDOM}
                nop
                nop
                mtc0 {gpr_wired}, ${WIRED}
                nop
                mfc0 {gpr_readback}, ${RANDOM}
                nop
                nop
            ",
                gpr_init_wired = in(reg) 20u32,
                gpr_start = out(reg) start,
                gpr_wired = in(reg) 5u32, // value here shouldn't matter
                gpr_readback = out(reg) readback,
                WIRED = const RegisterIndex::Wired as usize,
                RANDOM = const RegisterIndex::Random as usize,
            )
        }

        soft_assert_eq(start, 21, "Wired set to 20, Random decremented 10 times")?;
        soft_assert_eq(readback, 31, "Random expected to reset")?;

        Ok(())
    }
}

pub struct ContextMasking {}

impl Test for ContextMasking {
    fn name(&self) -> &str {
        "Context (masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous = cop0::context();
        for value in [
            0,
            1,
            5,
            15,
            30,
            31,
            32,
            63,
            64,
            64,
            1020,
            63102,
            0x0F000000,
            0xFFFF0002,
            0xFFFFFFFF,
            0xF2345678_0000000,
            0xFFFFFFFF_FFFFFFFF,
            0,
        ] {
            unsafe {
                cop0::set_context_64(value);
            }
            let expected = (value & 0xFFFFFFFF_FF800000) | (previous.raw_value() & 0x7FFFFF);
            let readback = cop0::context().raw_value();
            soft_assert_eq(
                readback,
                expected,
                format!("Context was written as {:x}", value).as_str(),
            )?;
        }
        Ok(())
    }
}

pub struct ContextMixedBitWriting {}

impl Test for ContextMixedBitWriting {
    fn name(&self) -> &str {
        "Context (sign extension)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous = cop0::context_64();

        unsafe {
            cop0::set_context_64(0x12345678_00000000);
        }
        let expected1 = 0x12345678_00000000 | (previous & 0x7FFFFF);
        soft_assert_eq(
            cop0::context_64(),
            expected1,
            format!("Context after writing 64 bit").as_str(),
        )?;

        unsafe {
            cop0::set_context_32_64(0xFF123FFF_8B000000);
        }
        let expected2 = 0xFF123FFF_8B000000 | (previous & 0x7FFFFF);
        soft_assert_eq(
            cop0::context_64(),
            expected2,
            format!("Writing Context (MTC0) also transfers the upper bits (1)").as_str(),
        )?;

        unsafe {
            cop0::set_context_32_64(0xFFFFF123_7B000000);
        }
        let expected3 = 0xFFFFF123_7B000000 | (previous & 0x7FFFFF);
        soft_assert_eq(
            cop0::context_64(),
            expected3,
            format!("Writing Context (MTC0) also transfers the upper bits (2)").as_str(),
        )?;

        Ok(())
    }
}

/// Tests if read/write masking is correct for the COP0 Wired register.
pub struct WiredMasking;

impl Test for WiredMasking {
    fn name(&self) -> &str {
        "Wired (masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        unsafe {
            cop0::set_wired(0xFFFFFFFF);
        }
        soft_assert_eq(cop0::wired(), 63, "Wired was written as 0xFFFFFFFF")?;

        Ok(())
    }
}

pub struct XContextMasking {}

impl Test for XContextMasking {
    fn name(&self) -> &str {
        "XContext (masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous = cop0::xcontext_64();
        for value in [
            0xF1234567_89ABCDEF,
            0xFFFFFFFF_FFFFFFFF,
            0x00000000_FFFFFFFF,
            0x00000001_FFFFFFFF,
        ] {
            unsafe {
                cop0::set_xcontext_64(value);
            }
            let expected = (value & 0xFFFFFFFE_00000000) | (previous & 0x00000001_FFFFFFFF);
            let readback = cop0::xcontext_64();
            soft_assert_eq(
                readback,
                expected,
                format!(
                    "XContext was written as {:x}. But as it is readonly it shouldn't change",
                    value
                )
                .as_str(),
            )?;
        }
        Ok(())
    }
}

pub struct XContextMaskingMixed {}

impl Test for XContextMaskingMixed {
    fn name(&self) -> &str {
        "XContext (masking, mixed)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous = cop0::xcontext_64();

        unsafe {
            cop0::set_xcontext_64(0x12345678_00000000);
        }
        let expected1 = 0x12345678_00000000 | (previous & 0x00000001_FFFFFFFF);
        soft_assert_eq(
            cop0::xcontext_64(),
            expected1,
            format!("XContext after writing 64 bit").as_str(),
        )?;

        unsafe {
            cop0::set_xcontext_32_64(0x87654321_8B000000);
        }
        let expected2 = 0x87654320_00000000 | (previous & 0x00000001_FFFFFFFF);
        soft_assert_eq(
            cop0::xcontext_64(),
            expected2,
            format!("Writing XContext (MTC0) also writes the upper 32 bits").as_str(),
        )?;

        unsafe {
            cop0::set_xcontext_32_64(0x8B000000);
        }
        let expected3 = previous & 0x00000001_FFFFFFFF;
        soft_assert_eq(
            cop0::xcontext_64(),
            expected3,
            format!("Writing XContext (MTC0) also writes the upper 32 bits").as_str(),
        )?;
        Ok(())
    }
}

pub struct BadVAddrReadOnly {}

impl Test for BadVAddrReadOnly {
    fn name(&self) -> &str {
        "BadVAddr (readonly)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous = cop0::badvaddr();
        for value in [
            0xF1234567_89ABCDEF,
            0x00000000_00000000,
            0x00000000_FFFFFFFF,
            0xFFFFFFFF_FFFFFFFF,
        ] {
            unsafe {
                cop0::set_badvaddr(value);
            }
            let expected = previous;
            let readback = cop0::badvaddr();
            soft_assert_eq(
                readback,
                expected,
                format!(
                    "BadVAddr was written as {:x}. But as it is readonly it shouldn't change",
                    value
                )
                .as_str(),
            )?;
        }
        Ok(())
    }
}

pub struct ExceptPCNoMasking {}

impl Test for ExceptPCNoMasking {
    fn name(&self) -> &str {
        "ExceptPC (no masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        for value in [
            0xF7654321_89ABCDEF,
            0x00000000_00000000,
            0x00000000_FFFFFFFF,
            0xFFFFFFFF_FFFFFFFF,
        ] {
            unsafe {
                cop0::set_exceptpc(value);
            }
            let expected = value;
            let readback = cop0::exceptpc();
            soft_assert_eq(
                readback,
                expected,
                format!(
                    "ExceptPC was written as 0x{:x} via DMTC0. It shouldn't be masked",
                    value
                )
                .as_str(),
            )?;

            unsafe {
                cop0::set_exceptpc_32_64(value);
            }
            let expected = value;
            let readback = cop0::exceptpc();
            soft_assert_eq(
                readback,
                expected,
                format!(
                    "ExceptPC was written as 0x{:x} via MTC0. It shouldn't be masked",
                    value
                )
                .as_str(),
            )?;
        }
        Ok(())
    }
}

pub struct ErrorEPCNoMasking {}

impl Test for ErrorEPCNoMasking {
    fn name(&self) -> &str {
        "ErrorEPC (no masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        for value in [
            0xF7654321_89ABCDEF,
            0x00000000_00000000,
            0x00000000_FFFFFFFF,
            0xFFFFFFFF_FFFFFFFF,
            0x12345678_AABBCCDD,
        ] {
            unsafe {
                cop0::set_errorepc(value);
            }
            let expected = value;
            let readback = cop0::errorepc();
            soft_assert_eq(
                readback,
                expected,
                format!(
                    "ErrorEPC was written as 0x{:x}. It shouldn't be masked",
                    value
                )
                .as_str(),
            )?;
        }
        Ok(())
    }
}

pub struct LLAddrIs32Bit {}

impl Test for LLAddrIs32Bit {
    fn name(&self) -> &str {
        "LLAddr (32 bit)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        for value in [
            0xF7654321_89ABCDEF,
            0x00000000_00000000,
            0x00000000_FFFFFFFF,
            0xFFFFFFFF_FFFFFFFF,
        ] {
            unsafe {
                cop0::set_lladdr(value);
            }
            let expected = value & 0xFFFFFFFF;
            let readback = cop0::lladdr();
            soft_assert_eq(
                readback,
                expected,
                format!(
                    "LLAddr was written as 0x{:x}. Only its lower 32 bit should change",
                    value
                )
                .as_str(),
            )?;
        }
        Ok(())
    }
}

pub struct StatusIs32Bit {}

impl Test for StatusIs32Bit {
    fn name(&self) -> &str {
        "Status (upper 32 bit ignored)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous = cop0::status_64();
        for value in [0xFFFFFFFF_00000000, 0x00000000_00000000] {
            let write_value = (value & 0xFFFFFFFF_00000000) | (previous & 0xFFFFFFFF);
            unsafe {
                cop0::set_status_64(write_value);
            }
            let expected = previous;
            let readback = cop0::status_64();
            soft_assert_eq(
                readback,
                expected,
                format!(
                    "Status was written as 0x{:x}. It's upper 32 bit should be constant",
                    write_value
                )
                .as_str(),
            )?;
        }
        Ok(())
    }
}

/// Tests if read/write masking is correct for the COP0 Status register.
pub struct StatusMasking;

impl Test for StatusMasking {
    fn name(&self) -> &str {
        "Status (masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // Bit 19 is not writable
        let previous = cop0::status();
        let bit19 = 1 << 19;
        unsafe { cop0::set_status(Status::new_with_raw_value(previous.raw_value() | bit19)) }
        let readback = cop0::status();

        soft_assert_eq(
            readback,
            previous,
            "Status bit-19 set. Expected readback bit-19 to be clear",
        )?;

        Ok(())
    }
}

/// Tests if read/write masking is correct for the COP0 Config register.
pub struct ConfigMasking;

impl Test for ConfigMasking {
    fn name(&self) -> &str {
        "Config (masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let previous = cop0::config();
        unsafe { cop0::set_config((previous & 0x7F00800F) | 0x80F91B90) }
        let readback = cop0::config();

        soft_assert_eq(
            readback,
            (previous & 0x7F00800F) | 0x00066460,
            "Config written with {:#010X}",
        )?;

        Ok(())
    }
}

/// Tests COP0 ProcessorRevisionId register for expected values and read-only behavior.
///
/// Sample testing of consoles from the n64 homebrew community, shows that the majority of systems
/// report the value 0x00000B22 when reading this register. However, one sample reported the
/// value 0x00000B10.
///
/// More info: https://github.com/lemmy-64/n64-systemtest/pull/25#issuecomment-1146854023
pub struct ProcessorRevisionIdMasking;

impl Test for ProcessorRevisionIdMasking {
    fn name(&self) -> &str {
        "ProcessorRevisionID (masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        unsafe {
            cop0::set_previd(0xFFFFFFFF);
        }
        let readback = cop0::previd();

        match soft_assert_eq(
            readback,
            0x00000B22,
            "PRId written with 0xFFFFFFFF. Expected constant readback of 0xB22 or 0xB10",
        ) {
            Ok(()) => return Ok(()),
            Err(_) => soft_assert_eq(
                readback,
                0x00000B10,
                "PRId written with 0xFFFFFFFF. Expected constant readback of 0xB22 or 0xB10",
            )?,
        }

        Ok(())
    }
}

pub struct TagLoMasking;

impl Test for TagLoMasking {
    fn name(&self) -> &str {
        "TagLo (masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        for value in [
            0xF1234567_89ABCDEF,
            0xFFFFFFFF_FFFFFFFF,
            0x00000000_FFFFFFFF,
            0x00000001_FFFFFFFF,
            0,
        ] {
            cop0::set_taglo64(value);
            soft_assert_eq(
                cop0::taglo64(),
                value & 0xFFFFFFFF,
                format!("TagLo was written as {:x} and then readback", value).as_str(),
            )?;
        }
        Ok(())
    }
}

pub struct TagHiMasking;

impl Test for TagHiMasking {
    fn name(&self) -> &str {
        "TagHi (masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        for value in [
            0xF1234567_89ABCDEF,
            0xFFFFFFFF_FFFFFFFF,
            0x00000000_FFFFFFFF,
            0x00000001_FFFFFFFF,
            0,
        ] {
            cop0::set_taghi64(value);
            soft_assert_eq(
                cop0::taglo64(),
                0,
                "TagHi can not be written to. It is always 0",
            )?;
        }
        Ok(())
    }
}

pub struct CacheErrMasking;

impl Test for CacheErrMasking {
    fn name(&self) -> &str {
        "CacheErr (masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        for value in [
            0xF1234567_89ABCDEF,
            0xFFFFFFFF_FFFFFFFF,
            0x00000000_FFFFFFFF,
            0x00000001_FFFFFFFF,
            0,
        ] {
            cop0::set_cacheerr64(value);
            soft_assert_eq(
                cop0::cacheerr64(),
                0,
                "CacheErr can not be written to. It is always 0",
            )?;
        }
        Ok(())
    }
}

pub struct PErrMasking;

impl Test for PErrMasking {
    fn name(&self) -> &str {
        "PErr (masking)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        for value in [
            0xF1234567_89ABCDEF,
            0xFFFFFFFF_FFFFFFFF,
            0x00000000_FFFFFFFF,
            0x00000001_FFFFFFFF,
            0,
        ] {
            cop0::set_perr64(value);
            soft_assert_eq(cop0::perr64(), value & 0xFF, "PErr has the write mask 0xFF")?;
        }
        Ok(())
    }
}

/// Tests write/read behavior for all unused COP0 registers, using an extra COP0 write.
///
/// Unused registers include number 7, 21, 22, 23, 24, 25, and 31. Writes to these registers exhibit
/// odd behavior. Reads after writes, will always return the same value. But if another write occurs
/// to another COP0 register, the readback from the first register, will be whatever was written to
/// the second.
pub struct UnusedRegistersExtraMtc0;

impl Test for UnusedRegistersExtraMtc0 {
    fn name(&self) -> &str {
        "Unused COP0 Registers (with extra mtc0)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        #[inline]
        fn write_read_cop0<const INDEX: u32>(value: u32, junk: u32) -> u32 {
            let readback: u32;
            unsafe {
                asm!("
                    .set noat
                    mtc0 {gpr_write}, ${cop0_reg}
                    nop
                    nop
                    mtc0 {junk}, $11
                    nop
                    nop
                    mfc0 {gpr_read}, ${cop0_reg}
                    nop
                    nop
                ",
                gpr_write = in(reg) value,
                cop0_reg = const INDEX,
                junk = in(reg) junk,
                gpr_read = out(reg) readback,
                );
            }

            readback
        }

        macro_rules! perform_test {
            ($reg:expr, $value:expr, $junk:expr) => {
                let readback = write_read_cop0::<$reg>($value, $junk);
                soft_assert_eq(readback, $junk, &format!("Unused COP0 Reg{} written with {:#010X}, then any other COP0 register written with {:#010X} before readback", $reg, $value, $junk))?;
            }
        }

        // Test with different write and junk values to make sure emulator isn't cheating
        for write in [
            0x13171A1Eu32,
            0xAAAAAAAA,
            0xFEDCBA98,
            0x12345678,
            0xFFFFFFFF,
        ] {
            for junk in [0x8BADF00Du32, 0xDEADBEEF, 0xBADDCAFE] {
                perform_test!(7, write, junk);
                perform_test!(21, write, junk);
                perform_test!(22, write, junk);
                perform_test!(23, write, junk);
                perform_test!(24, write, junk);
                perform_test!(25, write, junk);
                perform_test!(31, write, junk);
            }
        }

        Ok(())
    }
}

/// Tests write/read behavior for all unused COP0 registers, using an extra unrelated instructions.
///
/// Unused registers include number 7, 21, 22, 23, 24, 25, and 31. Writes to these registers exhibit
/// odd behavior. Reads after writes, will always return the same value. But if another write occurs
/// to another COP0 register, the readback from the first register, will be whatever was written to
/// the second.
///
/// This test uses extra instructions between the write and readback, that simulate other typical
/// CPU operations. These do not affect the latched state from the COP0 write, thus the readback
/// should be the same value as what was written.
pub struct UnusedRegistersExtraUnrelated;

impl Test for UnusedRegistersExtraUnrelated {
    fn name(&self) -> &str {
        "Unused COP0 Registers (with extra unrelated instructions)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        #[inline]
        fn write_read_cop0<const INDEX: u32>(value: u64) -> u64 {
            unsafe {
                write_cop0_32_64::<INDEX>(value);
            }

            // Simulate some other work - it won't affect the latch
            unsafe {
                asm!(
                    "
                    .set noat
                    addiu $0, $0, 0xABCD
                    xor $0, $5, $9
                    mflo $0
                ",
                );
            }

            unsafe { read_cop0_64::<INDEX>() }
        }

        macro_rules! perform_test {
            ($reg:expr, $value:expr) => {
                let readback = write_read_cop0::<$reg>($value);
                soft_assert_eq(readback, $value, &format!("Unused COP0 Reg{} written with {:#010X}, then various unrelated CPU instructions (ADDIU, XOR, MFLO) before DMFC0. Expecting same value back.", $reg, $value))?;
            }
        }

        // Test with different write and junk values to make sure emulator isn't cheating
        for value in [
            0x12345678_13171A1Eu64,
            0x00000000_AAAAAAAA,
            0x11111111_FEDCBA98,
            0x12345678,
            0xFFFFFFFF_FFFFFFFF,
        ] {
            perform_test!(7, value);
            perform_test!(21, value);
            perform_test!(22, value);
            perform_test!(23, value);
            perform_test!(24, value);
            perform_test!(25, value);
            perform_test!(31, value);
        }

        Ok(())
    }
}

/// Tests write/read behavior for all unused COP0 registers, by reading back shortly after writing.
///
/// Unused registers include number 7, 21, 22, 23, 24, 25, and 31. Writes to these registers exhibit
/// odd behavior. Reads after writes, will always return the same value. But if another write occurs
/// to another COP0 register, the readback from the first register, will be whatever was written to
/// the second.
///
/// This test doesn't write to any other register before reading back.
pub struct UnusedRegistersWriteRead;

impl Test for UnusedRegistersWriteRead {
    fn name(&self) -> &str {
        "Unused COP0 Registers (write/read)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        #[inline]
        fn write_read_cop0<const INDEX: u32>(value: u32) -> u32 {
            let readback: u32;
            unsafe {
                asm!("
                    .set noat
                    mtc0 {gpr_write}, ${cop0_reg}
                    nop
                    nop
                    mfc0 {gpr_read}, ${cop0_reg}
                    nop
                    nop
                ",
                gpr_write = in(reg) value,
                cop0_reg = const INDEX,
                gpr_read = out(reg) readback,
                );
            }

            readback
        }

        macro_rules! perform_test {
            ($reg:expr, $value:expr) => {
                let readback = write_read_cop0::<$reg>($value);
                soft_assert_eq(
                    readback,
                    $value,
                    &format!(
                        "Unused COP0 Reg{} written with {:#010X}, expecting same value back",
                        $reg, $value
                    ),
                )?;
            };
        }

        // Test with different write values to make sure emulator isn't cheating
        for write in [
            0x13171A1Eu32,
            0xAAAAAAAA,
            0xFEDCBA98,
            0x12345678,
            0xFFFFFFFF,
        ] {
            perform_test!(7, write);
            perform_test!(21, write);
            perform_test!(22, write);
            perform_test!(23, write);
            perform_test!(24, write);
            perform_test!(25, write);
            perform_test!(31, write);
        }

        Ok(())
    }
}

/// Various COP0.Count related tests
pub struct Count;

impl Test for Count {
    fn name(&self) -> &str {
        "Count"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let old_count = count();

        unsafe { set_count(0x100) }

        let count1_32 = count32_64();
        let count1_64 = count64();

        // Write without properly sign-extending - that should be completely ignored
        unsafe { set_count32_64(0xFFFF0000) }

        let count2_32 = count32_64();
        let count2_64 = count64();

        unsafe { set_count64(0xFFFF_FFFF0000) }

        let count3_32 = count32_64();
        let count3_64 = count64();

        // Use incorrect sign extension
        unsafe { set_count32_64(0xFF0000FF_00000100) }

        let count4_32 = count32_64();
        let count4_64 = count64();

        // Restore previous count as otherwise the test harness thinks this test uses a lot of time
        unsafe {
            set_count(old_count);
        }

        soft_assert_range(
            count1_32,
            0x100,
            0x300,
            "MFC0 COUNT after MTC0 COUNT <- 0x100",
        )?;
        soft_assert_range(
            count1_64,
            0x100,
            0x300,
            "DMFC0 COUNT after MTC0 COUNT <- 0x100",
        )?;

        soft_assert_range(
            count2_32,
            0xFFFFFFFF_FFFF0000,
            0xFFFFFFFF_FFFF0200,
            "MFC0 COUNT after MTC0 COUNT <- 0x00000000_FFFF0000",
        )?;
        soft_assert_range(
            count2_64,
            0xFFFF0000,
            0xFFFF0200,
            "DMFC0 COUNT after MTC0 COUNT <- 0x00000000_FFFF0000",
        )?;

        soft_assert_range(
            count3_32,
            0xFFFFFFFF_FFFF0000,
            0xFFFFFFFF_FFFF0200,
            "MFC0 COUNT after DMTC0 COUNT <- 0xFFFF_FFFF0000",
        )?;
        soft_assert_range(
            count3_64,
            0xFFFF0000,
            0xFFFF0200,
            "DMFC0 COUNT after DMTC0 COUNT <- 0xFFFF_FFFF0000",
        )?;

        soft_assert_range(
            count4_32,
            0x100,
            0x300,
            "MFC0 COUNT after MTC0 COUNT <- 0xFF0000FF_00000100",
        )?;
        soft_assert_range(
            count4_64,
            0x100,
            0x300,
            "DMFC0 COUNT after MTC0 COUNT <- 0xFF0000FF_00000100",
        )?;

        Ok(())
    }
}

/// Write to a large COUNT value, wait a bit and observe overflow
pub struct CountOverflow;

impl Test for CountOverflow {
    fn name(&self) -> &str {
        "Count (overflow)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let old_count = count();

        // The upper 32 bits are ignored, so place some garbage in them
        unsafe { set_count64(0xF0FF0012_FFFFFFFD) }

        // Wait for the counter to overflow
        unsafe { asm!("NOP; NOP; NOP; NOP; NOP; NOP; NOP; NOP; NOP; NOP; NOP; NOP; ") }

        let count_32 = count32_64();
        let count_64 = count64();

        // Restore previous count as otherwise the test harness thinks this test uses a lot of time
        unsafe {
            set_count(old_count);
        }

        // This is a very inaccurate test - tests/timing has precise COUNT tests. Here we just want to observe the overflow behavior

        soft_assert_range(
            count_32,
            0x0,
            0x200,
            "MFC0 COUNT after DMTC0 COUNT <- 0x00000000_FFFFFFFD and wait until overflow",
        )?;
        soft_assert_range(
            count_64,
            0x0,
            0x200,
            "DMFC0 COUNT after DMTC0 COUNT <- 0x00000000_FFFFFFFD and wait until overflow",
        )?;

        Ok(())
    }
}

/// Reading COUNT immediately after writing it returns the just written value a couple of times,
/// like if count doesn't increase for a little bit
/// TODO: Reading COUNT right after performing e.g. DMULTU returns a non-incremented value
pub struct CountHazards {}

impl Test for CountHazards {
    fn name(&self) -> &str {
        "MTC0/MFC0 COUNT hazards"
    }

    fn level(&self) -> Level {
        Level::COP0Hazard
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        for count_value in [0, 100, 0x1234, 0x8000000, 0xFFFFFFFC, 0xFFFFFFFF] {
            let backup_count = count();
            let out0: u32;
            let out1: u32;
            let out2: u32;
            let out3: u32;
            unsafe {
                asm!("
                    .set noat
1:
                    NOP
                    MTC0 {count_value}, ${COUNT}

                    // Read back
                    MFC0 {out0}, ${COUNT}
                    MFC0 {out1}, ${COUNT}
                    MFC0 {out2}, ${COUNT}
                    MFC0 {out3}, ${COUNT}

                    SUB {counter}, {counter}, 1
                    BNE {counter}, $0, 1b
                    NOP  // delay
                ",
                out0 = out(reg) out0,
                out1 = out(reg) out1,
                out2 = out(reg) out2,
                out3 = out(reg) out3,
                count_value = in(reg) count_value,
                counter = inout(reg) 2 => _,
                COUNT = const RegisterIndex::Count as usize);
            }

            unsafe {
                set_count(backup_count);
            }

            soft_assert_eq(out0, count_value + 0, "First readback")?;
            soft_assert_eq(out1, count_value + 0, "Second readback")?;
            soft_assert_eq(out2, count_value + 0, "Third readback")?;
            soft_assert_eq(out3, count_value + 1, "Fourth readback")?;
        }
        Ok(())
    }
}
