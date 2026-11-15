use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use arbitrary_int::prelude::*;

use crate::cop0::RegisterIndex;
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

const REPEATS: usize = 2;
const DISCARDED_PASSES: u32 = 1;
const CPU_CLOCK_CYCLES: u32 = 100000;

fn sample_once() -> Result<(u24, u24), String> {
    let rdp_clock_address = RDP::clock_register_address();
    let loop_address: usize;
    let cpu_start: u32;
    let rdp_start: u32;
    let cpu_end: u32;
    let rdp_end: u32;

    unsafe {
        asm!("
            .set noat
            .set noreorder
            ORI   {pass}, $0, 0
            DLA   {loop_address}, 1f
            .balign 32
1:
            // These two stay adjacent, at the start of a cache line
            MFC0  {count}, ${COUNT}
            LW    {rdp}, 0({rdp_address})
 
            ADDIU {pass}, {pass}, 1
            ORI   {tmp}, $0, {START_PASS}
            BNE   {pass}, {tmp}, 2f
            NOP
            // Pass START_PASS: this is the start sample
            ADDU  {count_start}, {count}, $0
            ADDU  {rdp_start}, {rdp}, $0
            B     1b
            NOP
2:
            // Still discarding passes?
            SLTIU {tmp}, {pass}, {START_PASS}
            BNEZ  {tmp}, 1b
            NOP
            // Enough Count ticks since the start sample?
            SUBU  {diff}, {count}, {count_start}
            SLTU  {tmp}, {diff}, {target}
            BNEZ  {tmp}, 1b
            NOP
        ",
        COUNT = const RegisterIndex::Count as usize,
        START_PASS = const DISCARDED_PASSES + 1,
        rdp_address = in(reg) rdp_clock_address,
        target = in(reg) CPU_CLOCK_CYCLES,
        loop_address = out(reg) loop_address,
        pass = out(reg) _,
        tmp = out(reg) _,
        diff = out(reg) _,
        count = out(reg) cpu_end,
        rdp = out(reg) rdp_end,
        count_start = out(reg) cpu_start,
        rdp_start = out(reg) rdp_start);
    }

    // Self check: the guarantee above depends on the build honouring `.balign 32`
    if loop_address & 31 != 0 {
        return Err(format!(
            "Sampling loop isn't aligned to an icache line (address {:#x})",
            loop_address
        ));
    }

    // The expectation comes from the Count ticks that really passed between the two samples, so the
    // overshoot of the loop doesn't matter
    let ticks = cpu_end.wrapping_sub(cpu_start);
    let rdp_delta = u24::extract_u32(rdp_end, 0).wrapping_sub(u24::extract_u32(rdp_start, 0));
    Ok((rdp_delta, u24::new(ticks * 4 / 3)))
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
        const RDP_CYCLE_EPSILON: u24 = u24::new(20); // calibrate after looking at the errors below

        // Freeze doesn't matter. Set it to avoid misconceptions
        unsafe {
            RDP::set_status(DP_SET_STATUS_SET_FREEZE);
        }

        let mut results: Vec<(i32, u24, u24)> = Vec::with_capacity(REPEATS);
        let mut failure = None;
        for _ in 0..REPEATS {
            match sample_once() {
                Ok((delta, expected)) => results.push((
                    delta.value() as i32 - expected.value() as i32,
                    delta,
                    expected,
                )),
                Err(message) => {
                    failure = Some(message);
                    break;
                }
            }
        }

        // Let's not leave it frozen
        unsafe {
            RDP::set_status(DP_SET_STATUS_CLEAR_FREEZE);
        }
        if let Some(message) = failure {
            return Err(message);
        }

        results.sort_by_key(|result| result.0);
        // Shown in the message: identical errors in every repeat point to a fixed skew, errors that vary
        // with isolated large ones point to outliers
        let errors: Vec<i32> = results.iter().map(|result| result.0).collect();
        let (_, delta, expected) = results[REPEATS / 2];

        soft_assert_eq_with_epsilon(
            RDP_CYCLE_EPSILON,
            delta,
            expected,
            format!(
                "Expected RDP cycles that pass during {} count ticks (median of {} repeats, errors {:?})",
                CPU_CLOCK_CYCLES, REPEATS, errors
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

    let reported_delta = u24::new(SPMEM::read(0xFFC)).wrapping_sub(u24::new(SPMEM::read(0xFF8)));

    // We put in 8 nops. 1 extra instruction (one of the two mfc0).
    reported_delta.value() - 9
}

pub struct CyclesSLL {}

impl Test for CyclesSLL {
    fn name(&self) -> &str {
        "RSP Timing: SLL"
    }

    fn level(&self) -> Level {
        Level::Timing
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
