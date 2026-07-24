use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use arbitrary_int::prelude::*;

use crate::cop0::{self, make_entry_hi, make_entry_lo, Coherency, Pagemask, Status};
use crate::tests::soft_asserts::{soft_assert_eq, soft_assert_neq};
use crate::tests::{Level, Test};
use crate::uncached_memory::UncachedHeapMemory;

pub struct LL {}

impl Test for LL {
    fn name(&self) -> &str {
        "LL (sign extension + LLAddr)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        unsafe {
            cop0::set_status(Status::ADDRESSING_MODE_64_BIT);
        }

        let mut memory: u32 = 0x89AB_CDEF;
        let ptr = (&mut memory as *mut u32) as isize as u64;

        let ll_value: u64;
        unsafe {
            asm!("
                .set noat
                LL $4, 0($3)
            ", in("$3") ptr, out("$4") ll_value);
        }

        let expected_lladdr = ((ptr as usize & 0x1FFF_FFFF) >> 4) as u64;
        soft_assert_eq(ll_value, 0xFFFF_FFFF_89AB_CDEF, "LL value")?;
        soft_assert_eq(cop0::lladdr(), expected_lladdr, "LLAddr after LL")?;

        Ok(())
    }
}

pub struct SC {}

impl Test for SC {
    fn name(&self) -> &str {
        "SC (successful store conditional)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        unsafe {
            cop0::set_status(Status::ADDRESSING_MODE_64_BIT);
        }

        let mut memory: u32 = 0x89AB_CDEF;
        let ptr = (&mut memory as *mut u32) as isize as u64;

        let ll_value: u64;
        let sc_status: u64;
        unsafe {
            asm!("
                .set noat
                LL $5, 0($3)
                LUI $4, 0x1357
                ORI $4, $4, 0x9BDF
                SC $4, 0($3)
            ", in("$3") ptr, out("$4") sc_status, out("$5") ll_value);
        }

        soft_assert_eq(ll_value, 0xFFFF_FFFF_89AB_CDEF, "LL value before SC")?;
        soft_assert_eq(sc_status, 1, "SC success flag")?;
        soft_assert_eq(memory, 0x1357_9BDF, "Memory after SC")?;

        Ok(())
    }
}

pub struct LLD {}

impl Test for LLD {
    fn name(&self) -> &str {
        "LLD (load linked doubleword)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        unsafe {
            cop0::set_status(Status::ADDRESSING_MODE_64_BIT);
        }

        let mut memory: u64 = 0x89AB_CDEF_0123_4567;
        let ptr = (&mut memory as *mut u64) as isize as u64;

        let lld_value: u64;
        unsafe {
            asm!("
                .set noat
                LLD $4, 0($3)
            ", in("$3") ptr, out("$4") lld_value);
        }

        let expected_lladdr = ((ptr as usize & 0x1FFF_FFFF) >> 4) as u64;
        soft_assert_eq(lld_value, 0x89AB_CDEF_0123_4567, "LLD value")?;
        soft_assert_eq(cop0::lladdr(), expected_lladdr, "LLAddr after LLD")?;

        Ok(())
    }
}

pub struct SCD {}

impl Test for SCD {
    fn name(&self) -> &str {
        "SCD (successful store conditional doubleword)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        unsafe {
            cop0::set_status(Status::ADDRESSING_MODE_64_BIT);
        }

        let mut memory: u64 = 0x89AB_CDEF_0123_4567;
        let ptr = (&mut memory as *mut u64) as isize as u64;

        let lld_value: u64;
        let scd_status: u64;
        let write_value: u64 = 0x1020_3040_5060_7080;
        unsafe {
            asm!("
                .set noat
                LLD $5, 0($3)
                SCD $4, 0($3)
            ", in("$3") ptr, inout("$4") write_value => scd_status, out("$5") lld_value);
        }

        soft_assert_eq(lld_value, 0x89AB_CDEF_0123_4567, "LLD value before SCD")?;
        soft_assert_eq(scd_status, 1, "SCD success flag")?;
        soft_assert_eq(memory, 0x1020_3040_5060_7080, "Memory after SCD")?;

        Ok(())
    }
}

pub struct SCAfterERET {}

impl Test for SCAfterERET {
    fn name(&self) -> &str {
        "SC after ERET fails and keeps LLAddr"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        unsafe {
            cop0::set_status(Status::ADDRESSING_MODE_64_BIT);
        }

        let mut memory: u32 = 0x89AB_CDEF;
        let ptr = (&mut memory as *mut u32) as isize as u64;

        let ll_value: u64;
        let sc_status: u64;
        unsafe {
            asm!("
                .set noat
                .set noreorder
                LL $5, 0($3)
                MFC0 $6, $12
                ORI $6, $6, 0X2
                MTC0 $6, $12
                NOP
                NOP
                DLA $7, 1f
                DMTC0 $7, $14
                NOP
                NOP
                NOP
                ERET
            1:
                LUI $4, 0x1357
                ORI $4, $4, 0x9BDF
                SC $4, 0($3)
            ", in("$3") ptr, out("$4") sc_status, out("$5") ll_value,
               out("$6") _, out("$7") _);
        }

        soft_assert_eq(ll_value, 0xFFFF_FFFF_89AB_CDEF, "LL value before ERET")?;
        soft_assert_eq(sc_status, 0, "SC success flag after ERET")?;
        soft_assert_eq(memory, 0x89AB_CDEF, "Memory after SC following ERET")?;
        soft_assert_neq(
            cop0::lladdr(),
            0,
            "LLAddr after ERET and SC must remain set",
        )?;

        Ok(())
    }
}

pub struct SCDAfterERET {}

impl Test for SCDAfterERET {
    fn name(&self) -> &str {
        "SCD after ERET fails and keeps LLAddr"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        unsafe {
            cop0::set_status(Status::ADDRESSING_MODE_64_BIT);
        }

        let mut memory: u64 = 0x89AB_CDEF_0123_4567;
        let ptr = (&mut memory as *mut u64) as isize as u64;

        let lld_value: u64;
        let scd_status: u64;
        unsafe {
            asm!("
                .set noat
                .set noreorder
                LLD $5, 0($3)
                MFC0 $6, $12
                ORI $6, $6, 0x2
                MTC0 $6, $12
                NOP
                NOP
                DLA $7, 1f
                DMTC0 $7, $14
                NOP
                NOP
                NOP
                ERET
            1:
                LUI $4, 0x1020
                ORI $4, $4, 0x3040
                DSLL32 $4, $4, 0
                ORI $4, $4, 0x5060
                DSLL $4, $4, 16
                ORI $4, $4, 0x7080
                SCD $4, 0($3)
            ", in("$3") ptr, out("$4") scd_status, out("$5") lld_value,
               out("$6") _, out("$7") _);
        }

        soft_assert_eq(lld_value, 0x89AB_CDEF_0123_4567, "LLD value before ERET")?;
        soft_assert_eq(scd_status, 0, "SCD success flag after ERET")?;
        soft_assert_eq(
            memory,
            0x89AB_CDEF_0123_4567,
            "Memory after SCD following ERET",
        )?;
        soft_assert_neq(
            cop0::lladdr(),
            0,
            "LLAddr after ERET and SCD must remain set",
        )?;

        Ok(())
    }
}

pub struct SCIgnoresLLAddr {}

impl Test for SCIgnoresLLAddr {
    fn name(&self) -> &str {
        "LL/SC: LLAddr is observational; SC gates on LLBit only"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        unsafe {
            cop0::set_status(Status::DEFAULT);
        }

        let mut data = UncachedHeapMemory::<u32>::new_with_align(1024, 4096);
        data.write(0, 0x89AB_CDEF); // LL target (offset 0)
        data.write(0x10, 0xFFFF_FFFF); // SC target (offset 0x40 - a different cache line)
        let physical = data.start_physical() as u64;
        let pfn = (physical >> 12) as u32;

        // Two virtual aliases of the same physical page. LL goes through one, SC through the other
        // but at a different line, so they resolve to different physical addresses.
        let ll_virtual: u32 = 0x0DEA_0000;
        let sc_virtual: u32 = 0x0BEE_0000;
        // LLAddr is the LL's physical address (offset 0), cache-line granular.
        let expected_lladdr = (physical >> 4) & 0x1FF_FFFF;

        unsafe {
            cop0::clear_tlb();
            cop0::set_context_64(0);
            cop0::set_xcontext_64(0);
            cop0::write_tlb(
                10,
                Pagemask::M4K,
                make_entry_lo(true, true, true, Coherency::Cached, pfn),
                make_entry_lo(true, false, false, Coherency::Cached, 0),
                make_entry_hi(0, u27::new(ll_virtual >> 13), u2::new(0)),
            );
            cop0::write_tlb(
                11,
                Pagemask::M4K,
                make_entry_lo(true, true, true, Coherency::Cached, pfn),
                make_entry_lo(true, false, false, Coherency::Cached, 0),
                make_entry_hi(0, u27::new(sc_virtual >> 13), u2::new(0)),
            );
            cop0::set_entry_hi(0);
        }

        let ll_value: u32;
        let sc_status: u32;
        let sc_readback: u32;
        let ll_readback: u32;
        unsafe {
            asm!("
                .set noat
                LL $5, 0($3)
                LUI $6, 0x1357
                ORI $6, $6, 0x9BDF
                SC $6, 0x40($4)
                LW $7, 0x40($4)
                LW $8, 0($3)
            ",
            in("$3") ll_virtual, in("$4") sc_virtual,
            out("$5") ll_value, out("$6") sc_status,
            out("$7") sc_readback, out("$8") ll_readback);
        }

        soft_assert_eq(ll_value, 0x89AB_CDEF, "LL value")?;
        // SC succeeds even though its address is a different physical line than the LL: SC gates on
        // the LLBit alone and never compares against LLAddr.
        soft_assert_eq(sc_status, 1, "SC succeeds to a different line than the LL")?;
        soft_assert_eq(sc_readback, 0x1357_9BDF, "SC stored to its own address")?;
        soft_assert_eq(ll_readback, 0x89AB_CDEF, "SC left the LL line untouched")?;
        soft_assert_eq(
            cop0::lladdr(),
            expected_lladdr,
            "LLAddr tracks the LL physical line (observational only)",
        )?;

        unsafe {
            cop0::clear_tlb();
        }
        Ok(())
    }
}
