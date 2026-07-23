use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use crate::assembler::{Assembler, GPR};
use crate::memory_map::MemoryMap;
use crate::tests::soft_asserts::soft_assert_eq2;
use crate::tests::{Level, Test};
use crate::uncached_memory::UncachedHeapMemory;

// Tests around the interlocking of MULT/DIV (and friends) with MFLO/MFHI:
// - Reading HI/LO right after starting a multiply/divide (no padding NOPs) must still return the
//   correct result: the VR4300 stalls MFLO/MFHI until the operation completes.
// - The reverse direction is the interesting hazard: on the R4000, an MFLO/MFHI followed too
//   closely by a new MULT/DIV makes the destination register of the MFLO/MFHI undefined.

const OPERAND_A: u64 = 0xC001_CAFE_9ABC_DEF0; // negative as i64/i32
const OPERAND_B: u64 = 0x0000_0000_7654_3210; // positive, non-zero for the divides

/// Runs `code` (which must end on JR RA + NOP) with OPERAND_A in $2 and OPERAND_B in $3.
/// Returns ($4, $5) after the call. HI and LO are preserved around the call.
fn run_fragment(code: &[u32]) -> (u64, u64) {
    let mut buffer = UncachedHeapMemory::<u32>::new_with_align(code.len(), 64);
    for (i, &word) in code.iter().enumerate() {
        buffer.write(i, word);
    }
    let target = MemoryMap::physical_to_cached::<u8>(buffer.start_physical()) as usize;

    let out_a: u64;
    let out_b: u64;
    unsafe {
        asm!("
            .set noat
            .set noreorder
            ORI $20, $31, 0  // stash RA
            MFLO $21
            MFHI $22
            JALR {target}
            NOP
            ORI $31, $20, 0  // restore RA
            MTLO $21
            MTHI $22
        ",
        target = in(reg) target,
        in("$2") OPERAND_A,
        in("$3") OPERAND_B,
        out("$4") out_a,
        out("$5") out_b,
        out("$20") _,
        out("$21") _,
        out("$22") _,
        options(nostack));
    }
    (out_a, out_b)
}

/// The 8 multiply/divide instructions, operating on $2 (rs) and $3 (rt).
const OPS: &[(&str, u32)] = &[
    ("MULT", Assembler::make_mult(GPR::V1, GPR::V0)),
    ("MULTU", Assembler::make_multu(GPR::V1, GPR::V0)),
    ("DMULT", Assembler::make_dmult(GPR::V1, GPR::V0)),
    ("DMULTU", Assembler::make_dmultu(GPR::V1, GPR::V0)),
    ("DIV", Assembler::make_div(GPR::V1, GPR::V0)),
    ("DIVU", Assembler::make_divu(GPR::V1, GPR::V0)),
    ("DDIV", Assembler::make_ddiv(GPR::V1, GPR::V0)),
    ("DDIVU", Assembler::make_ddivu(GPR::V1, GPR::V0)),
];

/// Builds: op; <gap NOPs>; MFLO $4 / MFHI $5 (or the other way around); JR RA; NOP
fn read_after_op_fragment(op: u32, gap: usize, hi_first: bool) -> Vec<u32> {
    let mut code = Vec::new();
    code.push(op);
    for _ in 0..gap {
        code.push(Assembler::make_nop());
    }
    if hi_first {
        code.push(Assembler::make_mfhi(GPR::A1));
        code.push(Assembler::make_mflo(GPR::A0));
    } else {
        code.push(Assembler::make_mflo(GPR::A0));
        code.push(Assembler::make_mfhi(GPR::A1));
    }
    code.push(Assembler::make_jr(GPR::RA));
    code.push(Assembler::make_nop());
    code
}

/// MFLO/MFHI directly after MULT/DIV etc. must return the completed result, no matter how few
/// instructions are in between.
pub struct ReadAfterOpWithoutGap {}

impl Test for ReadAfterOpWithoutGap {
    fn name(&self) -> &str {
        "HI/LO: MFLO/MFHI right after mult/div"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        let mut result: Vec<Box<dyn Any>> = Vec::new();
        for &(name, op) in OPS {
            for gap in 0..3usize {
                for hi_first in [false, true] {
                    result.push(Box::new((name, op, gap, hi_first)));
                }
            }
        }
        result
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(&str, u32, usize, bool)>() {
            Some((name, op, gap, hi_first)) => {
                // Reference: same operation with a gap that safely covers any latency
                let (expected_lo, expected_hi) =
                    run_fragment(&read_after_op_fragment(*op, 80, *hi_first));
                let (lo, hi) = run_fragment(&read_after_op_fragment(*op, *gap, *hi_first));
                soft_assert_eq2(lo, expected_lo, || {
                    format!("LO of {} read {} instructions after it started", name, gap)
                })?;
                soft_assert_eq2(hi, expected_hi, || {
                    format!("HI of {} read {} instructions after it started", name, gap)
                })?;
                Ok(())
            }
            _ => Err(format!("Unexpected pattern")),
        }
    }
}

/// The reverse direction: MFLO/MFHI followed closely by a new MULT/DIV. On the R4000 this makes
/// the MFLO/MFHI destination undefined; this test documents what the VR4300 does.
pub struct ReadThenStartOp {}

impl Test for ReadThenStartOp {
    fn name(&self) -> &str {
        "HI/LO: mult/div right after MFLO/MFHI"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        let mut result: Vec<Box<dyn Any>> = Vec::new();
        for &(name, op) in OPS {
            for gap in 0..3usize {
                result.push(Box::new((name, op, gap)));
            }
        }
        result
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(&str, u32, usize)>() {
            Some((name, op, gap)) => {
                // Set up known HI/LO contents, read them back with a new mult/div breathing down
                // the MFLO/MFHI's neck, then let the new operation finish and drain it.
                let mut code = Vec::new();
                code.push(Assembler::make_mtlo(GPR::V0)); // LO = OPERAND_A
                code.push(Assembler::make_mthi(GPR::V1)); // HI = OPERAND_B
                for _ in 0..4 {
                    code.push(Assembler::make_nop());
                }
                code.push(Assembler::make_mflo(GPR::A0));
                code.push(Assembler::make_mfhi(GPR::A1));
                for _ in 0..*gap {
                    code.push(Assembler::make_nop());
                }
                code.push(*op);
                // Give the operation time to finish before returning
                for _ in 0..80 {
                    code.push(Assembler::make_nop());
                }
                code.push(Assembler::make_jr(GPR::RA));
                code.push(Assembler::make_nop());

                let (lo, hi) = run_fragment(&code);
                soft_assert_eq2(lo, OPERAND_A, || {
                    format!(
                        "MFLO result with {} started {} instructions after it",
                        name, gap
                    )
                })?;
                soft_assert_eq2(hi, OPERAND_B, || {
                    format!(
                        "MFHI result with {} started {} instructions after it",
                        name, gap
                    )
                })?;
                Ok(())
            }
            _ => Err(format!("Unexpected pattern")),
        }
    }
}
