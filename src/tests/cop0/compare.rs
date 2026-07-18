use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use arbitrary_int::prelude::*;

use crate::assembler::{Assembler, GPR};
use crate::cop0::{cause, compare, count, preset_cause_to_copindex2, set_compare, RegisterIndex};
use crate::tests::soft_asserts::{soft_assert_eq, soft_assert_greater_or_equal};
use crate::tests::{Level, Test};

pub struct CompareInterruptSignalling;

impl Test for CompareInterruptSignalling {
    fn name(&self) -> &str {
        "Compare (signalling)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        // These numbers should be easily greater than the time the icache might need. We don't have
        // a actual worst case, but 500 should be fine in practice
        vec![Box::new(500u32), Box::new(2000u32), Box::new(30000u32)]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<u32>() {
            Some(offset) => {
                preset_cause_to_copindex2()?;
                let current = count();
                let target = current.wrapping_add(*offset);
                set_compare(target);
                soft_assert_eq(compare(), target, "Compare readback not the same")?;

                // Compare Interrupt should be false until the number is hit
                let mut ever_true = false;
                while count() < target {
                    let cause = cause();
                    // Check again, just in case we JUST hit compare
                    if count() < target {
                        ever_true |= cause.interrupt_compare();
                    }
                }

                soft_assert_eq(
                    ever_true,
                    false,
                    "COMPARE INT must be false until we reach COMPARE",
                )?;

                soft_assert_eq(
                    cause().interrupt_compare(),
                    true,
                    "COMPARE INT must be true once we reach COMPARE",
                )?;

                soft_assert_eq(
                    cause().coprocessor_error(),
                    u2::new(2),
                    "Coprocessor error should not be change until the interrupt actually fires",
                )?;

                Ok(())
            }
            _ => Err("Unexpected value".to_string()),
        }
    }
}

/// Similar to the test before, but this one waits for the interrupt to be signalled
/// and then verifies count
pub struct CompareInterruptSignalling2 {}

impl Test for CompareInterruptSignalling2 {
    fn name(&self) -> &str {
        "Compare (signalling 2)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((2000u32, 0u32)),
            Box::new((500u32, 0u32)),
            Box::new((100u32, 0u32)),
            Box::new((50u32, 0u32)),
            Box::new((4u32, 0u32)),
            Box::new((4u32, 1u32)),
            Box::new((4u32, 2u32)),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        fn test<const OP1: u32, const OP2: u32, const OP3: u32>(offset: u32) -> Result<(), String> {
            let iterations: u32;
            let target: u32;
            let count_out: u32;
            // Write in asm to get predictable timings
            unsafe {
                asm!("
                    .set noreorder
                    ORI $7, $0, 500
0:
                    // Remove even/odd jitter (count moves at half-cycles)
                    MFC0 $6, ${COUNT}
                    MFC0 $8, ${COUNT}
                    BEQ $6, $8, 1f
                    NOP
                    NOP
1:
                    MFC0 $6, ${COUNT}
                    ADDU $6, $6, $5
                    ADDU $6, $6, $7   // first iteration, increment by 500. This way we won't miss if the cache is cold
                    .word {OP1}
                    .word {OP2}
                    .word {OP3}
                    LUI $3, 0
2:
                    MFC0 $4, ${CAUSE}
                    SRL $4, $4, 15
                    ANDI $4, $4, 1
                    BEQZL $4, 2b
                    ADDIU $3, $3, 1

                    // Run the whole thing twice to ensure we're running out of icache
                    BNE $7, $0, 0b
                    ADDIU $7, $7, -500

                    MFC0 $4, ${COUNT}
                ",
                COUNT = const RegisterIndex::Count as usize,
                CAUSE = const RegisterIndex::Cause as usize,
                OP1 = const OP1,
                OP2 = const OP2,
                OP3 = const OP3,
                out("$3") iterations, out("$4") count_out, in("$5") offset, out("$6") target, out("$7") _, out("$8") _)
            }
            soft_assert_greater_or_equal(
                count_out,
                target,
                "COUNT must be >= the target compare value",
            )?;
            // The expected number of cycles per loop iteration is 6, because we have 5
            // instructions, but SRL has a stall on MFC0, causing 1 extra. 6 cycles cause 3 COUNT
            // increments. We also have to subtrace 2 due to the instructions before the loop
            let expected_cycles = (offset - 2) / 3;
            soft_assert_eq(iterations, expected_cycles, "Loop iterations")?;

            Ok(())
        }

        match (*value).downcast_ref::<(u32, u32)>() {
            Some((offset, mode)) => {
                preset_cause_to_copindex2()?;

                const NOP: u32 = Assembler::make_nop();
                const SET_COMPARE: u32 = Assembler::make_mtc0(GPR::A2, RegisterIndex::Compare);
                const BRANCH: u32 = Assembler::make_b(1);

                match *mode {
                    0 => {
                        // Set COMPARE, followed by two NOPs
                        test::<SET_COMPARE, NOP, NOP>(*offset)
                    }
                    1 => {
                        // Set COMPARE, then branch. If run by a dynarec, this will cause a new basic block
                        test::<SET_COMPARE, BRANCH, NOP>(*offset)
                    }
                    2 => {
                        // Branch and set compare from within delay slot.
                        test::<BRANCH, SET_COMPARE, NOP>(*offset)
                    }
                    _ => Err("Unexpected value".to_string()),
                }
            }
            _ => Err("Unexpected value".to_string()),
        }
    }
}

/// Setting COMPARE to the past should not trigger the interrupt (at least not until overflow)
pub struct CompareInterruptsPast;

impl Test for CompareInterruptsPast {
    fn name(&self) -> &str {
        "Compare (past)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        preset_cause_to_copindex2()?;
        let current = count();
        let target = current.wrapping_sub(2);
        set_compare(target);
        soft_assert_eq(compare(), target, "Compare readback not the same")?;

        // Compare Interrupt should be false and stay false. Cycle a bit to be sure
        let mut ever_true = false;
        while count() < current + 100 {
            let cause = cause();
            // Check again, just in case we JUST hit compare
            if count() < target {
                ever_true |= cause.interrupt_compare();
            }
        }

        soft_assert_eq(
            ever_true,
            false,
            "COMPARE INT must be false until we reach COMPARE",
        )?;

        Ok(())
    }
}
