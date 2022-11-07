use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;
use core::mem::transmute;

use crate::assembler::{Assembler, Cop1Condition, FR, GPR};
use crate::cop0::{count, RegisterIndex, set_count};
use crate::memory_map::MemoryMap;
use crate::tests::{Level, Test};
use crate::tests::soft_asserts::{soft_assert_eq, soft_assert_eq_decimal, soft_assert_range};
use crate::uncached_memory::{UncachedHeapMemory, UncachedHeapMemoryWriter};

// TODO: COP1 data dependencies (one instruction using the output of another).
// TODO: Try again to find data dependencies in integer land
// TODO: For COP1 there can be data dependencies (ADD.S that uses a register that was just calculated is slower). How's that with .D?


/// This test repeatedly reads COP0.Count and looks at the differences.
/// It expects count to increase on every other call, so the differences should be 1, 0, 1, 0, 1...
pub struct RepeatedMFC0Count {

}

impl Test for RepeatedMFC0Count {
    fn name(&self) -> &str { "Timing: Repeated MFC0 COUNT" }

    fn level(&self) -> Level { Level::Timing }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        fn test_function(output: &mut [[u32; 8]; 10]) {
            unsafe {
                asm!("
            .set noat
1:
            MFC0 {c0}, ${cop0reg}
            MFC0 {c1}, ${cop0reg}
            MFC0 {c2}, ${cop0reg}
            MFC0 {c3}, ${cop0reg}
            MFC0 {c4}, ${cop0reg}
            MFC0 {c5}, ${cop0reg}
            MFC0 {c6}, ${cop0reg}
            MFC0 {c7}, ${cop0reg}
            MFC0 {c8}, ${cop0reg}
            MFC0 {c9}, ${cop0reg}

            SUB {temp}, {c1}, {c0}
            SW {temp}, 0*4 ({target})
            SUB {temp}, {c2}, {c1}
            SW {temp}, 1*4 ({target})
            SUB {temp}, {c3}, {c2}
            SW {temp}, 2*4 ({target})
            SUB {temp}, {c4}, {c3}
            SW {temp}, 3*4 ({target})
            SUB {temp}, {c5}, {c4}
            SW {temp}, 4*4 ({target})
            SUB {temp}, {c6}, {c5}
            SW {temp}, 5*4 ({target})
            SUB {temp}, {c7}, {c6}
            SW {temp}, 6*4 ({target})
            SUB {temp}, {c8}, {c7}
            SW {temp}, 7*4 ({target})
            SUB {temp}, {c9}, {c8}
            SW {temp}, 8*4 ({target})

            ADD {target}, {target}, 8*4  // add stride
            SUB {counter}, {counter}, 1
            BNE {counter}, $0, 1b
            NOP  // delay
        ",
                c0 = out(reg) _,
                c1 = out(reg) _,
                c2 = out(reg) _,
                c3 = out(reg) _,
                c4 = out(reg) _,
                c5 = out(reg) _,
                c6 = out(reg) _,
                c7 = out(reg) _,
                c8 = out(reg) _,
                c9 = out(reg) _,
                temp = out(reg) _,
                target = inout(reg) output => _,
                counter = inout(reg) output.len() => _,
                cop0reg = const RegisterIndex::Count as usize);
            }
        }

        let mut differences: [[u32; 8]; 10] = Default::default();

        test_function(&mut differences);

        // Ignore the first iteration as it includes ICACHE warmup
        // After that, each row has iterating 0 and 1
        for i in 1..differences.len() {
            let row = &differences[i];
            // Expect 1s and 1s, but accept both orders
            if row[0] == 0 {
                soft_assert_eq(row, &[0, 1, 0, 1, 0, 1, 0, 1], "Expected iterating 0s and 1s")?;
            } else {
                soft_assert_eq(row, &[1, 0, 1, 0, 1, 0, 1, 0], "Expected iterating 0s and 1s")?;
            }
        }

        Ok(())
    }
}

/// Writing to COUNT should set cycles precisely, eliminating any half-cycles that might be present.
/// This will be used in the measure function below, so have a test specifically for it
pub struct HalfCycleExactCalibration {

}

impl Test for HalfCycleExactCalibration {
    fn name(&self) -> &str { "Timing: Half cycle calibration" }

    fn level(&self) -> Level { Level::Timing }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        for count_value in [0u32, 100, 0x1234, 0x8000000, 0xFFFFFFFC, 0xFFFFFFFF] {
            let backup_count = count();
            let out0: u32;
            let out1: u32;
            let out2: u32;
            let out3: u32;
            unsafe {
                asm!("
                    .set noat
1:
                    MTC0 {count_value}, ${COUNT}

                    // This is not a hazard test (cop0 has that), so put two NOPs here so that the
                    // COP0 gets some time to figure itself out
                    NOP
                    NOP
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

            unsafe { set_count(backup_count); }

            // The hazard test tests the specific values we're getting back, which are more difficult
            // to reproduce. Here we just care about the deltas

            soft_assert_eq(out1 - out0, 1, "2nd - 1st readback")?;
            soft_assert_eq(out2 - out1, 0, "3rd - 2nd readback")?;
            soft_assert_eq(out3 - out2, 1, "4th - 3rd readback")?;
        }
        Ok(())
    }
}

/// Runs the passed in naked function and measures its runtime precisely.
/// The function is expected to end on JR RA with a NOP in the delay slot, but is otherwise
/// free to do anything. Registers $2-$18 are available to the test function
fn assert_cycles(expected_cycles: u32, f: extern "C" fn()) -> Result<(), String> {
    let ticks1: u32;
    let ticks2: u32;
    unsafe {
        // A couple of issues this test harness fixes:
        // - ICACHE: The first time the test runs, it is freshly loaded from ROM, introducing
        //   delays. Solve this by running twice
        // - COUNT increments at half-intervals, so when we're measuring cycles, half-cycles
        //   (e.g. 1.5) can be rounded up or down. Solution: Write to COUNT, as that
        //   calibrates.
        // - In order to measure half-cycles we can then add an additional NOP in another run.
        asm!("
            .set noat
            .set noreorder
            ORI $20, $31, 0  // stash RA
            ORI $22, $0, 3   // run three times
1:
            // Prepare some register values:
            // - 1: 0x00000000_00000000
            // - 2: 0x00000000_00000123
            // - 3: 0xFEDCBA09_87654321
            // - 4: 0x00000000_00000125
            // - F0: 123.0 (single)
            // - F2: 125.0 (single)
            // - F14: 123 (word)
            // - F16: 123.0 (double)
            // - F18: 125.0 (double)
            // - F30: 125 (long)

            ORI $1, $0, 0x0

            ORI $2, $0, 123
            ORI $4, $0, 125

            LUI $3, 0xFEDC
            ORI $3, 0xBA98
            DSLL $3, $3, 16
            ORI $3, 0x7654
            DSLL $3, $3, 16
            ORI $3, 0x3210

            MTC1 $2, $14
            DMTC1 $4, $30
            CVT.S.W $0, $14
            CVT.S.L $2, $30
            CVT.D.W $16, $14
            CVT.D.L $18, $30
            NOP; NOP; NOP; NOP;


            // Calibration step
            MFC0 $19, ${COUNT}
            NOP
            NOP
            MTC0 $19, ${COUNT}
            NOP

            // If loop counter is 2, skip an instruction. This shifts half of a cycle compared to the last round
            ORI $19, $0, 2
            BEQ $22, $19, 2f
            NOP  // delay
            NOP  // an extra instruction that is skipped if $22 == 2
2:
            MFC0 $19, ${COUNT}
            JALR $25
            NOP
            MFC0 $21, ${COUNT}
            ORI $23, $24, 0     // stash previous iteration's result in 23
            SUB $24, $21, $19
            ADDIU $22, $22, 0xFFFF
            BNE $22, $0, 1b
            NOP // delay slot
            ORI $31, $20, 0  // restore RA
        ",
        COUNT = const RegisterIndex::Count as usize,

        // Free these up for the test function
        out("$2") _,
        out("$3") _,
        out("$4") _,
        out("$5") _,
        out("$6") _,
        out("$7") _,
        out("$8") _,
        out("$9") _,
        out("$10") _,
        out("$11") _,
        out("$12") _,
        out("$13") _,
        out("$14") _,
        out("$15") _,
        out("$16") _,
        out("$17") _,
        out("$18") _,

        // These are for the test infra itself
        out("$19") _,
        out("$20") _,
        out("$21") _,
        out("$22") _,
        out("$23") ticks2,
        out("$24") ticks1,
        in("$25") f);
    }

    // ticks2 is either the same or 1 larger than ticks
    soft_assert_range(ticks2, ticks1, ticks1 + 1, "Runtime with COUNT starting at half a cycle")?;

    // In addition to the code being measured, we need a few extra cycles:
    // - 1 for the JALR
    // - 1 for the JALR's NOP
    // - 1 for the return JR RA
    // - 1 for the delay of the JR RA
    // - 1 for one of the MFC0 COUNT itself
    soft_assert_eq_decimal((ticks1 + ticks2) - 5, expected_cycles, "Measured cycles")?;

    Ok(())
}

fn assert_cycles_with_codegen<F: FnOnce(&mut UncachedHeapMemoryWriter<u32>)>(expected_cycles: u32, f: F) -> Result<(), String> {
    // Dynamically generate code
    let mut code_memory = UncachedHeapMemory::<u32>::new_with_align(64, 64);
    let mut writer = UncachedHeapMemoryWriter::new(&mut code_memory);

    f(&mut writer);
    writer.write(Assembler::make_jr(GPR::RA));
    writer.write(Assembler::make_nop());   // delay slot

    // To execute, we'll run from a 0x8xxxxxxx address as otherwise the CPU will mostly
    // spend time reading from memory
    let cached_ptr = MemoryMap::physical_to_cached::<u8>(code_memory.start_phyiscal());

    // Turn the pointer into a function pointer
    let function_ptr: extern "C" fn() = unsafe { transmute(cached_ptr) };

    assert_cycles(expected_cycles, function_ptr)
}

fn assert_cycles_with_codegen_one_instruction(expected_cycles: u32, instruction: u32) -> Result<(), String> {
    assert_cycles_with_codegen(expected_cycles, |writer| writer.write(instruction))
}

/// Assemble a test program that just has a few NOPs
pub struct PreciseMeasureJustNOPs {

}

impl Test for PreciseMeasureJustNOPs {
    fn name(&self) -> &str { "Timing: Just NOPs" }

    fn level(&self) -> Level { Level::Timing }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            Box::new(0u32),
            Box::new(1u32),
            Box::new(2u32),
            Box::new(3u32),
            Box::new(6u32),
            Box::new(11u32),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<u32>() {
            Some(number_of_nops) => {
                assert_cycles_with_codegen(*number_of_nops, |writer| {
                    for _ in 0..*number_of_nops {
                        writer.write(Assembler::make_nop());
                    }
                })
            }
            _ => Err(format!("Unexpected pattern"))
        }
    }
}

pub struct SingleInstructionCPUTiming {

}

impl Test for SingleInstructionCPUTiming {
    fn name(&self) -> &str { "Timing: Individual instructions (CPU)" }

    fn level(&self) -> Level { Level::Timing }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            // Expensive CPU instructions
            Box::new(("DIV", 37u32, Assembler::make_div(GPR::R0, GPR::R0))),
            Box::new(("DIVU", 37u32, Assembler::make_divu(GPR::R0, GPR::R0))),
            Box::new(("DDIV", 69u32, Assembler::make_ddiv(GPR::R0, GPR::R0))),
            Box::new(("DDIVU", 69u32, Assembler::make_ddivu(GPR::R0, GPR::R0))),

            Box::new(("MULT", 5u32, Assembler::make_mult(GPR::R0, GPR::R0))),
            Box::new(("MULTU", 5u32, Assembler::make_multu(GPR::R0, GPR::R0))),
            Box::new(("DMULT", 8u32, Assembler::make_dmult(GPR::R0, GPR::R0))),
            Box::new(("DMULTU", 8u32, Assembler::make_dmultu(GPR::R0, GPR::R0))),

            Box::new(("MFLO", 1u32, Assembler::make_mflo(GPR::V0))),
            Box::new(("MFHI", 1u32, Assembler::make_mfhi(GPR::V0))),
            Box::new(("MTLO", 1u32, Assembler::make_mtlo(GPR::V0))),
            Box::new(("MTHI", 1u32, Assembler::make_mthi(GPR::V0))),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(&str, u32, u32)>() {
            Some((_context, expected_cycles, instruction)) => {
                assert_cycles_with_codegen_one_instruction(*expected_cycles, *instruction)
            }
            _ => Err(format!("Unexpected pattern"))
        }
    }
}

pub struct SingleInstructionCOP1Timing {

}

impl Test for SingleInstructionCOP1Timing {
    fn name(&self) -> &str { "Timing: Individual instructions (COP1)" }

    fn level(&self) -> Level { Level::Timing }

    fn values(&self) -> Vec<Box<dyn Any>> {
        const OUTPUT: FR = FR::F14;

        const SINGLE_INPUT: FR = FR::F0;
        const SINGLE_INPUT2: FR = FR::F2;
        const WORD_INPUT: FR = FR::F14;
        const DOUBLE_INPUT: FR = FR::F16;
        const DOUBLE_INPUT2: FR = FR::F18;
        const LONG_INPUT: FR = FR::F30;

        vec! {
            // COP1 with input Single
            Box::new(("ADD.S", 3u32, Assembler::make_cop1_add(OUTPUT, SINGLE_INPUT, SINGLE_INPUT).s())),
            Box::new(("SUB.S", 3u32, Assembler::make_cop1_sub(OUTPUT, SINGLE_INPUT, SINGLE_INPUT).s())),
            Box::new(("MUL.S", 5u32, Assembler::make_cop1_mul(OUTPUT, SINGLE_INPUT, SINGLE_INPUT).s())),
            Box::new(("DIV.S", 29u32, Assembler::make_cop1_div(OUTPUT, SINGLE_INPUT, SINGLE_INPUT).s())),
            Box::new(("NEG.S", 1u32, Assembler::make_cop1_neg(OUTPUT, SINGLE_INPUT).s())),
            Box::new(("ABS.S", 1u32, Assembler::make_cop1_abs(OUTPUT, SINGLE_INPUT).s())),
            Box::new(("SQRT.S", 29u32, Assembler::make_cop1_sqrt(OUTPUT, SINGLE_INPUT).s())),
            Box::new(("MOV.S", 1u32, Assembler::make_cop1_mov(OUTPUT, SINGLE_INPUT).s())),

            Box::new(("ROUND.W.S", 5u32, Assembler::make_cop1_round_w(OUTPUT, SINGLE_INPUT).s())),
            Box::new(("TRUNC.W.S", 5u32, Assembler::make_cop1_trunc_w(OUTPUT, SINGLE_INPUT).s())),
            Box::new(("CEIL.W.S", 5u32, Assembler::make_cop1_ceil_w(OUTPUT, SINGLE_INPUT).s())),
            Box::new(("FLOOR.W.S", 5u32, Assembler::make_cop1_floor_w(OUTPUT, SINGLE_INPUT).s())),
            Box::new(("CVT.W.S", 5u32, Assembler::make_cop1_cvt_w(OUTPUT, SINGLE_INPUT).s())),

            Box::new(("ROUND.L.S", 5u32, Assembler::make_cop1_round_l(OUTPUT, SINGLE_INPUT).s())),
            Box::new(("TRUNC.L.S", 5u32, Assembler::make_cop1_trunc_l(OUTPUT, SINGLE_INPUT).s())),
            Box::new(("CEIL.L.S", 5u32, Assembler::make_cop1_ceil_l(OUTPUT, SINGLE_INPUT).s())),
            Box::new(("FLOOR.L.S", 5u32, Assembler::make_cop1_floor_l(OUTPUT, SINGLE_INPUT).s())),
            Box::new(("CVT.L.S", 5u32, Assembler::make_cop1_cvt_l(OUTPUT, SINGLE_INPUT).s())),

            Box::new(("CVT.D.S", 1u32, Assembler::make_cop1_cvt_d(OUTPUT, SINGLE_INPUT).s())),

            Box::new(("C.F", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::F, SINGLE_INPUT, SINGLE_INPUT).s())),
            Box::new(("C.F", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::F, SINGLE_INPUT, SINGLE_INPUT2).s())),
            Box::new(("C.EQ", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::EQ, SINGLE_INPUT, SINGLE_INPUT2).s())),
            Box::new(("C.EQ", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::EQ, SINGLE_INPUT, SINGLE_INPUT).s())),
            Box::new(("C.NGLE", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::NGLE, SINGLE_INPUT, SINGLE_INPUT2).s())),
            Box::new(("C.NGLE", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::NGLE, SINGLE_INPUT, SINGLE_INPUT).s())),

            // COP1 with input Double
            Box::new(("ADD.D", 3u32, Assembler::make_cop1_add(OUTPUT, DOUBLE_INPUT, DOUBLE_INPUT).d())),
            Box::new(("SUB.D", 3u32, Assembler::make_cop1_sub(OUTPUT, DOUBLE_INPUT, DOUBLE_INPUT).d())),
            Box::new(("MUL.D", 8u32, Assembler::make_cop1_mul(OUTPUT, DOUBLE_INPUT, DOUBLE_INPUT).d())),
            Box::new(("DIV.D", 58u32, Assembler::make_cop1_div(OUTPUT, DOUBLE_INPUT, DOUBLE_INPUT).d())),
            Box::new(("NEG.D", 1u32, Assembler::make_cop1_neg(OUTPUT, DOUBLE_INPUT).d())),
            Box::new(("ABS.D", 1u32, Assembler::make_cop1_abs(OUTPUT, DOUBLE_INPUT).d())),
            Box::new(("SQRT.D", 58u32, Assembler::make_cop1_sqrt(OUTPUT, DOUBLE_INPUT).d())),
            Box::new(("MOV.D", 1u32, Assembler::make_cop1_mov(OUTPUT, DOUBLE_INPUT).d())),

            Box::new(("ROUND.W.D", 5u32, Assembler::make_cop1_round_w(OUTPUT, DOUBLE_INPUT).d())),
            Box::new(("TRUNC.W.D", 5u32, Assembler::make_cop1_trunc_w(OUTPUT, DOUBLE_INPUT).d())),
            Box::new(("CEIL.W.D", 5u32, Assembler::make_cop1_ceil_w(OUTPUT, DOUBLE_INPUT).d())),
            Box::new(("FLOOR.W.D", 5u32, Assembler::make_cop1_floor_w(OUTPUT, DOUBLE_INPUT).d())),
            Box::new(("CVT.W.D", 5u32, Assembler::make_cop1_cvt_w(OUTPUT, DOUBLE_INPUT).d())),

            Box::new(("ROUND.L.D", 5u32, Assembler::make_cop1_round_l(OUTPUT, DOUBLE_INPUT).d())),
            Box::new(("TRUNC.L.D", 5u32, Assembler::make_cop1_trunc_l(OUTPUT, DOUBLE_INPUT).d())),
            Box::new(("CEIL.L.D", 5u32, Assembler::make_cop1_ceil_l(OUTPUT, DOUBLE_INPUT).d())),
            Box::new(("FLOOR.L.D", 5u32, Assembler::make_cop1_floor_l(OUTPUT, DOUBLE_INPUT).d())),
            Box::new(("CVT.L.D", 5u32, Assembler::make_cop1_cvt_l(OUTPUT, DOUBLE_INPUT).d())),

            Box::new(("CVT.S.D", 2u32, Assembler::make_cop1_cvt_s(OUTPUT, DOUBLE_INPUT).d())),

            Box::new(("C.F", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::F, DOUBLE_INPUT, DOUBLE_INPUT).d())),
            Box::new(("C.F", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::F, DOUBLE_INPUT, DOUBLE_INPUT2).d())),
            Box::new(("C.EQ", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::EQ, DOUBLE_INPUT, DOUBLE_INPUT2).d())),
            Box::new(("C.EQ", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::EQ, DOUBLE_INPUT, DOUBLE_INPUT).d())),
            Box::new(("C.NGLE", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::NGLE, DOUBLE_INPUT, DOUBLE_INPUT2).d())),
            Box::new(("C.NGLE", 1u32, Assembler::make_cop1_c_cond(Cop1Condition::NGLE, DOUBLE_INPUT, DOUBLE_INPUT).d())),

            // COP1 with input Word
            Box::new(("CVT.S.W", 5u32, Assembler::make_cop1_cvt_s(OUTPUT, WORD_INPUT).w())),
            Box::new(("CVT.D.W", 5u32, Assembler::make_cop1_cvt_d(OUTPUT, WORD_INPUT).w())),

            // COP1 with input Long
            Box::new(("CVT.S.L", 5u32, Assembler::make_cop1_cvt_s(OUTPUT, LONG_INPUT).l())),
            Box::new(("CVT.D.L", 5u32, Assembler::make_cop1_cvt_d(OUTPUT, LONG_INPUT).l())),

            // CPU <-> COP1
            Box::new(("MTC1", 1u32, Assembler::make_mtc1(GPR::V0, OUTPUT))),
            Box::new(("MFC1", 1u32, Assembler::make_mfc1(GPR::V0, FR::F0))),
            Box::new(("DMTC1", 1u32, Assembler::make_dmtc1(GPR::V0, OUTPUT))),
            Box::new(("DMFC1", 1u32, Assembler::make_dmfc1(GPR::V0, FR::F0))),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(&str, u32, u32)>() {
            Some((_context, expected_cycles, instruction)) => {
                assert_cycles_with_codegen_one_instruction(*expected_cycles, *instruction)
            }
            _ => Err(format!("Unexpected pattern"))
        }
    }
}

