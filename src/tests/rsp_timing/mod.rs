use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;

use arbitrary_int::{u24, u5};
use num_traits::ops::wrapping::WrappingSub;

use crate::cop0::count;
use crate::rdp::rdp::{DP_SET_STATUS_CLEAR_FREEZE, DP_SET_STATUS_SET_FREEZE, RDP};
use crate::rsp::rsp::RSP;
use crate::rsp::rsp_assembler::{CP0Register, RSPAssembler, GPR};
use crate::rsp::spmem::SPMEM;
use crate::tests::soft_asserts::{
    soft_assert_eq, soft_assert_eq_with_epsilon, soft_assert_less_or_equal, soft_assert_neq,
};
use crate::tests::{Level, Test};

fn early_return_if_rdp_clock_is_missing() -> Result<(), String> {
    let a = RDP::clock_32();
    let b = RDP::clock_32();

    soft_assert_neq(
        a,
        b,
        "RDP clock is not advancing. Returning early, but this is a fail",
    )?;

    Ok(())
}

pub struct ClockCPUvsRSP {}

impl Test for ClockCPUvsRSP {
    fn name(&self) -> &str {
        "RSP Timing: Clock CPU vs RDP"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const CPU_CLOCK_CYCLES: u32 = 100000;
        const EXPECTED_RDP_CYCLES: u24 = u24::new(CPU_CLOCK_CYCLES * 4 / 3);
        const RDP_CYCLE_EPSILON: u24 = u24::new(20);

        // Freeze doesn't matter. Set it to avoid misconceptions
        unsafe {
            RDP::set_status(DP_SET_STATUS_SET_FREEZE);
        }
        let cpu_clock_before = count();
        let rdp_clock_before = RDP::clock();
        let (cpu_clock_target, cpu_overflow) = cpu_clock_before.overflowing_add(CPU_CLOCK_CYCLES);

        let rdp_clock_after = loop {
            let cpu_clock = count();
            let rdp_clock = RDP::clock();
            if (!cpu_overflow || cpu_clock < cpu_clock_before) && cpu_clock >= cpu_clock_target {
                break rdp_clock;
            }
        };

        // Let's not leave it frozen
        unsafe {
            RDP::set_status(DP_SET_STATUS_CLEAR_FREEZE);
        }

        soft_assert_eq_with_epsilon(
            RDP_CYCLE_EPSILON,
            rdp_clock_after.wrapping_sub(&rdp_clock_before),
            EXPECTED_RDP_CYCLES,
            format!(
                "Expected RDP cycles that pass during {} cpu cycles",
                CPU_CLOCK_CYCLES
            )
            .as_str(),
        )?;

        Ok(())
    }
}

pub struct ClockFoolishlyAttemptToWrite {}

impl Test for ClockFoolishlyAttemptToWrite {
    fn name(&self) -> &str {
        "RSP Timing: Clock must be readonly"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let clock_before = RDP::clock();

        RDP::set_clock_32((clock_before - u24::new(0x0F_FFFF)).value());

        let clock_after = RDP::clock();

        soft_assert_less_or_equal(
            clock_after - clock_before,
            u24::new(100),
            "RDP Clock should be write-only",
        )?;

        Ok(())
    }
}

pub struct ClockIsMasked {}

impl Test for ClockIsMasked {
    fn name(&self) -> &str {
        "RSP Timing: Clock is just 24 bit"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        early_return_if_rdp_clock_is_missing()?;

        let mut clock = RDP::clock_32();

        soft_assert_eq(
            u24::extract_u32(clock, 0).value(),
            clock,
            "Clock is just 24 bit",
        )?;

        loop {
            let next_clock = RDP::clock_32();

            // Overflowed in u24?
            if u24::extract_u32(next_clock, 0) < u24::extract_u32(clock, 0) {
                // Make sure the upper bits are still 0
                soft_assert_eq(
                    u24::extract_u32(next_clock, 0).value(),
                    next_clock,
                    "Clock is just 24 bit",
                )?;
                break;
            }

            clock = next_clock;
        }

        Ok(())
    }
}

fn measure(f: &dyn Fn(&mut RSPAssembler)) -> u32 {
    let mut assembler = RSPAssembler::new(0);
    assembler.write_mfc0(CP0Register::DPClock, GPR::K0);

    assembler.write_nop();
    assembler.write_nop();
    assembler.write_nop();
    assembler.write_nop();

    f(&mut assembler);

    assembler.write_nop();
    assembler.write_nop();
    assembler.write_nop();
    assembler.write_nop();

    assembler.write_mfc0(CP0Register::DPClock, GPR::K1);

    assembler.write_sw(GPR::K0, GPR::R0, 0xFF8);
    assembler.write_sw(GPR::K1, GPR::R0, 0xFFC);

    assembler.write_break();

    RSP::run_and_wait(0);

    let reported_delta = u24::new(SPMEM::read(0xFFC)).wrapping_sub(&u24::new(SPMEM::read(0xFF8)));

    // We put in 8 nops. 1 extra instruction (one of the two mfc0).
    reported_delta.value() - 9
}

pub struct CyclesSLL {}

impl Test for CyclesSLL {
    fn name(&self) -> &str {
        "RSP Timing: SLL"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        soft_assert_eq(
            measure(&|assembler| assembler.write_sll(GPR::A0, GPR::A1, u5::new(5))),
            1,
            "Expected cycle count",
        )?;
        Ok(())
    }
}
