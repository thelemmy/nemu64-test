pub mod cache;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::{asm, naked_asm};
use core::mem::transmute;

use arbitrary_int::{u26, u5};

use crate::assembler::{
    Assembler, Cop1Condition, Cop1FloatInstruction, Opcode, RegimmOpcode, FR, GPR,
};
use crate::cop0::{count, set_count, set_status, status, CacheOp, RegisterIndex, Status};
use crate::cop1::{fcsr, set_fcsr, FCSRFlags, FConst, FCSR};
use crate::memory_map::MemoryMap;
use crate::tests::soft_asserts::{soft_assert_eq, soft_assert_eq_decimal, soft_assert_range};
use crate::tests::{Level, Test};
use crate::uncached_memory::{UncachedHeapMemory, UncachedHeapMemoryWriter};

// TODO: Test BC1
// TODO: This test harness isn't entirely stable:
//     The main test runner (assert_cycles) calls into a function, with some hand-written assembly.
//     That function in turn might raise an exception (e.g. BREAK). Therefore, there are some
//     possibilities for icache problems if two or three of those areas need the same icache line.
//     In that case, the test becomes a lot slower.
//     To properly fix this, the main test runner should be generated into an array that is allocated
//     with precise alignment.
// TODO: Set register to immediate, then jump to that register via JR

#[unsafe(naked)]
extern "C" fn instant_return_function() {
    naked_asm!(
        "
            .set noat
            .set noreorder
            JR $31
            NOP
        "
    )
}

#[unsafe(naked)]
extern "C" fn one_nop_then_return_function() {
    naked_asm!(
        "
            .set noat
            .set noreorder
            NOP
            JR $31
            NOP
        "
    )
}

/// This test repeatedly reads COP0.Count and looks at the differences.
/// It expects count to increase on every other call, so the differences should be 1, 0, 1, 0, 1...
pub struct RepeatedMFC0Count {}

impl Test for RepeatedMFC0Count {
    fn name(&self) -> &str {
        "Timing: Repeated MFC0 COUNT"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        fn test_function(output: &mut [[u32; 8]; 10]) {
            unsafe {
                asm!("
            .set noat
            .set noreorder
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
                soft_assert_eq(
                    row,
                    &[0, 1, 0, 1, 0, 1, 0, 1],
                    "Expected iterating 0s and 1s",
                )?;
            } else {
                soft_assert_eq(
                    row,
                    &[1, 0, 1, 0, 1, 0, 1, 0],
                    "Expected iterating 0s and 1s",
                )?;
            }
        }

        Ok(())
    }
}

/// Writing to COUNT should set cycles precisely, eliminating any half-cycles that might be present.
/// This will be used in the measure function below, so have a test specifically for it
pub struct HalfCycleExactCalibration {}

impl Test for HalfCycleExactCalibration {
    fn name(&self) -> &str {
        "Timing: Half cycle calibration"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

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
                    NOP
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

            unsafe {
                set_count(backup_count);
            }

            // The hazard test tests the specific values we're getting back, which are more difficult
            // to reproduce. Here we just care about the deltas
            soft_assert_eq(out1 - out0, 1, "2nd - 1st readback")?;
            soft_assert_eq(out2 - out1, 0, "3rd - 2nd readback")?;
            soft_assert_eq(out3 - out2, 1, "4th - 3rd readback")?;
        }
        Ok(())
    }
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExceptionTimingMode {
    /// Don't do anything special about exceptions - if they fire, they will be shown as errors
    Off = 0,

    // If an exception fires, the "end" of the operation will be within the exception handler
    JustFire = 1,

    // If an exception fires, it will return but the total time will include the eret
    Roundtrip = 2,
}

/// Runs the passed in naked function and measures its runtime precisely.
/// The function is expected to end on JR RA with a NOP in the delay slot, but is otherwise
/// free to do anything. Registers during the test function:
///   - Integer registers $2 and $4 are the given input values2. Don't overwrite
///   - FPU registers F2 and F4 also have those two values. Don't overwrite
///   - Integer register $3 will hold a cached virtual address which can be read/written
///   - Integer register $5 will hold a uncached virtual address (to the same location as $3) which can be read/written
///   - Registers $6-$16 are free to be used in any way. $6-$9 will be initialized to $2-$5 in each loop, so they can overwritten
/// if use_exception_timing is true, the end timestamp will be determined within the exception handler (so it will exclude the ERET)

#[inline(never)]
fn assert_cycles(
    expected_cycles: u32,
    value2: u64,
    value4: u64,
    test_status: Status,
    exception_timing: ExceptionTimingMode,
    f: extern "C" fn(),
) -> Result<(), String> {
    let mut memory = UncachedHeapMemory::<u64>::new_with_align(8, 64);
    // Make this a pointer to itself. That allows reading from the same register again
    let start_physical = memory.start_physical();
    memory.write(
        0,
        MemoryMap::physical_to_cached_mut::<u64>(start_physical) as i32 as u64,
    );
    memory.write(
        1,
        MemoryMap::physical_to_uncached_mut::<u64>(start_physical + 8) as i32 as u64,
    );
    memory.write(2, (f as u64 + 16) as i32 as u64);
    memory.write(3, 0x89ABCDEF01234567u64); // when signed, this is negative
    memory.write(4, 0x456789ABCDEF1234u64); // when signed, this is positive
    memory.write(5, 0x56789ABCDEF12345u64);
    memory.write(6, 0x6789ABCDEF123456u64);
    memory.write(7, 0x789ABCDEF1234567u64);

    let ticks1: u32;
    let ticks2: u32;
    unsafe {
        let previous_status = status();

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
            // stash RA
            ORI $20, $31, 0

            DMFC1 $2, $2
            DMFC1 $4, $4
            NOP; NOP
            MTC0 $16, ${STATUS}

            .align 5 // align so that the loop below fits within the fewest ICACHE cachelines as possible
1:
            ORI $26, $18, 0  // Configure exception handler
            OR $6, $2, $0
            OR $7, $3, $0
            OR $8, $4, $0
            OR $9, $5, $0

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

            // If enabled, don't use the timing value we just got but instead use k1, which is what the exception handler stored
            ORI $17, $0, 1
            BNE $17, $18, 3f
            NOP
            ORI $21, $27, 0
3:
            ORI $23, $24, 0     // stash previous iteration's result in 23
            SUB $24, $21, $19
            ADDIU $22, $22, 0xFFFF

            // Loop a bit - this clears out the write-buffer if it was used by the test
            ORI $19, $0, 70
4:
            BNE $19, $0, 4b
            ADDIU $19, $19, -1 // delay slot

            BNE $22, $0, 1b
            NOP // delay slot
            ORI $31, $20, 0  // restore RA
            ORI $26, $0, 0  // restore normal exception handler behavior
        ",
        COUNT = const RegisterIndex::Count as usize,
        STATUS = const RegisterIndex::Status as usize,

        // Pass in values as fpu registers - this allows passing them in as 64 bit (even if illegal float values)
        in("$f2") transmute::<u64, f64>(value2),
        in("$f4") transmute::<u64, f64>(value4),

        // $2 and $4 will be copied from the FPU via DMFC1
        out("$2") _,
        in("$3") MemoryMap::physical_to_cached_mut::<u64>(memory.start_physical()),
        out("$4") _,
        in("$5") MemoryMap::physical_to_uncached_mut::<u64>(memory.start_physical()),

        // These are freely to be used to by test
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
        inout("$16") test_status.raw_value() => _,

        // These are for the test infra itself
        out("$17") _,
        in("$18") exception_timing as u32,
        out("$19") _,
        out("$20") _,
        out("$21") _,
        inout("$22") 3 => _,
        out("$23") ticks2,
        out("$24") ticks1,
        in("$25") f,
        options(nostack));

        set_status(previous_status);
    }

    // ticks2 is either the same or 1 larger than ticks
    soft_assert_range(
        ticks2,
        ticks1,
        ticks1 + 1,
        if exception_timing == ExceptionTimingMode::JustFire {
            "Two measurements aren't close. Most likely, exception didn't fire"
        } else {
            "Two measurements aren't close. Most likely, the MTC0 COUNT initialization didn't work or something else (e.g. cache effect) kicked in"
        },
    )?;

    let effective_ticks = if exception_timing == ExceptionTimingMode::JustFire {
        // In addition to the code being measured, we need a few extra cycles:
        // - 1 for the JALR
        // - 1 for the JALR's NOP
        (ticks1 + ticks2) - 2
    } else {
        // In addition to the code being measured, we need a few extra cycles:
        // - 1 for the JALR
        // - 1 for the JALR's NOP
        // - 1 for the return JR RA
        // - 1 for the delay of the JR RA
        // - 1 for one of the MFC0 COUNT itself
        (ticks1 + ticks2) - 5
    };
    soft_assert_eq_decimal(effective_ticks, expected_cycles, "Measured cycles")?;

    Ok(())
}

fn assert_cycles_with_codegen<F: FnOnce(&mut UncachedHeapMemoryWriter<u32>)>(
    expected_cycles: u32,
    value2: u64,
    value4: u64,
    test_status: Status,
    exception_timing: ExceptionTimingMode,
    f: F,
) -> Result<(), String> {
    // Dynamically generate code
    let mut code_memory = UncachedHeapMemory::<u32>::new_with_align(64, 64);
    let mut writer = UncachedHeapMemoryWriter::new(&mut code_memory);

    f(&mut writer);
    writer.write(Assembler::make_jr(GPR::RA));
    writer.write(Assembler::make_nop()); // delay slot

    // To execute, we'll run from a 0x8xxxxxxx address as otherwise the CPU will mostly
    // spend time reading from memory
    let cached_ptr = MemoryMap::physical_to_cached::<u8>(code_memory.start_physical());

    // Turn the pointer into a function pointer
    let function_ptr: extern "C" fn() = unsafe { transmute(cached_ptr) };

    assert_cycles(
        expected_cycles,
        value2,
        value4,
        test_status,
        exception_timing,
        function_ptr,
    )
}

fn assert_cycles_with_codegen_one_instruction(
    expected_cycles: u32,
    value2: u64,
    value4: u64,
    test_status: Status,
    exception_timing: ExceptionTimingMode,
    instruction: u32,
) -> Result<(), String> {
    assert_cycles_with_codegen(
        expected_cycles,
        value2,
        value4,
        test_status,
        exception_timing,
        |writer| writer.write(instruction),
    )
}

/// Assemble a test program that just has a few NOPs
pub struct PreciseMeasureJustNOPs {}

impl Test for PreciseMeasureJustNOPs {
    fn name(&self) -> &str {
        "Timing: Just NOPs"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new(0u32),
            Box::new(1u32),
            Box::new(2u32),
            Box::new(3u32),
            Box::new(6u32),
            Box::new(11u32),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<u32>() {
            Some(number_of_nops) => assert_cycles_with_codegen(
                *number_of_nops,
                0x12345678_ABCDEF,
                0,
                Status::DEFAULT,
                ExceptionTimingMode::Off,
                |writer| {
                    for _ in 0..*number_of_nops {
                        writer.write(Assembler::make_nop());
                    }
                },
            ),
            _ => Err(format!("Unexpected pattern")),
        }
    }
}

pub struct SingleInstructionCPUTiming {}

impl Test for SingleInstructionCPUTiming {
    fn name(&self) -> &str {
        "Timing: Individual instructions (CPU)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            // Expensive CPU instructions
            Box::new(("DIV", 37u32, Assembler::make_div(GPR::R0, GPR::R0))),
            Box::new(("DIVU", 37u32, Assembler::make_divu(GPR::R0, GPR::R0))),
            Box::new(("DDIV", 69u32, Assembler::make_ddiv(GPR::R0, GPR::R0))),
            Box::new(("DDIVU", 69u32, Assembler::make_ddivu(GPR::R0, GPR::R0))),
            Box::new(("MULT", 5u32, Assembler::make_mult(GPR::R0, GPR::R0))),
            Box::new(("MULTU", 5u32, Assembler::make_multu(GPR::R0, GPR::R0))),
            Box::new(("DMULT", 8u32, Assembler::make_dmult(GPR::R0, GPR::R0))),
            Box::new(("DMULTU", 8u32, Assembler::make_dmultu(GPR::R0, GPR::R0))),
            // These are all instant.
            Box::new(("MFLO", 1u32, Assembler::make_mflo(GPR::V0))),
            Box::new(("MFHI", 1u32, Assembler::make_mfhi(GPR::V0))),
            Box::new(("MTLO", 1u32, Assembler::make_mtlo(GPR::V0))),
            Box::new(("MTHI", 1u32, Assembler::make_mthi(GPR::V0))),
            Box::new(("TLBR", 1u32, Assembler::make_cop0_tlbr())),
            Box::new(("TLBP", 1u32, Assembler::make_cop0_tlbp())),
            Box::new(("TLBWR", 1u32, Assembler::make_cop0_tlbwr())),
            Box::new(("TLBWI", 1u32, Assembler::make_cop0_tlbwi())),
            Box::new(("SYNC", 1u32, Assembler::make_sync())),
            // Reads from COP0 need 1 cycle
            Box::new((
                "MFC0 Wired",
                1u32,
                Assembler::make_mfc0(GPR::V0, RegisterIndex::Wired),
            )),
            Box::new((
                "MFC0 Count",
                1u32,
                Assembler::make_mfc0(GPR::V0, RegisterIndex::Count),
            )),
            Box::new((
                "MFC0 Random",
                1u32,
                Assembler::make_mfc0(GPR::V0, RegisterIndex::Random),
            )),
            Box::new((
                "DMFC0 Wired",
                1u32,
                Assembler::make_dmfc0(GPR::V0, RegisterIndex::Wired),
            )),
            Box::new((
                "DMFC0 Count",
                1u32,
                Assembler::make_dmfc0(GPR::V0, RegisterIndex::Count),
            )),
            Box::new((
                "DMFC0 Random",
                1u32,
                Assembler::make_dmfc0(GPR::V0, RegisterIndex::Random),
            )),
            // Writes need 2 cycles
            Box::new((
                "MTC0 _Unused7",
                2u32,
                Assembler::make_mtc0(GPR::V0, RegisterIndex::_Unused7),
            )),
            Box::new((
                "MTC0 Random",
                2u32,
                Assembler::make_mtc0(GPR::R0, RegisterIndex::Random),
            )),
            Box::new((
                "MTC0 EntryHi",
                2u32,
                Assembler::make_mtc0(GPR::R0, RegisterIndex::EntryHi),
            )),
            Box::new((
                "MTC0 EntryLo0",
                2u32,
                Assembler::make_mtc0(GPR::R0, RegisterIndex::EntryLo0),
            )),
            Box::new((
                "MTC0 EntryLo1",
                2u32,
                Assembler::make_mtc0(GPR::R0, RegisterIndex::EntryLo1),
            )),
            Box::new((
                "DMTC0 _Unused7",
                2u32,
                Assembler::make_dmtc0(GPR::V0, RegisterIndex::_Unused7),
            )),
            Box::new((
                "DMTC0 Random",
                2u32,
                Assembler::make_dmtc0(GPR::R0, RegisterIndex::Random),
            )),
            Box::new((
                "DMTC0 EntryHi",
                2u32,
                Assembler::make_dmtc0(GPR::R0, RegisterIndex::EntryHi),
            )),
            Box::new((
                "DMTC0 EntryLo0",
                2u32,
                Assembler::make_dmtc0(GPR::R0, RegisterIndex::EntryLo0),
            )),
            Box::new((
                "DMTC0 EntryLo1",
                2u32,
                Assembler::make_dmtc0(GPR::R0, RegisterIndex::EntryLo1),
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(&str, u32, u32)>() {
            Some((_context, expected_cycles, instruction)) => {
                assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    0,
                    0,
                    Status::DEFAULT,
                    ExceptionTimingMode::Off,
                    *instruction,
                )
            }
            _ => Err(format!("Unexpected pattern")),
        }
    }
}

/// Exceptions are surprisingly quick
pub struct Exceptions {}

impl Test for Exceptions {
    fn name(&self) -> &str {
        "Timing: Exceptions"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new((
                "BREAK",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_break(),
            )),
            Box::new((
                "BREAK",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::Roundtrip,
                15u32,
                Assembler::make_break(),
            )),
            Box::new((
                "SYSCALL",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_syscall(),
            )),
            Box::new((
                "SYSCALL",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::Roundtrip,
                15u32,
                Assembler::make_syscall(),
            )),
            Box::new((
                "ADD",
                0x7FFFFFFFu64,
                1u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_add(GPR::R0, GPR::V0, GPR::A0),
            )),
            Box::new((
                "DADD",
                0x7FFFFFFF_FFFFFFFFu64,
                1u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_dadd(GPR::R0, GPR::V0, GPR::A0),
            )),
            Box::new((
                "SUB",
                0x80000000u64,
                0x7FFFFFFFu64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_sub(GPR::R0, GPR::V0, GPR::A0),
            )),
            Box::new((
                "DSUB",
                0x80000000_00000000u64,
                0x7FFFFFFF_FFFFFFFFu64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_dsub(GPR::R0, GPR::V0, GPR::A0),
            )),
            Box::new((
                "ADDI",
                0x7FFFFFFFu64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_addi(GPR::R0, GPR::V0, 1),
            )),
            Box::new((
                "DADDI",
                0x7FFFFFFF_FFFFFFFFu64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_daddi(GPR::R0, GPR::V0, 1),
            )),
            Box::new((
                "TNE",
                1u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_tne(GPR::V0, GPR::A0),
            )),
            Box::new((
                "TNEI",
                1u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_regimm_trap(RegimmOpcode::TNEI, GPR::V0.raw_value(), 0),
            )),
            Box::new((
                "TEQ",
                1u64,
                1u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_teq(GPR::V0, GPR::A0),
            )),
            Box::new((
                "TEQI",
                1u64,
                1u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_regimm_trap(RegimmOpcode::TEQI, GPR::V0.raw_value(), 1),
            )),
            Box::new((
                "TGE",
                2u64,
                1u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_tge(GPR::V0, GPR::A0),
            )),
            Box::new((
                "TGEI",
                2u64,
                1u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_regimm_trap(RegimmOpcode::TGEI, GPR::V0.raw_value(), 0),
            )),
            Box::new((
                "TGEU",
                2u64,
                1u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_tgeu(GPR::V0, GPR::A0),
            )),
            Box::new((
                "TGEIU",
                2u64,
                1u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_regimm_trap(RegimmOpcode::TGEIU, GPR::V0.raw_value(), 0),
            )),
            Box::new((
                "TLT",
                1u64,
                2u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_tlt(GPR::V0, GPR::A0),
            )),
            Box::new((
                "TLTI",
                1u64,
                1u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_regimm_trap(RegimmOpcode::TLTI, GPR::V0.raw_value(), 2),
            )),
            Box::new((
                "TLTU",
                1u64,
                2u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_tltu(GPR::V0, GPR::A0),
            )),
            Box::new((
                "TLTIU",
                1u64,
                1u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_regimm_trap(RegimmOpcode::TLTIU, GPR::V0.raw_value(), 2),
            )),
            // Address error (load from 0)
            Box::new((
                "LB $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lb(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LBU $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lbu(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LH $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lh(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LHU $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lhu(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LW $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lw(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LWU $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lwu(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LWR $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lwr(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LWL $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lwl(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LD $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_ld(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LDL $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_ldl(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LDR $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_ldr(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LL $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_ll(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LLD $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lld(GPR::R0, 0, GPR::R0),
            )),
            Box::new((
                "LWC1 $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lwc1(FR::F0, 0, GPR::R0),
            )),
            Box::new((
                "LDC1 $0, 0 ($0) (address error)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_ldc1(FR::F0, 0, GPR::R0),
            )),
            // Misalign exception
            Box::new((
                "LH $0, 1 ($V1) (misalign)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lh(GPR::R0, 1, GPR::V1),
            )),
            Box::new((
                "LHU $0, 1 ($V1) (misalign)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lhu(GPR::R0, 1, GPR::V1),
            )),
            Box::new((
                "LW $0, 1 ($V1) (misalign)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lw(GPR::R0, 1, GPR::V1),
            )),
            Box::new((
                "LWU $0, 1 ($V1) (misalign)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lwu(GPR::R0, 1, GPR::V1),
            )),
            Box::new((
                "LD $0, 1 ($V1) (misalign)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_ld(GPR::R0, 1, GPR::V1),
            )),
            Box::new((
                "LL $0, 1 ($V1) (misalign)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_ll(GPR::R0, 1, GPR::V1),
            )),
            Box::new((
                "LLD $0, 1 ($V1) (misalign)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lld(GPR::R0, 1, GPR::V1),
            )),
            Box::new((
                "LWC1 $0, 1 ($V1) (misalign)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_lwc1(FR::F0, 1, GPR::V1),
            )),
            Box::new((
                "LDC1 $0, 1 ($V1) (misalign)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_ldc1(FR::F0, 1, GPR::V1),
            )),
            // Non-existing CPU instruction
            Box::new((
                "_I28 (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_main_immediate(Opcode::_I28, GPR::R0, GPR::R0, 0),
            )),
            // COP1 unusable
            Box::new((
                "ADD.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SQRT.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ABS.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "NEG.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "MOV.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_mov(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.L.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.W.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.L.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.W.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.L.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.L.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.L.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "C.EQ.S (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SQRT.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ABS.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "NEG.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "MOV.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_mov(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.W.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.W.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "C.EQ.D (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "CVT.S.W (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.D.W (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.S.L (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.D.L (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "MFC1 (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_mfc1(GPR::R0, FR::F0),
            )),
            Box::new((
                "DMFC1 (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_dmfc1(GPR::R0, FR::F0),
            )),
            Box::new((
                "MTC1 (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_mtc1(GPR::R0, FR::F0),
            )),
            Box::new((
                "DMTC1 (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_dmtc1(GPR::R0, FR::F0),
            )),
            Box::new((
                "LWC1 (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_lwc1(FR::F0, 0, GPR::V1),
            )),
            Box::new((
                "LDC1 (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_ldc1(FR::F0, 0, GPR::V1),
            )),
            Box::new((
                "SWC1 (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_swc1(FR::F0, 0, GPR::V1),
            )),
            Box::new((
                "SDC1 (COP1 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop1usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_ldc1(FR::F0, 0, GPR::V1),
            )),
            // Non-existing COP1 instruction. The rule seems to be:
            // If an instruction is .S and .D (e.g. ADD, MUL, CVT.W) but used with W or L instead, it is 7 cycles. Any other situation the cost is 6 cycles
            Box::new((
                "ADD.W (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                7u32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).w(),
            )),
            Box::new((
                "ADD.L (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                7u32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).l(),
            )),
            Box::new((
                "MUL.W (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                7u32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).w(),
            )),
            Box::new((
                "MUL.L (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                7u32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).l(),
            )),
            Box::new((
                "_F16.S (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F16,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .s(),
            )),
            Box::new((
                "_F16.D (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F16,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .d(),
            )),
            Box::new((
                "_F16.W (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F16,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .w(),
            )),
            Box::new((
                "_F16.L (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F16,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .l(),
            )),
            Box::new((
                "_F31.S (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F31,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .s(),
            )),
            Box::new((
                "_F31.D (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F31,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .d(),
            )),
            Box::new((
                "_F31.W (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F31,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .w(),
            )),
            Box::new((
                "_F31.L (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F31,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .l(),
            )),
            Box::new((
                "_F34.S (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F34,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .s(),
            )),
            Box::new((
                "_F34.D (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F34,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .d(),
            )),
            Box::new((
                "_F34.W (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F34,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .w(),
            )),
            Box::new((
                "_F34.L (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F34,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .l(),
            )),
            Box::new((
                "_F35.S (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F35,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .s(),
            )),
            Box::new((
                "_F35.D (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F35,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .d(),
            )),
            Box::new((
                "_F35.W (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F35,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .w(),
            )),
            Box::new((
                "_F35.L (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F35,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .l(),
            )),
            Box::new((
                "_F38.S (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F38,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .s(),
            )),
            Box::new((
                "_F38.D (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F38,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .d(),
            )),
            Box::new((
                "_F38.W (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F38,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .w(),
            )),
            Box::new((
                "_F38.L (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F38,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .l(),
            )),
            Box::new((
                "_F47.S (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F47,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .s(),
            )),
            Box::new((
                "_F47.D (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F47,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .d(),
            )),
            Box::new((
                "_F47.W (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F47,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .w(),
            )),
            Box::new((
                "_F47.L (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_float_instruction(
                    Cop1FloatInstruction::_F47,
                    FR::F0,
                    FR::F2,
                    FR::F4,
                )
                .l(),
            )),
            Box::new((
                "CVT.S.S (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.D (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                6u32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.W (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                7u32,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.L.L (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                7u32,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.W.L (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                7u32,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.L.W (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                7u32,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "ROUND.W.W (doesn't exit)",
                0u64,
                0u64,
                Status::DEFAULT,
                ExceptionTimingMode::JustFire,
                7u32,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).w(),
            )),
            // Some of the most useless metrics
            Box::new((
                "MFC2 (COP2 usable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop2usable(true),
                ExceptionTimingMode::Off,
                1u32,
                Assembler::make_mfc2(GPR::R0, u5::new(0)),
            )),
            Box::new((
                "MFC2 (COP2 unusable)",
                0u64,
                0u64,
                Status::DEFAULT.with_cop2usable(false),
                ExceptionTimingMode::JustFire,
                5u32,
                Assembler::make_mfc2(GPR::R0, u5::new(0)),
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(&str, u64, u64, Status, ExceptionTimingMode, u32, u32)>() {
            Some((
                _context,
                value2,
                value4,
                status,
                exception_mode,
                expected_cycles,
                instruction,
            )) => {
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    *value2,
                    *value4,
                    *status,
                    *exception_mode,
                    *instruction,
                );
            }
            _ => {}
        }

        Err(format!("Unexpected pattern"))
    }
}

pub struct CachedLoadsAndStoreTiming {}

impl Test for CachedLoadsAndStoreTiming {
    fn name(&self) -> &str {
        "Timing: Cached loads and store (with warm cache)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            // Cached reads and writes are instant as the cache is already warm when we measure
            Box::new(("LB (cached)", 1u32, Assembler::make_lb(GPR::R0, 0, GPR::V1))),
            Box::new((
                "LBU (cached)",
                1u32,
                Assembler::make_lbu(GPR::R0, 0, GPR::V1),
            )),
            Box::new(("LH (cached)", 1u32, Assembler::make_lh(GPR::R0, 0, GPR::V1))),
            Box::new((
                "LHU (cached)",
                1u32,
                Assembler::make_lhu(GPR::R0, 0, GPR::V1),
            )),
            Box::new(("LW (cached)", 1u32, Assembler::make_lw(GPR::R0, 0, GPR::V1))),
            Box::new((
                "LWU (cached)",
                1u32,
                Assembler::make_lwu(GPR::R0, 0, GPR::V1),
            )),
            Box::new(("LD (cached)", 1u32, Assembler::make_ld(GPR::R0, 0, GPR::V1))),
            Box::new((
                "LWL (cached)",
                1u32,
                Assembler::make_lwl(GPR::R0, 0, GPR::V1),
            )),
            Box::new((
                "LWR (cached)",
                1u32,
                Assembler::make_lwr(GPR::R0, 0, GPR::V1),
            )),
            Box::new((
                "LDL (cached)",
                1u32,
                Assembler::make_ldl(GPR::R0, 0, GPR::V1),
            )),
            Box::new((
                "LDR (cached)",
                1u32,
                Assembler::make_ldr(GPR::R0, 0, GPR::V1),
            )),
            Box::new(("SB (cached)", 1u32, Assembler::make_sb(GPR::R0, 0, GPR::V1))),
            Box::new(("SH (cached)", 1u32, Assembler::make_sh(GPR::R0, 0, GPR::V1))),
            Box::new(("SW (cached)", 1u32, Assembler::make_sw(GPR::R0, 0, GPR::V1))),
            Box::new(("SD (cached)", 1u32, Assembler::make_sd(GPR::R0, 0, GPR::V1))),
            Box::new((
                "SWL (cached)",
                1u32,
                Assembler::make_swl(GPR::R0, 0, GPR::V1),
            )),
            Box::new((
                "SWR (cached)",
                1u32,
                Assembler::make_swr(GPR::R0, 0, GPR::V1),
            )),
            Box::new((
                "SDL (cached)",
                1u32,
                Assembler::make_sdl(GPR::R0, 0, GPR::V1),
            )),
            Box::new((
                "SDR (cached)",
                1u32,
                Assembler::make_sdr(GPR::R0, 0, GPR::V1),
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(&str, u32, u32)>() {
            Some((_context, expected_cycles, instruction)) => {
                assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    0,
                    0,
                    Status::DEFAULT,
                    ExceptionTimingMode::Off,
                    *instruction,
                )
            }
            _ => Err(format!("Unexpected pattern")),
        }
    }
}

/// Measures timing for COP1 instructions. In general, timing works like this:
///  - If a trivial special case is detected (e.g. an input value of 0.0 for ADD.S), 2 cycles are needed
///  - Otherwise each instruction has a regular number of cycles, e.g. 3 for ADD.S
///  - If there's a register dependency, a bonus cycle is added (see below for a test of those)
/// It is unclear how slow exception throwing operations are as these tests all have exceptions disabled
pub struct COP1Instructions32 {}

impl Test for COP1Instructions32 {
    fn name(&self) -> &str {
        "Timing: COP1 instruction (32 bit)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            // ADD.S with all exceptions disabled. It has these trivial cases: NAN, INFINITY, NEG_INFINITY, 0, -0
            Box::new((
                "ADD.S",
                3u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                3u32,
                1.0f32,
                -1.0f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                3u32,
                1.0f32,
                1000.0f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                3u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                3u32,
                1.0f32,
                -1000.0f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                0.0f32,
                -1000.0f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                -0.0f32,
                -1000.0f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                -1000.0f32,
                0.0f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                -1000.0f32,
                -0.0f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                -1000.0f32,
                f32::INFINITY,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                -1000.0f32,
                f32::NEG_INFINITY,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                f32::INFINITY,
                -1000.0f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                f32::NEG_INFINITY,
                -1000.0f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                f32::INFINITY,
                f32::NEG_INFINITY,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                FConst::QUIET_NAN_START_32,
                123f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                123f32,
                FConst::QUIET_NAN_START_32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            // Overflow
            Box::new((
                "ADD.S",
                3u32,
                f32::MAX,
                100000f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                3u32,
                3e38f32,
                8e37f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            // Underflow
            Box::new((
                "ADD.S",
                3u32,
                5285104e-37f32,
                -1.5391543e-37f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            // ADD.S with all exceptions enabled: Normal, trivial (same as above), then the various exception
            Box::new((
                "ADD.S",
                3u32,
                1f32,
                3f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                2u32,
                0f32,
                0f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                8u32,
                f32::MIN,
                -1f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                7u32,
                f32::INFINITY,
                f32::NEG_INFINITY,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                8u32,
                3e38f32,
                8e37f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                8u32,
                5285104e-37f32,
                -1.5391543e-37f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                1.1f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                7u32,
                1.1f32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                7u32,
                2f32,
                FConst::QUIET_NAN_START_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                2f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                7u32,
                2f32,
                FConst::SIGNALLING_NAN_START_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                2f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            // SUB is just like ADD
            Box::new((
                "SUB.S",
                3u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                3u32,
                1.0f32,
                -1.0f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                3u32,
                1.0f32,
                1000.0f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                3u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                3u32,
                1.0f32,
                -1000.0f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                0.0f32,
                -1000.0f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                -0.0f32,
                -1000.0f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                -1000.0f32,
                0.0f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                -1000.0f32,
                -0.0f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                -1000.0f32,
                f32::INFINITY,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                -1000.0f32,
                f32::NEG_INFINITY,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                f32::INFINITY,
                -1000.0f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                f32::NEG_INFINITY,
                -1000.0f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                f32::INFINITY,
                f32::NEG_INFINITY,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                FConst::QUIET_NAN_START_32,
                123f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                123f32,
                FConst::QUIET_NAN_START_32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            // Overflow
            Box::new((
                "SUB.S",
                3u32,
                f32::MAX,
                -100000f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                3u32,
                3e38f32,
                -8e37f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            // Underflow
            Box::new((
                "SUB.S",
                3u32,
                5285104e-37f32,
                1.5391543e-37f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            // SUB.S with all exceptions enabled: Normal, trivial (same as above), then the various exception
            Box::new((
                "SUB.S",
                3u32,
                1f32,
                3f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                2u32,
                0f32,
                0f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                8u32,
                f32::MIN,
                1f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                7u32,
                f32::INFINITY,
                f32::INFINITY,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                8u32,
                3e38f32,
                -8e37f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                8u32,
                5285104e-37f32,
                1.5391543e-37f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                1.1f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                7u32,
                1.1f32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                7u32,
                2f32,
                FConst::QUIET_NAN_START_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                2f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                7u32,
                2f32,
                FConst::SIGNALLING_NAN_START_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                2f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            // MUL.S has these trivial cases: NAN, INFINITY, NEG_INFINITY, 0, -0, underflows and any power of two (e.g. 32)
            Box::new((
                "MUL.S",
                5u32,
                123f32,
                0.66f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                1.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                2.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                5u32,
                123f32,
                3.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                4.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                5u32,
                123f32,
                6.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                8.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                16.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                32.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                64.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                128.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                256.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                512.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                1024.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                2048.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                4096.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                8192.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                16384.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                32768.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                65536.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                65536.0f32 * 65536.0f32 * 65536.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                124f32,
                1.7014118346e+38f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                0.5f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                0.25f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                5u32,
                123f32,
                0.2f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                0.125f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                1f32 / 8f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                1f32 / 65536f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                1f32 / 65536f32 / 65536f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                1f32 / 65536f32 / 65536f32 / 65536f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                0f32,
                123f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                f32::INFINITY,
                -1000.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                f32::NEG_INFINITY,
                -1000.0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                f32::INFINITY,
                f32::NEG_INFINITY,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                FConst::QUIET_NAN_START_32,
                123f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                123f32,
                FConst::QUIET_NAN_START_32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            // Overflow
            Box::new((
                "MUL.S",
                5u32,
                f32::MAX,
                f32::MAX,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                f32::MAX,
                2f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                5u32,
                f32::MAX,
                2.1f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            // Underflow
            Box::new((
                "MUL.S",
                2u32,
                f32::MIN_POSITIVE,
                0.5f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                2u32,
                f32::MIN_POSITIVE,
                0.222f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            // MUL.S with exceptions on
            Box::new((
                "MUL.S (inexact)",
                10u32,
                0.123456789f32,
                10.123456789f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S (underflow with other number being pow2)",
                7u32,
                1.17549449095e-38f32,
                0.125f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S (underflow)",
                10u32,
                1.17549449095e-38f32,
                0.13f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S (overflow)",
                10u32,
                f32::MAX,
                f32::MAX,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S (overflow trivial)",
                7u32,
                f32::MAX,
                128f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                7u32,
                f32::INFINITY,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                1.1f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                7u32,
                1.1f32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                7u32,
                1.1f32,
                FConst::QUIET_NAN_START_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                1.1f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                7u32,
                1.1f32,
                FConst::SIGNALLING_NAN_START_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                1.1f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            // DIV.S has these trivial cases: 0, NAN, INFINITY, NEG_INFINITY, 0, -0
            Box::new((
                "DIV.S",
                29u32,
                123f32,
                0.66f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                29u32,
                123f32,
                1f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                29u32,
                1f32,
                123f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                29u32,
                1f32,
                1f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                29u32,
                12f32,
                12f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F2).s(),
            )), // note the second argument is F2 to the same register
            Box::new((
                "DIV.S",
                2u32,
                0f32,
                123f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                -0f32,
                123f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                123f32,
                0f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                123f32,
                -0f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                0f32,
                0f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                0f32,
                -0f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                -0f32,
                0f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                -0f32,
                -0f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                29u32,
                f32::MIN,
                123f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                29u32,
                123f32,
                f32::MIN,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                f32::INFINITY,
                -1000.0f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                f32::INFINITY,
                f32::INFINITY,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                f32::NEG_INFINITY,
                -1000.0f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                f32::INFINITY,
                f32::NEG_INFINITY,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                FConst::QUIET_NAN_START_32,
                123f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S",
                2u32,
                123f32,
                FConst::QUIET_NAN_START_32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            // DIV.S with exceptions on
            Box::new((
                "DIV.S (inexact)",
                34u32,
                0.123456789f32,
                10.123456789f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (15/0)",
                7u32,
                15f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (overflow)",
                34u32,
                f32::MAX,
                0.125f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (underflow)",
                34u32,
                f32::MIN_POSITIVE,
                128f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (inf/inf)",
                7u32,
                f32::INFINITY,
                f32::INFINITY,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (inf/-inf)",
                7u32,
                f32::INFINITY,
                f32::NEG_INFINITY,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (subnormal/1)",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                1f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (1/subnormal)",
                7u32,
                1f32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (1/qNAN)",
                7u32,
                1f32,
                FConst::QUIET_NAN_START_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (qNAN/1)",
                7u32,
                FConst::QUIET_NAN_START_32,
                1f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (1/sNAN)",
                7u32,
                1f32,
                FConst::SIGNALLING_NAN_START_32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (sNAN/1)",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                1f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            // NEG.S is always 1 cycle
            Box::new((
                "NEG.S",
                1u32,
                123f32,
                0f32,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "NEG.S",
                1u32,
                -123f32,
                0f32,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "NEG.S",
                1u32,
                f32::INFINITY,
                0f32,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "NEG.S",
                1u32,
                f32::NEG_INFINITY,
                0f32,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "NEG.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "NEG.S",
                6u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "NEG.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "NEG.S",
                6u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
            )),
            // ABS.S is always 1 cycle
            Box::new((
                "ABS.S",
                1u32,
                123f32,
                0f32,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ABS.S",
                1u32,
                -123f32,
                0f32,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ABS.S",
                1u32,
                f32::INFINITY,
                0f32,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ABS.S",
                1u32,
                f32::NEG_INFINITY,
                0f32,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ABS.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ABS.S",
                6u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ABS.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ABS.S",
                6u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
            )),
            // SQRT.S has these trivial cases: negative input, 0, NAN, INFINITY.
            Box::new((
                "SQRT.S",
                29u32,
                123f32,
                0f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                29u32,
                16f32,
                0f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                29u32,
                4f32,
                0f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                29u32,
                1f32,
                0f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                29u32,
                f32::MAX,
                0f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                2u32,
                0f32,
                0f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                2u32,
                -0f32,
                0f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                2u32,
                -123f32,
                0f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                2u32,
                f32::INFINITY,
                0f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                2u32,
                f32::NEG_INFINITY,
                0f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S (qNAN with exceptions disabled)",
                2u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S (negative)",
                7u32,
                -16f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S (inexact)",
                34u32,
                f32::MAX,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
            )),
            // MOV.S is always 1 cycle
            Box::new((
                "MOV.S",
                1u32,
                123f32,
                0f32,
                Assembler::make_cop1_mov(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "MOV.S",
                1u32,
                -123f32,
                0f32,
                Assembler::make_cop1_mov(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "MOV.S",
                1u32,
                f32::INFINITY,
                0f32,
                Assembler::make_cop1_mov(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "MOV.S",
                1u32,
                f32::NEG_INFINITY,
                0f32,
                Assembler::make_cop1_mov(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "MOV.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                Assembler::make_cop1_mov(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "MOV.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                Assembler::make_cop1_mov(FR::F0, FR::F2).s(),
            )),
            // ROUND.W.S is 5 cycles. In the exception case, subnormal and nan are trivial. Too large numbers are trivial if exponent >= 0b10011111
            Box::new((
                "ROUND.W.S",
                5u32,
                0f32,
                0f32,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S",
                5u32,
                1f32,
                0f32,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S",
                5u32,
                123.15f32,
                0f32,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S",
                5u32,
                -123f32,
                0f32,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S (inexact)",
                10u32,
                0.5f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S (a bit too large)",
                10u32,
                2147483600f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S (largest non trivial)",
                10u32,
                4294967040f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S (largest non trivial neg)",
                10u32,
                -4294967040f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S (smallest trivial)",
                7u32,
                4294967296f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S (smallest trivial neg)",
                7u32,
                -4294967296f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S (max)",
                7u32,
                f32::MAX,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S (inf)",
                7u32,
                f32::INFINITY,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.W.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.W.S",
                5u32,
                123.15f32,
                0f32,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.W.S",
                5u32,
                -123.15f32,
                0f32,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.W.S",
                5u32,
                0f32,
                0f32,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.W.S",
                10u32,
                0.5f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.W.S",
                10u32,
                2147483600f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.W.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.W.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.W.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.W.S",
                5u32,
                123.15f32,
                0f32,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.W.S",
                5u32,
                -123.15f32,
                0f32,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.W.S",
                5u32,
                0f32,
                0f32,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.W.S",
                10u32,
                0.5f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.W.S",
                10u32,
                2147483600f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.W.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.W.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.W.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.W.S",
                5u32,
                123.15f32,
                0f32,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.W.S",
                5u32,
                -123.15f32,
                0f32,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.W.S",
                5u32,
                0f32,
                0f32,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.W.S",
                10u32,
                0.5f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.W.S",
                10u32,
                2147483600f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.W.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.W.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.W.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S",
                5u32,
                0f32,
                0f32,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S",
                5u32,
                123.15f32,
                0f32,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S",
                5u32,
                -123.15f32,
                0f32,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S",
                10u32,
                0.5f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S",
                10u32,
                2147483600f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
            )),
            // ROUND.L.S is 5 cycles. In the exception case, subnormal, nan and too-large-to-fit are trivial
            Box::new((
                "ROUND.L.S",
                5u32,
                0f32,
                0f32,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.L.S",
                5u32,
                1f32,
                0f32,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.L.S",
                5u32,
                123.15f32,
                0f32,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.L.S",
                5u32,
                -123f32,
                0f32,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.L.S (inexact)",
                10u32,
                0.5f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.L.S (too large)",
                7u32,
                9.00719925474e15f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.L.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.L.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "ROUND.L.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.L.S",
                5u32,
                0f32,
                0f32,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.L.S",
                5u32,
                100f32,
                0f32,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.L.S",
                5u32,
                -100f32,
                0f32,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.L.S",
                10u32,
                0.5f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.L.S",
                7u32,
                9.00719925474e15f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.L.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.L.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "TRUNC.L.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.L.S",
                5u32,
                0f32,
                0f32,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.L.S",
                5u32,
                10f32,
                0f32,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.L.S",
                5u32,
                -10f32,
                0f32,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.L.S",
                10u32,
                0.5f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.L.S",
                7u32,
                9.00719925474e15f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.L.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.L.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CEIL.L.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.L.S",
                5u32,
                0f32,
                0f32,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.L.S",
                5u32,
                15f32,
                0f32,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.L.S",
                5u32,
                -15f32,
                0f32,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.L.S",
                10u32,
                0.5f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.L.S",
                7u32,
                9.00719925474e15f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.L.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.L.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "FLOOR.L.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S",
                5u32,
                0f32,
                0f32,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S",
                5u32,
                1f32,
                0f32,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S",
                5u32,
                -1.5f32,
                0f32,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S",
                10u32,
                0.5f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S",
                7u32,
                9.00719925474e15f32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S",
                7u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S",
                7u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
            )),
            // CVT.D.S is always 1 cycle
            Box::new((
                "CVT.D.S",
                1u32,
                0f32,
                0f32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.S",
                1u32,
                f32::INFINITY,
                0f32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.S",
                1u32,
                0.1111f32,
                0f32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.S",
                1u32,
                -0.1111f32,
                0f32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.S",
                6u32,
                FConst::SUBNORMAL_MIN_POSITIVE_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.S",
                6u32,
                FConst::SIGNALLING_NAN_START_32,
                0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
            )),
            // CVT.S.W has one trivial case: 0. Otherwise it needs 5 cycles. The only possible exception is inexact, which isn't trivial.
            Box::new((
                "CVT.S.W",
                2u32,
                0i32,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.S.W",
                5u32,
                i32::MAX,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.S.W",
                5u32,
                u32::MAX as i32,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.S.W",
                5u32,
                1i32,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.S.W",
                5u32,
                2i32,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.S.W",
                5u32,
                -2i32,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.S.W",
                5u32,
                1234567891i32,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.S.W (inexact)",
                10u32,
                1234567891i32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).w(),
            )),
            // CVT.S.L has one special case: 0. Otherwise it needs 5 cycles. Two exception cases: Too large is trivial, inexact is not.
            Box::new((
                "CVT.S.L",
                2u32,
                0i64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.S.L",
                5u32,
                1i64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.S.L",
                5u32,
                128i64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.S.L",
                5u32,
                1i64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.S.L",
                5u32,
                2i64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.S.L",
                5u32,
                -2i64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.S.L",
                5u32,
                1234567891i64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.S.L",
                10u32,
                1234567891i64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.S.L",
                7u32,
                1i64 << 55,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).l(),
            )),
            // MTC1
            Box::new((
                "MTC1",
                1u32,
                0f32,
                0f32,
                Assembler::make_mtc1(GPR::V0, FR::F0),
            )),
            // Compares are all single-cycle
            Box::new((
                "C.F.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.F.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.F.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.F.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.F.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.UN.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::UN, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.UN.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::UN, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.UN.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::UN, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.UN.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::UN, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.UN.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::UN, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.EQ.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.EQ.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.EQ.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.EQ.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.EQ.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.UEQ.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::UEQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.UEQ.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::UEQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.UEQ.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::UEQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.UEQ.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::UEQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.UEQ.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::UEQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLT.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::OLT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLT.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::OLT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLT.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::OLT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLT.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::OLT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLT.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::OLT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.ULT.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::ULT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.ULT.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::ULT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.ULT.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::ULT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.ULT.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::ULT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.ULT.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::ULT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLE.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::OLE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLE.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::OLE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLE.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::OLE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLE.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::OLE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLE.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::OLE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLE.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::ULE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLE.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::ULE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLE.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::ULE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLE.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::ULE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.OLE.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::ULE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SF.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SF.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SF.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SF.S",
                6u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SF.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SF.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGLE.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGLE.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGLE.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGLE.S",
                6u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGLE.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGLE.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SEQ.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SEQ.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SEQ.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SEQ.S",
                6u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SEQ.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.SEQ.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGL.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGL.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGL.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGL.S",
                6u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGL.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGL.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LT.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LT.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LT.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LT.S",
                6u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LT.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LT.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGE.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGE.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGE.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGE.S",
                6u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGE.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGE.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LE.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LE.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LE.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LE.S",
                6u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LE.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.LE.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGT.S",
                1u32,
                0.0f32,
                -0.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGT.S",
                1u32,
                1.0f32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGT.S",
                1u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGT.S",
                6u32,
                FConst::SIGNALLING_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGT.S",
                1u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "C.NGT.S",
                6u32,
                FConst::QUIET_NAN_START_32,
                1.0f32,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).s(),
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(&str, u32, f32, f32, u32)>() {
            Some((_context, expected_cycles, value2, value4, instruction)) => {
                set_fcsr(
                    fcsr()
                        .with_enable_invalid_operation(false)
                        .with_flush_denorm_to_zero(true),
                );
                let value2_u64 = unsafe { transmute::<f32, u32>(*value2) } as u64;
                let value4_u64 = unsafe { transmute::<f32, u32>(*value4) } as u64;
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    value2_u64,
                    value4_u64,
                    Status::DEFAULT,
                    ExceptionTimingMode::Off,
                    *instruction,
                );
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, f32, f32, ExceptionTimingMode, u32)>() {
            Some((_context, expected_cycles, value2, value4, exception_mode, instruction)) => {
                set_fcsr(FCSR::DEFAULT.with_enables(FCSRFlags::ALL));
                let value2_u64 = unsafe { transmute::<f32, u32>(*value2) } as u64;
                let value4_u64 = unsafe { transmute::<f32, u32>(*value4) } as u64;
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    value2_u64,
                    value4_u64,
                    Status::DEFAULT,
                    *exception_mode,
                    *instruction,
                );
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, i32, u32)>() {
            Some((_context, expected_cycles, value2, instruction)) => {
                set_fcsr(
                    fcsr()
                        .with_enable_invalid_operation(false)
                        .with_flush_denorm_to_zero(true),
                );
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    *value2 as u64,
                    0,
                    Status::DEFAULT,
                    ExceptionTimingMode::Off,
                    *instruction,
                );
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, i32, ExceptionTimingMode, u32)>() {
            Some((_context, expected_cycles, value2, exception_mode, instruction)) => {
                set_fcsr(FCSR::DEFAULT.with_enables(FCSRFlags::ALL));
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    *value2 as u64,
                    0,
                    Status::DEFAULT,
                    *exception_mode,
                    *instruction,
                );
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, i64, u32)>() {
            Some((_context, expected_cycles, value2, instruction)) => {
                set_fcsr(
                    fcsr()
                        .with_enable_invalid_operation(false)
                        .with_flush_denorm_to_zero(true),
                );
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    *value2 as u64,
                    0,
                    Status::DEFAULT,
                    ExceptionTimingMode::Off,
                    *instruction,
                );
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, i64, ExceptionTimingMode, u32)>() {
            Some((_context, expected_cycles, value2, exception_mode, instruction)) => {
                set_fcsr(FCSR::DEFAULT.with_enables(FCSRFlags::ALL));
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    *value2 as u64,
                    0,
                    Status::DEFAULT,
                    *exception_mode,
                    *instruction,
                );
            }
            _ => {}
        }

        return Err(format!("Unexpected pattern"));
    }
}

pub struct COP1Instructions64 {}

impl Test for COP1Instructions64 {
    fn name(&self) -> &str {
        "Timing: COP1 instruction (64 bit)"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            // ADD.D with all exceptions disabled. It has these trivial cases: NAN, INFINITY, NEG_INFINITY, 0, -0
            Box::new((
                "ADD.D",
                3u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                3u32,
                1.0f64,
                -1.0f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                3u32,
                1.0f64,
                1000.0f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                3u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                3u32,
                1.0f64,
                -1000.0f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                0.0f64,
                -1000.0f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                -0.0f64,
                -1000.0f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                -1000.0f64,
                0.0f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                -1000.0f64,
                -0.0f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                -1000.0f64,
                f64::INFINITY,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                -1000.0f64,
                f64::NEG_INFINITY,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                f64::INFINITY,
                -1000.0f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                f64::NEG_INFINITY,
                -1000.0f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                f64::INFINITY,
                f64::NEG_INFINITY,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                FConst::QUIET_NAN_START_64,
                123f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                123f64,
                FConst::QUIET_NAN_START_64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            // Overflow
            Box::new((
                "ADD.D",
                3u32,
                f64::MAX,
                100000f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                3u32,
                3e38f64,
                8e37f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            // Underflow
            Box::new((
                "ADD.D",
                3u32,
                5285104e-37f64,
                -1.5391543e-37f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            // ADD.D with all exceptions enabled: Normal, trivial (same as above), then the various exception
            Box::new((
                "ADD.D",
                3u32,
                1f64,
                3f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                2u32,
                0f64,
                0f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                8u32,
                f64::MIN,
                -1f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                7u32,
                f64::INFINITY,
                f64::NEG_INFINITY,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                8u32,
                3e38f64,
                8e37f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                8u32,
                5285104e-37f64,
                -1.5391543e-37f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                1.1f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                7u32,
                1.1f64,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                7u32,
                2f64,
                FConst::QUIET_NAN_START_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                2f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                7u32,
                2f64,
                FConst::SIGNALLING_NAN_START_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                2f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            // SUB is just like ADD
            Box::new((
                "SUB.D",
                3u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                3u32,
                1.0f64,
                -1.0f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                3u32,
                1.0f64,
                1000.0f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                3u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                3u32,
                1.0f64,
                -1000.0f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                0.0f64,
                -1000.0f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                -0.0f64,
                -1000.0f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                -1000.0f64,
                0.0f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                -1000.0f64,
                -0.0f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                -1000.0f64,
                f64::INFINITY,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                -1000.0f64,
                f64::NEG_INFINITY,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                f64::INFINITY,
                -1000.0f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                f64::NEG_INFINITY,
                -1000.0f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                f64::INFINITY,
                f64::NEG_INFINITY,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                FConst::QUIET_NAN_START_64,
                123f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                123f64,
                FConst::QUIET_NAN_START_64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            // Overflow
            Box::new((
                "SUB.D",
                3u32,
                f64::MAX,
                -100000f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                3u32,
                3e38f64,
                -8e37f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            // Underflow
            Box::new((
                "SUB.D",
                3u32,
                5285104e-37f64,
                1.5391543e-37f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            // SUB.D with all exceptions enabled: Normal, trivial (same as above), then the various exception
            Box::new((
                "SUB.D",
                3u32,
                1f64,
                3f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                2u32,
                0f64,
                0f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                8u32,
                f64::MIN,
                1f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                7u32,
                f64::INFINITY,
                f64::INFINITY,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                8u32,
                3e38f64,
                -8e37f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                8u32,
                5285104e-37f64,
                1.5391543e-37f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                1.1f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                7u32,
                1.1f64,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                7u32,
                2f64,
                FConst::QUIET_NAN_START_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                2f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                7u32,
                2f64,
                FConst::SIGNALLING_NAN_START_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                2f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            // MUL.D has these trivial cases: NAN, INFINITY, NEG_INFINITY, 0, -0, underflows and any power of two (e.g. 32)
            Box::new((
                "MUL.D",
                8u32,
                123f64,
                0.66f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                1.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                2.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                8u32,
                123f64,
                3.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                4.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                8u32,
                123f64,
                6.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                8.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                16.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                32.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                64.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                128.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                256.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                512.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                1024.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                2048.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                4096.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                8192.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                16384.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                32768.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                65536.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                65536.0f64 * 65536.0f64 * 65536.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                0.5f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                0.25f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                8u32,
                123f64,
                0.2f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                0.125f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                1f64 / 8f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                1f64 / 65536f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                1f64 / 65536f64 / 65536f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                1f64 / 65536f64 / 65536f64 / 65536f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                0f64,
                123f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                f64::INFINITY,
                -1000.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                f64::NEG_INFINITY,
                -1000.0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                f64::INFINITY,
                f64::NEG_INFINITY,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                FConst::QUIET_NAN_START_64,
                123f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                123f64,
                FConst::QUIET_NAN_START_64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            // Overflow
            Box::new((
                "MUL.D",
                8u32,
                f64::MAX,
                f64::MAX,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                f64::MAX,
                2f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                8u32,
                f64::MAX,
                2.1f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            // Underflow
            Box::new((
                "MUL.D",
                2u32,
                f64::MIN_POSITIVE,
                0.5f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                2u32,
                f64::MIN_POSITIVE,
                0.222f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            // MUL.D with exceptions on
            Box::new((
                "MUL.D (inexact)",
                13u32,
                0.123456789f64,
                10.123456789f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D (underflow with other number pow2)",
                7u32,
                2.225073858507202e-308f64,
                0.125f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D (underflow)",
                13u32,
                2.225073858507202e-308f64,
                0.13f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D (overflow)",
                13u32,
                f64::MAX,
                f64::MAX,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D (overflow trivial)",
                7u32,
                f64::MAX,
                128f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                7u32,
                f64::INFINITY,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                1.1f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                7u32,
                1.1f64,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                7u32,
                1.1f64,
                FConst::QUIET_NAN_START_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                1.1f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                7u32,
                1.1f64,
                FConst::SIGNALLING_NAN_START_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                1.1f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            // DIV.D has these trivial cases: 0, NAN, INFINITY, NEG_INFINITY, 0, -0
            Box::new((
                "DIV.D",
                58u32,
                123f64,
                0.66f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                58u32,
                123f64,
                1f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                58u32,
                1f64,
                123f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                58u32,
                1f64,
                1f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                58u32,
                12f64,
                12f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F2).d(),
            )), // note the second argument is F2 to the same register
            Box::new((
                "DIV.D",
                2u32,
                0f64,
                123f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                -0f64,
                123f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                123f64,
                0f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                123f64,
                -0f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                0f64,
                0f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                0f64,
                -0f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                -0f64,
                0f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                -0f64,
                -0f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                58u32,
                f64::MIN,
                123f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                58u32,
                123f64,
                f64::MIN,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                f64::INFINITY,
                -1000.0f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                f64::INFINITY,
                f64::INFINITY,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                f64::NEG_INFINITY,
                -1000.0f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                f64::INFINITY,
                f64::NEG_INFINITY,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                FConst::QUIET_NAN_START_64,
                123f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D",
                2u32,
                123f64,
                FConst::QUIET_NAN_START_64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            // DIV.D with exceptions on
            Box::new((
                "DIV.D (inexact)",
                63u32,
                0.123456789f64,
                10.123456789f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (15/0)",
                7u32,
                15f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (overflow)",
                63u32,
                f64::MAX,
                0.125f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (underflow)",
                63u32,
                f64::MIN_POSITIVE,
                128f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (inf/inf)",
                7u32,
                f64::INFINITY,
                f64::INFINITY,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (inf/-inf)",
                7u32,
                f64::INFINITY,
                f64::NEG_INFINITY,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (subnormal/1)",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                1f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (subnormal/0)",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (1/subnormal)",
                7u32,
                1f64,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (1/qNAN)",
                7u32,
                1f64,
                FConst::QUIET_NAN_START_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (qNAN/1)",
                7u32,
                FConst::QUIET_NAN_START_64,
                1f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (1/sNAN)",
                7u32,
                1f64,
                FConst::SIGNALLING_NAN_START_64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (sNAN/1)",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                1f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            // NEG.D is always 1 cycle
            Box::new((
                "NEG.D",
                1u32,
                123f64,
                0f64,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "NEG.D",
                1u32,
                -123f64,
                0f64,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "NEG.D",
                1u32,
                f64::INFINITY,
                0f64,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "NEG.D",
                1u32,
                f64::NEG_INFINITY,
                0f64,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "NEG.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "NEG.D",
                6u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "NEG.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "NEG.D",
                6u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
            )),
            // ABS.D is always 1 cycle
            Box::new((
                "ABS.D",
                1u32,
                123f64,
                0f64,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ABS.D",
                1u32,
                -123f64,
                0f64,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ABS.D",
                1u32,
                f64::INFINITY,
                0f64,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ABS.D",
                1u32,
                f64::NEG_INFINITY,
                0f64,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ABS.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ABS.D",
                6u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ABS.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ABS.D",
                6u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
            )),
            // SQRT.D has these trivial cases: negative input, 0, NAN, INFINITY
            Box::new((
                "SQRT.D",
                58u32,
                123f64,
                0f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                58u32,
                16f64,
                0f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                58u32,
                4f64,
                0f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                58u32,
                1f64,
                0f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                58u32,
                f64::MAX,
                0f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                2u32,
                0f64,
                0f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                2u32,
                -0f64,
                0f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                2u32,
                -123f64,
                0f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                2u32,
                f64::INFINITY,
                0f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                2u32,
                f64::NEG_INFINITY,
                0f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D (qNAN with exceptions disabled)",
                2u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D (negative)",
                7u32,
                -16f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D (inexact)",
                63u32,
                f64::MAX,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
            )),
            // MOV.D is always 1 cycle
            Box::new((
                "MOV.D",
                1u32,
                123f64,
                0f64,
                Assembler::make_cop1_mov(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "MOV.D",
                1u32,
                -123f64,
                0f64,
                Assembler::make_cop1_mov(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "MOV.D",
                1u32,
                f64::INFINITY,
                0f64,
                Assembler::make_cop1_mov(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "MOV.D",
                1u32,
                f64::NEG_INFINITY,
                0f64,
                Assembler::make_cop1_mov(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "MOV.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                Assembler::make_cop1_mov(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "MOV.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                Assembler::make_cop1_mov(FR::F0, FR::F2).d(),
            )),
            // ROUND.W.D is 5 cycles. In the exception case, subnormal and nan are trivial. Too large numbers are trivial if exponent >= 0b10000011111
            Box::new((
                "ROUND.W.D",
                5u32,
                0f64,
                0f64,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D",
                5u32,
                1f64,
                0f64,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D",
                5u32,
                123.15f64,
                0f64,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D",
                5u32,
                -123f64,
                0f64,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D",
                10u32,
                0.5f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D (too large)",
                10u32,
                2147483648f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D (too large 2)",
                10u32,
                2147483649f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D (largest non trivial)",
                10u32,
                4294967295.9999995f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D (largest neg non trivial)",
                10u32,
                -4294967295.9999995f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D (way too large - trivial)",
                7u32,
                4294967296f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D (way too large - trivial neg)",
                7u32,
                -4294967296f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D (way too large 2 - trivial)",
                7u32,
                4294967297f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D (max)",
                7u32,
                f64::MAX,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D (inf)",
                7u32,
                f64::INFINITY,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.W.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.W.D",
                5u32,
                123.15f64,
                0f64,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.W.D",
                5u32,
                -123.15f64,
                0f64,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.W.D",
                5u32,
                0f64,
                0f64,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.W.D",
                10u32,
                0.5f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.W.D",
                10u32,
                2147483648f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.W.D (way too large - trivial)",
                7u32,
                4294967296f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.W.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.W.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.W.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.W.D",
                5u32,
                123.15f64,
                0f64,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.W.D",
                5u32,
                -123.15f64,
                0f64,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.W.D",
                5u32,
                0f64,
                0f64,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.W.D",
                10u32,
                0.5f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.W.D",
                10u32,
                2147483648f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.W.D (way too large - trivial)",
                7u32,
                4294967296f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.W.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.W.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.W.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.W.D",
                5u32,
                123.15f64,
                0f64,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.W.D",
                5u32,
                -123.15f64,
                0f64,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.W.D",
                5u32,
                0f64,
                0f64,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.W.D",
                10u32,
                0.5f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.W.D",
                10u32,
                2147483648f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.W.D (way too large - trivial)",
                7u32,
                4294967296f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.W.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.W.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.W.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D",
                5u32,
                0f64,
                0f64,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D",
                5u32,
                123.15f64,
                0f64,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D",
                5u32,
                -123.15f64,
                0f64,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D",
                10u32,
                0.5f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D",
                10u32,
                2147483648f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D (way too large - trivial)",
                7u32,
                4294967296f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
            )),
            // ROUND.L.D is 5 cycles. In the exception case, subnormal, nan and too-large-to-fit are trivial
            Box::new((
                "ROUND.L.D",
                5u32,
                0f64,
                0f64,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D",
                5u32,
                1f64,
                0f64,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D",
                5u32,
                123.15f64,
                0f64,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D",
                5u32,
                -123f64,
                0f64,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D",
                10u32,
                0.5f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D (too large)",
                7u32,
                9007199254740992f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D (way too large)",
                7u32,
                i64::MAX as f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D (way way way too large)",
                7u32,
                f64::MAX,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D (inf)",
                7u32,
                f64::INFINITY,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "ROUND.L.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_round_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D",
                5u32,
                0f64,
                0f64,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D",
                5u32,
                100f64,
                0f64,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D",
                5u32,
                -100f64,
                0f64,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D",
                10u32,
                0.5f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D (too large)",
                7u32,
                9007199254740992f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D (way too large)",
                7u32,
                i64::MAX as f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D (way way way too large)",
                7u32,
                f64::MAX,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D (inf)",
                7u32,
                f64::INFINITY,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "TRUNC.L.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_trunc_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D",
                5u32,
                0f64,
                0f64,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D",
                5u32,
                10f64,
                0f64,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D",
                5u32,
                -10f64,
                0f64,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D",
                10u32,
                0.5f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D (too large)",
                7u32,
                9007199254740992f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D (way too large)",
                7u32,
                i64::MAX as f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D (way way way too large)",
                7u32,
                f64::MAX,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D (inf)",
                7u32,
                f64::INFINITY as f64 * 1024f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CEIL.L.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_ceil_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D",
                5u32,
                0f64,
                0f64,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D",
                5u32,
                15f64,
                0f64,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D",
                5u32,
                -15f64,
                0f64,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D",
                10u32,
                0.5f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D (too large)",
                7u32,
                9007199254740992f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D (way too large)",
                7u32,
                i64::MAX as f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D (way way way too large)",
                7u32,
                f64::MAX,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D (inf)",
                7u32,
                f64::INFINITY,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "FLOOR.L.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_floor_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D",
                5u32,
                0f64,
                0f64,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D",
                5u32,
                1f64,
                0f64,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D",
                5u32,
                -1.5f64,
                0f64,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D",
                10u32,
                0.5f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D (too large)",
                7u32,
                9007199254740992f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D (way too large)",
                7u32,
                i64::MAX as f64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D (way way way too large)",
                7u32,
                f64::MAX,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D (inf)",
                7u32,
                f64::INFINITY,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
            )),
            // CVT.D.S is always 2 cycle
            Box::new((
                "CVT.S.D",
                2u32,
                0f64,
                0f64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D",
                2u32,
                f64::INFINITY,
                0f64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D",
                2u32,
                0.1111f64,
                0f64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D",
                2u32,
                -0.1111f64,
                0f64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D",
                2u32,
                f64::MAX,
                0f64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D",
                2u32,
                f64::INFINITY,
                0f64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D",
                2u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D",
                7u32,
                FConst::SUBNORMAL_MIN_POSITIVE_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D",
                7u32,
                f64::MAX,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D",
                7u32,
                FConst::QUIET_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D",
                7u32,
                FConst::SIGNALLING_NAN_START_64,
                0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
            )),
            // CVT.D.W has one trivial case: 0. Otherwise it needs 5 cycles. There are no exceptions
            Box::new((
                "CVT.D.W",
                2u32,
                0i32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.D.W",
                5u32,
                i32::MAX,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.D.W",
                5u32,
                u32::MAX as i32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.D.W",
                5u32,
                1i32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.D.W",
                5u32,
                2i32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.D.W",
                5u32,
                -2i32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).w(),
            )),
            Box::new((
                "CVT.D.W",
                5u32,
                1234567891i32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).w(),
            )),
            // CVT.D.L has one trivial case: 0. Otherwise it needs 5 cycles. In the exception case, too large is trivial, but inexact is not
            Box::new((
                "CVT.D.L",
                2u32,
                0i64,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.D.L",
                5u32,
                1i64,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.D.L",
                5u32,
                128i64,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.D.L",
                5u32,
                1i64,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.D.L",
                5u32,
                2i64,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.D.L",
                5u32,
                -2i64,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.D.L",
                5u32,
                1234567891i64,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.D.L (inexact)",
                10u32,
                (1i64 << 55) - 3,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).l(),
            )),
            Box::new((
                "CVT.D.L (too large)",
                7u32,
                1i64 << 55,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).l(),
            )),
            // DMTC1
            Box::new((
                "DMTC1",
                1u32,
                0f64,
                0f64,
                Assembler::make_dmtc1(GPR::V0, FR::F0),
            )),
            // Compares are all single-cycle
            Box::new((
                "C.F.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.F.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.F.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.F.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.F.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.UN.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::UN, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.UN.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::UN, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.UN.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::UN, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.UN.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::UN, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.UN.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::UN, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.EQ.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.EQ.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.EQ.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.EQ.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.EQ.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::EQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.UEQ.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::UEQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.UEQ.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::UEQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.UEQ.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::UEQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.UEQ.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::UEQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.UEQ.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::UEQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLT.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::OLT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLT.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::OLT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLT.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::OLT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLT.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::OLT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLT.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::OLT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.ULT.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::ULT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.ULT.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::ULT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.ULT.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::ULT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.ULT.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::ULT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.ULT.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::ULT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLE.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::OLE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLE.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::OLE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLE.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::OLE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLE.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::OLE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLE.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::OLE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLE.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::ULE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLE.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::ULE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLE.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::Off,
                Assembler::make_cop1_c_cond(Cop1Condition::ULE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLE.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::ULE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.OLE.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::ULE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SF.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SF.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SF.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SF.D",
                6u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SF.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SF.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::SF, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGLE.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGLE.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGLE.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGLE.D",
                6u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGLE.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGLE.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGLE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SEQ.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SEQ.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SEQ.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SEQ.D",
                6u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SEQ.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.SEQ.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::SEQ, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGL.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGL.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGL.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGL.D",
                6u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGL.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGL.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGL, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LT.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LT.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LT.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LT.D",
                6u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LT.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LT.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::LT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGE.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGE.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGE.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGE.D",
                6u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGE.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGE.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LE.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LE.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LE.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LE.D",
                6u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LE.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.LE.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::LE, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGT.D",
                1u32,
                0.0f64,
                -0.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGT.D",
                1u32,
                1.0f64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGT.D",
                1u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGT.D",
                6u32,
                FConst::SIGNALLING_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGT.D",
                1u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "C.NGT.D",
                6u32,
                FConst::QUIET_NAN_START_64,
                1.0f64,
                ExceptionTimingMode::JustFire,
                Assembler::make_cop1_c_cond(Cop1Condition::NGT, FR::F2, FR::F4).d(),
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        set_fcsr(
            fcsr()
                .with_enable_invalid_operation(false)
                .with_flush_denorm_to_zero(true),
        );
        match (*value).downcast_ref::<(&str, u32, f64, f64, u32)>() {
            Some((_context, expected_cycles, value2, value4, instruction)) => {
                let value2_u64 = unsafe { transmute::<f64, u64>(*value2) };
                let value4_u64 = unsafe { transmute::<f64, u64>(*value4) };
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    value2_u64,
                    value4_u64,
                    Status::DEFAULT,
                    ExceptionTimingMode::Off,
                    *instruction,
                );
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, f64, f64, ExceptionTimingMode, u32)>() {
            Some((_context, expected_cycles, value2, value4, exception_mode, instruction)) => {
                set_fcsr(FCSR::DEFAULT.with_enables(FCSRFlags::ALL));
                let value2_u64 = unsafe { transmute::<f64, u64>(*value2) };
                let value4_u64 = unsafe { transmute::<f64, u64>(*value4) };
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    value2_u64,
                    value4_u64,
                    Status::DEFAULT,
                    *exception_mode,
                    *instruction,
                );
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, i32, u32)>() {
            Some((_context, expected_cycles, value2, instruction)) => {
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    *value2 as u64,
                    0,
                    Status::DEFAULT,
                    ExceptionTimingMode::Off,
                    *instruction,
                )
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, i32, ExceptionTimingMode, u32)>() {
            Some((_context, expected_cycles, value2, exception_mode, instruction)) => {
                set_fcsr(FCSR::DEFAULT.with_enables(FCSRFlags::ALL));
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    *value2 as u64,
                    0,
                    Status::DEFAULT,
                    *exception_mode,
                    *instruction,
                );
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, i64, u32)>() {
            Some((_context, expected_cycles, value2, instruction)) => {
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    *value2 as u64,
                    0,
                    Status::DEFAULT,
                    ExceptionTimingMode::Off,
                    *instruction,
                )
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, i64, ExceptionTimingMode, u32)>() {
            Some((_context, expected_cycles, value2, exception_mode, instruction)) => {
                set_fcsr(FCSR::DEFAULT.with_enables(FCSRFlags::ALL));
                return assert_cycles_with_codegen_one_instruction(
                    *expected_cycles,
                    *value2 as u64,
                    0,
                    Status::DEFAULT,
                    *exception_mode,
                    *instruction,
                );
            }
            _ => {}
        }

        return Err(format!("Unexpected pattern"));
    }
}

// Up to four writes go into the write-buffer, so they are expected to be fast
pub struct UncachedWriteBufferTest {}

impl Test for UncachedWriteBufferTest {
    fn name(&self) -> &str {
        "Timing: 4 element write-buffer for uncached memory"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            Box::new(("SB", 1u32, Assembler::make_sb(GPR::R0, 0, GPR::A1))),
            Box::new(("SH", 1u32, Assembler::make_sh(GPR::R0, 0, GPR::A1))),
            Box::new(("SW", 1u32, Assembler::make_sw(GPR::R0, 0, GPR::A1))),
            Box::new(("SD", 1u32, Assembler::make_sd(GPR::R0, 0, GPR::A1))),
            Box::new(("SB", 2u32, Assembler::make_sb(GPR::R0, 0, GPR::A1))),
            Box::new(("SH", 2u32, Assembler::make_sh(GPR::R0, 0, GPR::A1))),
            Box::new(("SW", 2u32, Assembler::make_sw(GPR::R0, 0, GPR::A1))),
            Box::new(("SD", 2u32, Assembler::make_sd(GPR::R0, 0, GPR::A1))),
            Box::new(("SB", 3u32, Assembler::make_sb(GPR::R0, 0, GPR::A1))),
            Box::new(("SH", 3u32, Assembler::make_sh(GPR::R0, 0, GPR::A1))),
            Box::new(("SW", 3u32, Assembler::make_sw(GPR::R0, 0, GPR::A1))),
            Box::new(("SD", 3u32, Assembler::make_sd(GPR::R0, 0, GPR::A1))),
            Box::new(("SB", 4u32, Assembler::make_sb(GPR::R0, 0, GPR::A1))),
            Box::new(("SH", 4u32, Assembler::make_sh(GPR::R0, 0, GPR::A1))),
            Box::new(("SW", 4u32, Assembler::make_sw(GPR::R0, 0, GPR::A1))),
            Box::new(("SD", 4u32, Assembler::make_sd(GPR::R0, 0, GPR::A1))),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(&str, u32, u32)>() {
            Some((_context, number_of_writes, instruction)) => assert_cycles_with_codegen(
                *number_of_writes,
                0,
                0,
                Status::DEFAULT,
                ExceptionTimingMode::Off,
                |writer| {
                    for _ in 0..*number_of_writes as i16 {
                        writer.write(*instruction);
                    }
                },
            ),
            _ => Err(format!("Unexpected pattern")),
        }
    }
}

fn test_register_dependency(
    expected_cycles: u32,
    value2: u64,
    value4: u64,
    instruction1: u32,
    instruction2: u32,
) -> Result<(), String> {
    // Test both instructions directly
    assert_cycles_with_codegen(
        expected_cycles,
        value2,
        value4,
        Status::DEFAULT,
        ExceptionTimingMode::Off,
        |writer| {
            writer.write(instruction1);
            writer.write(instruction2);
        },
    )?;

    // Test first instruction in the delay slot of a branch taken
    assert_cycles_with_codegen(
        expected_cycles + 1,
        value2,
        value4,
        Status::DEFAULT,
        ExceptionTimingMode::Off,
        |writer| {
            // Branch (which is taken)
            writer.write(Assembler::make_beq(GPR::R0, GPR::R0, 4));
            // Delay slot
            writer.write(instruction1);

            writer.write(Assembler::make_nop());
            writer.write(Assembler::make_nop());
            writer.write(Assembler::make_nop());
            writer.write(instruction2);
        },
    )?;

    // Test first instruction in the delay slot of a likely branch taken
    assert_cycles_with_codegen(
        expected_cycles + 1,
        value2,
        value4,
        Status::DEFAULT,
        ExceptionTimingMode::Off,
        |writer| {
            // Branch (which is taken)
            writer.write(Assembler::make_beql(GPR::R0, GPR::R0, 4));
            // Delay slot
            writer.write(instruction1);

            writer.write(Assembler::make_nop());
            writer.write(Assembler::make_nop());
            writer.write(Assembler::make_nop());
            writer.write(instruction2);
        },
    )?;

    // Test first instruction in the delay slot of a branch not taken
    assert_cycles_with_codegen(
        expected_cycles + 1,
        value2,
        value4,
        Status::DEFAULT,
        ExceptionTimingMode::Off,
        |writer| {
            // Branch (which is not taken)
            writer.write(Assembler::make_beq(GPR::V1, GPR::R0, 1));
            // Delay slot
            writer.write(instruction1);

            writer.write(instruction2);
        },
    )?;

    // Test first instruction in the delay slot of a JR
    assert_cycles_with_codegen(
        expected_cycles + 2,
        value2,
        value4,
        Status::DEFAULT,
        ExceptionTimingMode::Off,
        |writer| {
            // T9 is the register that was used to call to here. Add to it to branch a few instructions forward
            writer.write(Assembler::make_addiu(GPR::A2, GPR::T9, 16));
            // Branch (which is taken)
            writer.write(Assembler::make_jr(GPR::A2));
            // Delay slot
            writer.write(instruction1);
            writer.write(Assembler::make_nop());

            // Branch target
            writer.write(instruction2);
        },
    )?;

    Ok(())
}

pub struct CPURegisterDependency {}

impl Test for CPURegisterDependency {
    fn name(&self) -> &str {
        "Timing: CPU register dependency"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            // Purely integer operations do not have dependencies
            Box::new((
                "ORI $A3, $R0, 123; ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_ori(GPR::A3, GPR::R0, 123),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "ADDIU $T0, $A3, 0; ORI $A3, $R0, 123",
                2u32,
                Assembler::make_ori(GPR::A3, GPR::R0, 123),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            // Loads create a dependency: If the next instruction needs the data, it gets delayed by 1
            Box::new((
                "LB $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LBU $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_lbu(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LH $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_lh(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LHU $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_lhu(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LW $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_lw(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LWU $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_lwu(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LD $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_ld(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LWL $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_lwl(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LWR $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_lwr(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LDL $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_ldl(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LDR $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_ldr(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LL $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_ll(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "LLD $A2 (from cached); ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_lld(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            // Loads create a dependency: If the next instruction doesn't need the data, there's no delay
            Box::new((
                "LB $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LBU $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_lbu(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LH $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_lh(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LHU $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_lhu(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LW $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_lw(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LWU $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_lwu(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LD $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_ld(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LWL $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_lwl(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LWR $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_lwr(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LDL $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_ldl(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LDR $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_ldr(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LL $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_ll(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            Box::new((
                "LLD $A2 (from cached); ADDIU $T0, $A3, 0",
                2u32,
                Assembler::make_lld(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A3, 0),
            )),
            // MFLO/MFHI don't create a data dependency
            Box::new((
                "MFLO $A2; ADDIU $A3, $A3, 0",
                2u32,
                Assembler::make_mflo(GPR::A2),
                Assembler::make_addiu(GPR::A2, GPR::A2, 0),
            )),
            Box::new((
                "MFHI $A2; ADDIU $A3, $A3, 0",
                2u32,
                Assembler::make_mfhi(GPR::A2),
                Assembler::make_addiu(GPR::A2, GPR::A2, 0),
            )),
            // Reads from COP0 create a data dependency
            Box::new((
                "MFC0 $A2; ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_mfc0(GPR::A2, RegisterIndex::Wired),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "MFC0 $A2; ADDIU $A3, $A3, 0",
                2u32,
                Assembler::make_mfc0(GPR::A2, RegisterIndex::Wired),
                Assembler::make_addiu(GPR::A3, GPR::A3, 0),
            )),
            Box::new((
                "DMFC0 $A2; ADDIU $A3, $A2, 0",
                3u32,
                Assembler::make_dmfc0(GPR::A2, RegisterIndex::Wired),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "DMFC0 $A2; ADDIU $A3, $A3, 0",
                2u32,
                Assembler::make_dmfc0(GPR::A2, RegisterIndex::Wired),
                Assembler::make_addiu(GPR::A3, GPR::A3, 0),
            )),
            // Reads from COP1 don't
            Box::new((
                "MFC1 $A2; ADDIU $A3, $A2, 0",
                2u32,
                Assembler::make_mfc1(GPR::A2, FR::F0),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            Box::new((
                "DMFC1 $A2; ADDIU $A3, $A2, 0",
                2u32,
                Assembler::make_dmfc1(GPR::A2, FR::F0),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
            // Loading into R0 doesn't cause a data dependency
            Box::new((
                "LB $R0; ADDIU $A3, $R0, 0",
                2u32,
                Assembler::make_lb(GPR::R0, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::R0, 0),
            )),
            Box::new((
                "LB $R0; ADDIU $R0, $R0, 0",
                2u32,
                Assembler::make_lb(GPR::R0, 0, GPR::V1),
                Assembler::make_addiu(GPR::R0, GPR::R0, 0),
            )),
            // Go through all integer operations to ensure they're all waiting accordingly. Oddly, even output registers cause a data dependency
            Box::new((
                "LB $A2; ADDI $T0, $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_addi(GPR::T0, GPR::A2, 0),
            )),
            Box::new((
                "LB $A2; ADDI $A2, $T0, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_addi(GPR::A2, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; ADDI $T0, $T0, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_addi(GPR::T0, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; ADDIU $T0, $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::A2, 0),
            )),
            Box::new((
                "LB $A2; ADDIU $A2, $T0, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::A2, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; ADDIU $T0, $T0, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_addiu(GPR::T0, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; DADDI $T0, $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_daddi(GPR::T0, GPR::A2, 0),
            )),
            Box::new((
                "LB $A2; DADDI $A2, $T0, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_daddi(GPR::A2, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; DADDI $T0, $T0, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_daddi(GPR::T0, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; DADDIU $T0, $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_daddiu(GPR::T0, GPR::A2, 0),
            )),
            Box::new((
                "LB $A2; DADDIU $A2, $T0, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_daddiu(GPR::A2, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; DADDIU $T0, $T0, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_daddiu(GPR::T0, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; SLTI $T0, $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_slti(GPR::T0, GPR::A2, 0),
            )),
            Box::new((
                "LB $A2; SLTI $A2, $T0, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_slti(GPR::A2, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; SLTI $T0, $T0, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_slti(GPR::T0, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; SLTIU $T0, $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sltiu(GPR::T0, GPR::A2, 0),
            )),
            Box::new((
                "LB $A2; SLTIU $A2, $T0, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sltiu(GPR::A2, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; SLTIU $T0, $T0, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sltiu(GPR::T0, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; ANDI $T0, $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_andi(GPR::T0, GPR::A2, 0),
            )),
            Box::new((
                "LB $A2; ANDI $A2, $T0, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_andi(GPR::A2, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; ANDI $T0, $T0, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_andi(GPR::T0, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; ORI $T0, $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ori(GPR::T0, GPR::A2, 0),
            )),
            Box::new((
                "LB $A2; ORI $A2, $T0, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ori(GPR::A2, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; ORI $T0, $T0, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ori(GPR::T0, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; XORI $T0, $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_xori(GPR::T0, GPR::A2, 0),
            )),
            Box::new((
                "LB $A2; XORI $A2, $T0, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_xori(GPR::A2, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; XORI $T0, $T0, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_xori(GPR::T0, GPR::T0, 0),
            )),
            Box::new((
                "LB $A2; LUI $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_lui(GPR::A2, 0),
            )),
            Box::new((
                "LB $A2; LUI $A3, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_lui(GPR::A3, 0),
            )),
            Box::new((
                "LB $A2; LUI $A3, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_lui_with_rs(GPR::A3, GPR::A2, 0),
            )),
            // MTC0 always has a dependency on $4 as MTC1 is the 4th instruction
            Box::new((
                "LB $A2; MTC0 $A2, _Unused7",
                4u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mtc0(GPR::A2, RegisterIndex::_Unused7),
            )),
            Box::new((
                "LB $A2; MTC0 $A3, _Unused7",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mtc0(GPR::A3, RegisterIndex::_Unused7),
            )),
            Box::new((
                "LB $A0; MTC0 $R0, _Unused7",
                4u32,
                Assembler::make_lb(GPR::A0, 0, GPR::V1),
                Assembler::make_mtc0(GPR::R0, RegisterIndex::_Unused7),
            )),
            Box::new((
                "LB $A1; MTC0 $R0, _Unused7",
                3u32,
                Assembler::make_lb(GPR::A1, 0, GPR::V1),
                Assembler::make_mtc0(GPR::R0, RegisterIndex::_Unused7),
            )),
            // DMTC0 always has a dependency on $4 as MTC1 is the 4th instruction
            Box::new((
                "LB $A2; DMTC0 $A2, _Unused7",
                4u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmtc0(GPR::A2, RegisterIndex::_Unused7),
            )),
            Box::new((
                "LB $A2; DMTC0 $A3, _Unused7",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmtc0(GPR::A3, RegisterIndex::_Unused7),
            )),
            Box::new((
                "LB $A0; DMTC0 $R0, _Unused7",
                3u32,
                Assembler::make_lb(GPR::A0, 0, GPR::V1),
                Assembler::make_dmtc0(GPR::R0, RegisterIndex::_Unused7),
            )),
            Box::new((
                "LB $A1; DMTC0 $R0, _Unused7",
                4u32,
                Assembler::make_lb(GPR::A1, 0, GPR::V1),
                Assembler::make_dmtc0(GPR::R0, RegisterIndex::_Unused7),
            )),
            Box::new((
                "LB $A2; MFC0 $A2, _Unused7",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mfc0(GPR::A2, RegisterIndex::_Unused7),
            )),
            Box::new((
                "LB $A2; MFC0 $A3, _Unused7",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mfc0(GPR::A3, RegisterIndex::_Unused7),
            )),
            Box::new((
                "LB $A2; DMFC0 $A2, _Unused7",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmfc0(GPR::A2, RegisterIndex::_Unused7),
            )),
            Box::new((
                "LB $A2; DMFC0 $A3, _Unused7",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmfc0(GPR::A3, RegisterIndex::_Unused7),
            )),
            // MTC1 doesn't have the weird behavior that MTC0 has
            Box::new((
                "LB $A2; MTC1 $A2, F0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mtc1(GPR::A2, FR::F0),
            )),
            Box::new((
                "LB $A0; MTC1 $A3, F6",
                2u32,
                Assembler::make_lb(GPR::A0, 0, GPR::V1),
                Assembler::make_mtc1(GPR::A3, FR::F6),
            )),
            Box::new((
                "LB $A1; MTC1 $A3, F6",
                2u32,
                Assembler::make_lb(GPR::A1, 0, GPR::V1),
                Assembler::make_mtc1(GPR::A3, FR::F6),
            )),
            Box::new((
                "LB $A2; MTC1 $A3, F6",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mtc1(GPR::A3, FR::F6),
            )),
            Box::new((
                "LB $A2; MFC1 $A2, F0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mfc1(GPR::A2, FR::F0),
            )),
            Box::new((
                "LB $A2; MFC1 $A3, F6",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mfc1(GPR::A3, FR::F6),
            )),
            Box::new((
                "LB $A2; CFC1 $A2, 31",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_cfc1(GPR::A2, u5::new(31)),
            )),
            Box::new((
                "LB $A2; CFC1 $A3, 31",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_cfc1(GPR::A3, u5::new(31)),
            )),
            Box::new((
                "LB $V0; CFC1 $A3, 31",
                2u32,
                Assembler::make_lb(GPR::V0, 0, GPR::V1),
                Assembler::make_cfc1(GPR::A3, u5::new(31)),
            )),
            Box::new((
                "LB $A2; DMTC1 $A2, F0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmtc1(GPR::A2, FR::F0),
            )),
            Box::new((
                "LB $A2; DMTC1 $A3, F6",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmtc1(GPR::A3, FR::F6),
            )),
            Box::new((
                "LB $A2; DMFC1 $A2, F0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmfc1(GPR::A2, FR::F0),
            )),
            Box::new((
                "LB $A2; DMFC1 $A3, F6",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmfc1(GPR::A3, FR::F6),
            )),
            // Write into 0, which is read-only
            Box::new((
                "LB $A2; CTC1 $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ctc1(GPR::A2, u5::new(0)),
            )),
            Box::new((
                "LB $A2; CTC1 $A3, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ctc1(GPR::A3, u5::new(0)),
            )),
            Box::new((
                "LB $A2; CTC1 $A2, 0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ctc1(GPR::A2, u5::new(0)),
            )),
            Box::new((
                "LB $A2; CTC1 $A3, 0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ctc1(GPR::A3, u5::new(0)),
            )),
            // MFLO and MFHI doesn't create a dependency based on the target reg
            Box::new((
                "LB $A2; MFLO $A2",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mflo(GPR::A2),
            )),
            Box::new((
                "LB $A2; MFLO $A3",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mflo(GPR::A3),
            )),
            Box::new((
                "LB $A2; MFHI $A2",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mfhi(GPR::A2),
            )),
            Box::new((
                "LB $A2; MFHI $A3",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mfhi(GPR::A3),
            )),
            // (D)MULT(U) and (D)DIV(U) don't create a dependency on LO/HI
            Box::new((
                "MULT $0, $0; MFLO $A2",
                6u32,
                Assembler::make_mult(GPR::R0, GPR::R0),
                Assembler::make_mflo(GPR::A2),
            )),
            Box::new((
                "MULTU $0, $0; MFLO $A2",
                6u32,
                Assembler::make_multu(GPR::R0, GPR::R0),
                Assembler::make_mflo(GPR::A2),
            )),
            Box::new((
                "DMULT $0, $0; MFLO $A2",
                9u32,
                Assembler::make_dmult(GPR::R0, GPR::R0),
                Assembler::make_mflo(GPR::A2),
            )),
            Box::new((
                "DMULTU $0, $0; MFLO $A2",
                9u32,
                Assembler::make_dmultu(GPR::R0, GPR::R0),
                Assembler::make_mflo(GPR::A2),
            )),
            Box::new((
                "MULT $0, $0; MFHI $A2",
                6u32,
                Assembler::make_mult(GPR::R0, GPR::R0),
                Assembler::make_mfhi(GPR::A2),
            )),
            Box::new((
                "MULTU $0, $0; MFHI $A2",
                6u32,
                Assembler::make_multu(GPR::R0, GPR::R0),
                Assembler::make_mfhi(GPR::A2),
            )),
            Box::new((
                "DMULT $0, $0; MFHI $A2",
                9u32,
                Assembler::make_dmult(GPR::R0, GPR::R0),
                Assembler::make_mfhi(GPR::A2),
            )),
            Box::new((
                "DMULTU $0, $0; MFHI $A2",
                9u32,
                Assembler::make_dmultu(GPR::R0, GPR::R0),
                Assembler::make_mfhi(GPR::A2),
            )),
            Box::new((
                "DIV $0, $0; MFLO $A2",
                38u32,
                Assembler::make_div(GPR::R0, GPR::R0),
                Assembler::make_mflo(GPR::A2),
            )),
            Box::new((
                "DIVU $0, $0; MFLO $A2",
                38u32,
                Assembler::make_divu(GPR::R0, GPR::R0),
                Assembler::make_mflo(GPR::A2),
            )),
            Box::new((
                "DDIV $0, $0; MFLO $A2",
                70u32,
                Assembler::make_ddiv(GPR::R0, GPR::R0),
                Assembler::make_mflo(GPR::A2),
            )),
            Box::new((
                "DDIVU $0, $0; MFLO $A2",
                70u32,
                Assembler::make_ddivu(GPR::R0, GPR::R0),
                Assembler::make_mflo(GPR::A2),
            )),
            Box::new((
                "DIV $0, $0; MFHI $A2",
                38u32,
                Assembler::make_div(GPR::R0, GPR::R0),
                Assembler::make_mfhi(GPR::A2),
            )),
            Box::new((
                "DIVU $0, $0; MFHI $A2",
                38u32,
                Assembler::make_divu(GPR::R0, GPR::R0),
                Assembler::make_mfhi(GPR::A2),
            )),
            Box::new((
                "DDIV $0, $0; MFHI $A2",
                70u32,
                Assembler::make_ddiv(GPR::R0, GPR::R0),
                Assembler::make_mfhi(GPR::A2),
            )),
            Box::new((
                "DDIVU $0, $0; MFHI $A2",
                70u32,
                Assembler::make_ddivu(GPR::R0, GPR::R0),
                Assembler::make_mfhi(GPR::A2),
            )),
            Box::new((
                "LB $A2; MTLO $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mtlo(GPR::A2),
            )),
            Box::new((
                "LB $A2; MTLO $A3",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mtlo(GPR::A3),
            )),
            Box::new((
                "LB $A2; MTLO $A3 (with rd=$A2)",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mtlo_with_extras(GPR::A2, GPR::A3, GPR::R0),
            )),
            Box::new((
                "LB $A2; MTLO $A3 (with rt=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mtlo_with_extras(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; MTHI $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mthi(GPR::A2),
            )),
            Box::new((
                "LB $A2; MTHI $A3",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mthi(GPR::A3),
            )),
            Box::new((
                "LB $A2; MTHI $A3 (with rd=$A2)",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mthi_with_extras(GPR::A2, GPR::A3, GPR::R0),
            )),
            Box::new((
                "LB $A2; MTHI $A3 (with rt=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mthi_with_extras(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; MFLO $A2",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mflo(GPR::A2),
            )),
            Box::new((
                "LB $A2; MFLO $A3",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mflo(GPR::A3),
            )),
            Box::new((
                "LB $A2; MFLO $A3 (with rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mflo_with_extras(GPR::A3, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; MFLO $A3 (with rt=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mflo_with_extras(GPR::A3, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; MFHI $A2",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mfhi(GPR::A2),
            )),
            Box::new((
                "LB $A2; MFHI $A3",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mfhi(GPR::A3),
            )),
            Box::new((
                "LB $A2; MFHI $A3 (with rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mfhi_with_extras(GPR::A3, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; MFHI $A3 (with rt=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mfhi_with_extras(GPR::A3, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; MULT $A2, $R0",
                7u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mult(GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; MULT $R0, $A2",
                7u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mult(GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; MULT $R0, $R0",
                6u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_mult(GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; MULTU $A2, $R0",
                7u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_multu(GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; MULTU $R0, $A2",
                7u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_multu(GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; MULTU $R0, $R0",
                6u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_multu(GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DMULT $A2, $R0",
                10u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmult(GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DMULT $R0, $A2",
                10u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmult(GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DMULT $R0, $R0",
                9u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmult(GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DMULTU $A2, $R0",
                10u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmultu(GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DMULTU $R0, $A2",
                10u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmultu(GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DMULTU $R0, $R0",
                9u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dmultu(GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DIV $A2, $R0",
                39u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_div(GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DIV $R0, $A2",
                39u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_div(GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DIV $R0, $R0",
                38u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_div(GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DIVU $A2, $R0",
                39u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_divu(GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DIVU $R0, $A2",
                39u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_divu(GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DIVU $R0, $R0",
                38u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_divu(GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DDIV $A2, $R0",
                71u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ddiv(GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DDIV $R0, $A2",
                71u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ddiv(GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DDIV $R0, $R0",
                70u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ddiv(GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DDIVU $A2, $R0",
                71u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ddivu(GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DDIVU $R0, $A2",
                71u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ddivu(GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DDIVU $R0, $R0",
                70u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_ddivu(GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; ADD $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_add(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; ADD $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_add(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; ADD $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_add(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; ADDU $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_addu(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; ADDU $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_addu(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; ADDU $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_addu(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; SUB $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sub(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; SUB $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sub(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; SUB $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sub(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; SUBU $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_subu(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; SUBU $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_subu(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; SUBU $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_subu(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; AND $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_and(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; AND $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_and(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; AND $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_and(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; OR $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_or(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; OR $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_or(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; OR $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_or(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; XOR $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_xor(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; XOR $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_xor(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; XOR $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_xor(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; NOR $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_nor(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; NOR $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_nor(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; NOR $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_nor(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; SLT $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_slt(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; SLT $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_slt(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; SLT $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_slt(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; SLTU $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sltu(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; SLTU $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sltu(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; SLTU $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sltu(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DADD $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dadd(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DADD $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dadd(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DADD $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dadd(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DADDU $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_daddu(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DADDU $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_daddu(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DADDU $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_daddu(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DSUB $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsub(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DSUB $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsub(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DSUB $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsub(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DSUBU $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsubu(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DSUBU $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsubu(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DSUBU $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsubu(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; SLLV $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sllv(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; SLLV $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sllv(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; SLLV $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sllv(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; SRLV $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_srlv(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; SRLV $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_srlv(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; SRLV $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_srlv(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; SRAV $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_srav(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; SRAV $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_srav(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; SRAV $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_srav(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DSLLV $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsllv(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DSLLV $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsllv(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DSLLV $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsllv(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DSRLV $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrlv(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DSRLV $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrlv(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DSRLV $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrlv(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; DSRAV $R0, $R0, $A2",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrav(GPR::R0, GPR::R0, GPR::A2),
            )),
            Box::new((
                "LB $A2; DSRAV $R0, $A2, $R0",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrav(GPR::R0, GPR::A2, GPR::R0),
            )),
            Box::new((
                "LB $A2; DSRAV $A2, $R0, $R0",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrav(GPR::A2, GPR::R0, GPR::R0),
            )),
            Box::new((
                "LB $A2; SLL $A2, $R0, 1",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sll(GPR::A2, GPR::R0, u5::new(1)),
            )),
            Box::new((
                "LB $A2; SLL $R0, $A2, 1",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sll(GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; SLL $R0, $R0, 1 (rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sll_with_extras(GPR::R0, GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; SRL $A2, $R0, 1",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_srl(GPR::A2, GPR::R0, u5::new(1)),
            )),
            Box::new((
                "LB $A2; SRL $R0, $A2, 1",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_srl(GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; SRL $R0, $R0, 1 (rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_srl_with_extras(GPR::R0, GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; SRA $A2, $R0, 1",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sra(GPR::A2, GPR::R0, u5::new(1)),
            )),
            Box::new((
                "LB $A2; SRA $R0, $A2, 1",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sra(GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; SRA $R0, $R0, 1 (rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sra_with_extras(GPR::R0, GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSLL $A2, $R0, 1",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsll(GPR::A2, GPR::R0, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSLL $R0, $A2, 1",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsll(GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSLL $R0, $R0, 1 (rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsll_with_extras(GPR::R0, GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRL $A2, $R0, 1",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrl(GPR::A2, GPR::R0, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRL $R0, $A2, 1",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrl(GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRL $R0, $R0, 1 (rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrl_with_extras(GPR::R0, GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRA $A2, $R0, 1",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsra(GPR::A2, GPR::R0, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRA $R0, $A2, 1",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsra(GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRA $R0, $R0, 1 (rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsra_with_extras(GPR::R0, GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSLL32 $A2, $R0, 1",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsll32(GPR::A2, GPR::R0, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSLL32 $R0, $A2, 1",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsll32(GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSLL32 $R0, $R0, 1 (rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsll32_with_extras(GPR::R0, GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRL32 $A2, $R0, 1",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrl32(GPR::A2, GPR::R0, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRL32 $R0, $A2, 1",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrl32(GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRL32 $R0, $R0, 1 (rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsrl32_with_extras(GPR::R0, GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRA32 $A2, $R0, 1",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsra32(GPR::A2, GPR::R0, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRA32 $R0, $A2, 1",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsra32(GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; DSRA32 $R0, $R0, 1 (rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_dsra32_with_extras(GPR::R0, GPR::R0, GPR::A2, u5::new(1)),
            )),
            // Even SYNC has a dependency on rs and rt. This makes it sound like all SPECIAL instructions do (even SYSCALL and BREAK), which are harder to test
            Box::new((
                "LB $A2; SYNC (rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sync_with_extras(GPR::R0, GPR::A2, GPR::R0, u5::new(1)),
            )),
            Box::new((
                "LB $A2; SYNC (rt=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sync_with_extras(GPR::R0, GPR::R0, GPR::A2, u5::new(1)),
            )),
            Box::new((
                "LB $A2; SYNC (rd=$A2)",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_sync_with_extras(GPR::A2, GPR::R0, GPR::R0, u5::new(1)),
            )),
            // Regimm: These are wild. Rt contains the regimm instruction (e.g. TLTI=10), but is still used for dependencies
            Box::new((
                "LB $A2; TGEI (rs=$A2)",
                3u32,
                Assembler::make_lbu(GPR::A2, 0, GPR::V1),
                Assembler::make_regimm_trap(RegimmOpcode::TGEI, GPR::A2.raw_value(), 32767),
            )),
            Box::new((
                "LB $A2; TGEI (rs=$R0)",
                2u32,
                Assembler::make_lbu(GPR::A2, 0, GPR::V1),
                Assembler::make_regimm_trap(RegimmOpcode::TGEI, GPR::R0.raw_value(), 32767),
            )),
            Box::new((
                "LB $T2; TGEI (rs=$A3)",
                3u32,
                Assembler::make_lbu(GPR::T0, 0, GPR::V1),
                Assembler::make_regimm_trap(RegimmOpcode::TGEI, GPR::R0.raw_value(), 32767),
            )),
            Box::new((
                "LB $A2; TLTI (rs=$A2)",
                3u32,
                Assembler::make_lbu(GPR::A2, 0, GPR::V1),
                Assembler::make_regimm_trap(RegimmOpcode::TLTI, GPR::A2.raw_value(), 0),
            )),
            Box::new((
                "LB $A2; TLTI (rs=$R0)",
                2u32,
                Assembler::make_lbu(GPR::A2, 0, GPR::V1),
                Assembler::make_regimm_trap(RegimmOpcode::TLTI, GPR::R0.raw_value(), 0),
            )),
            Box::new((
                "LB $T2; TLTI (rs=$A3)",
                3u32,
                Assembler::make_lbu(GPR::T2, 0, GPR::V1),
                Assembler::make_regimm_trap(RegimmOpcode::TLTI, GPR::R0.raw_value(), 0),
            )),
            Box::new((
                "LB $A2; TLTIU (rs=$A2)",
                3u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_regimm_trap(RegimmOpcode::TLTIU, GPR::A2.raw_value(), 0),
            )),
            Box::new((
                "LB $A2; TLTIU (rs=$A3)",
                2u32,
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_regimm_trap(RegimmOpcode::TLTIU, GPR::R0.raw_value(), 0),
            )),
            Box::new((
                "LB $T3; TLTIU (rs=$A3)",
                3u32,
                Assembler::make_lb(GPR::T3, 0, GPR::V1),
                Assembler::make_regimm_trap(RegimmOpcode::TLTIU, GPR::R0.raw_value(), 0),
            )),
            // Load after load
            Box::new((
                "LD $T3; LD $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ld(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LD $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LD $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ld(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LW $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lw(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LW $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lw(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LW $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lw(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LWU $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwu(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LWU $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwu(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LWU $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwu(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LH $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lh(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LH $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lh(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LH $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lh(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LHU $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lhu(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LHU $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lhu(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LHU $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lhu(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LBU $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lbu(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LBU $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lbu(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LBU $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lbu(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LB $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lb(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LB $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lb(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LB $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lb(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LWL $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwl(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LWL $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwl(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LWL $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwl(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LWR $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwr(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LWR $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwr(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LWR $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwr(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LDL $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ldl(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LDL $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ldl(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LDL $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ldl(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LDR $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ldr(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LDR $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ldr(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LDR $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ldr(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LL $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ll(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LL $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ll(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LL $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ll(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LLD $T4, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lld(GPR::T4, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LLD $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lld(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LLD $R0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lld(GPR::R0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LWC1 $F12, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwc1(FR::F12, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LWC1 $F11, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwc1(FR::F11, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LWC1 $F0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_lwc1(FR::F0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; LDC1 $F12, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ldc1(FR::F12, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LDC1 $F11, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ldc1(FR::F11, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; LDC1 $F0, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_ldc1(FR::F0, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; SD $V1, 0($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sd(GPR::V1, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; SD $T3, 0($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sd(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LD $T3; SD $V1, 0($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sd(GPR::V1, 0, GPR::T3),
            )),
            Box::new((
                "LD $T3; SW $V1, 4($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sw(GPR::V1, 4, GPR::V1),
            )),
            Box::new((
                "LD $T3; SW $T3, 4($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sw(GPR::T3, 4, GPR::V1),
            )),
            Box::new((
                "LD $T3; SW $V1, 4($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sw(GPR::V1, 4, GPR::T3),
            )),
            Box::new((
                "LD $T3; SH $V1, 6($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sh(GPR::V1, 6, GPR::V1),
            )),
            Box::new((
                "LD $T3; SH $T3, 6($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sh(GPR::T3, 6, GPR::V1),
            )),
            Box::new((
                "LD $T3; SH $V1, 6($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sh(GPR::V1, 6, GPR::T3),
            )),
            Box::new((
                "LD $T3; SB $V1, 7($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sb(GPR::V1, 7, GPR::V1),
            )),
            Box::new((
                "LD $T3; SB $T3, 7($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sb(GPR::T3, 7, GPR::V1),
            )),
            Box::new((
                "LD $T3; SB $V1, 7($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sb(GPR::V1, 7, GPR::T3),
            )),
            Box::new((
                "LD $T3; SWR $V1, 8($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_swr(GPR::V1, 8, GPR::V1),
            )),
            Box::new((
                "LD $T3; SWR $T3, 8($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_swr(GPR::T3, 8, GPR::V1),
            )),
            Box::new((
                "LD $T3; SWR $V1, 8($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_swr(GPR::V1, 8, GPR::T3),
            )),
            Box::new((
                "LD $T3; SWL $V1, 8($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_swl(GPR::V1, 8, GPR::V1),
            )),
            Box::new((
                "LD $T3; SWL $T3, 8($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_swl(GPR::T3, 8, GPR::V1),
            )),
            Box::new((
                "LD $T3; SWL $V1, 8($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_swl(GPR::V1, 8, GPR::T3),
            )),
            Box::new((
                "LD $T3; SDR $V1, 8($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sdr(GPR::V1, 8, GPR::V1),
            )),
            Box::new((
                "LD $T3; SDR $T3, 8($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sdr(GPR::T3, 8, GPR::V1),
            )),
            Box::new((
                "LD $T3; SDR $V1, 8($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sdr(GPR::V1, 8, GPR::T3),
            )),
            Box::new((
                "LD $T3; SDL $V1, 8($V1)",
                2u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sdl(GPR::V1, 8, GPR::V1),
            )),
            Box::new((
                "LD $T3; SDL $T3, 8($V1)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sdl(GPR::T3, 8, GPR::V1),
            )),
            Box::new((
                "LD $T3; SDL $V1, 8($T3)",
                3u32,
                Assembler::make_ld(GPR::T3, 0, GPR::V1),
                Assembler::make_sdl(GPR::V1, 8, GPR::T3),
            )),
            Box::new((
                "LLD $T3; SCD $A3, 0($V1)",
                2u32,
                Assembler::make_lld(GPR::T3, 0, GPR::V1),
                Assembler::make_scd(GPR::A3, 0, GPR::V1),
            )),
            Box::new((
                "LLD $T3; SCD $T3, 0($V1)",
                3u32,
                Assembler::make_lld(GPR::T3, 0, GPR::V1),
                Assembler::make_scd(GPR::T3, 0, GPR::V1),
            )),
            Box::new((
                "LLD $T3; SCD $A3, 0($T3)",
                3u32,
                Assembler::make_lld(GPR::T3, 0, GPR::V1),
                Assembler::make_scd(GPR::A3, 0, GPR::T3),
            )),
            Box::new((
                "LLD $T3; SC $A3, 4($V1)",
                2u32,
                Assembler::make_lld(GPR::T3, 0, GPR::V1),
                Assembler::make_sc(GPR::A3, 4, GPR::V1),
            )),
            Box::new((
                "LLD $T3; SC $T3, 4($V1)",
                3u32,
                Assembler::make_lld(GPR::T3, 0, GPR::V1),
                Assembler::make_sc(GPR::T3, 4, GPR::V1),
            )),
            Box::new((
                "LLD $T3; SC $A3, 4($T3)",
                3u32,
                Assembler::make_lld(GPR::T3, 0, GPR::V1),
                Assembler::make_sc(GPR::A3, 4, GPR::T3),
            )),
            Box::new((
                "LD $T4; SDC1 F12, 8($V1)",
                2u32,
                Assembler::make_ld(GPR::T4, 0, GPR::V1),
                Assembler::make_sdc1(FR::F12, 8, GPR::V1),
            )),
            Box::new((
                "LD $T4; SDC1 F0, 8($T4)",
                3u32,
                Assembler::make_ld(GPR::T4, 0, GPR::V1),
                Assembler::make_sdc1(FR::F0, 8, GPR::T4),
            )),
            Box::new((
                "LD $T4; SWC1 F12, 12($V1)",
                2u32,
                Assembler::make_ld(GPR::T4, 0, GPR::V1),
                Assembler::make_swc1(FR::F12, 12, GPR::V1),
            )),
            Box::new((
                "LD $T4; SWC1 F0, 12($T4)",
                3u32,
                Assembler::make_ld(GPR::T4, 0, GPR::V1),
                Assembler::make_swc1(FR::F0, 12, GPR::T4),
            )),
            Box::new((
                "LD $T4; CACHE (DataIndexLoadTag) $V1",
                7u32,
                Assembler::make_ld(GPR::T4, 0, GPR::V1),
                Assembler::make_cache(CacheOp::DataIndexLoadTag, 0, GPR::V1),
            )),
            Box::new((
                "LD $A0; CACHE (DataIndexLoadTag) $V1",
                8u32,
                Assembler::make_ld(GPR::A1, 0, GPR::V1),
                Assembler::make_cache(CacheOp::DataIndexLoadTag, 0, GPR::V1),
            )),
            Box::new((
                "LD $T4; CACHE (DataIndexLoadTag) $T4",
                8u32,
                Assembler::make_ld(GPR::T4, 0, GPR::V1),
                Assembler::make_cache(CacheOp::DataIndexLoadTag, 0, GPR::T4),
            )),
            Box::new((
                "LD $T4; NOP; BEQ $T4, $R0; NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_beq(GPR::T4, GPR::R0, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BEQ $T4, $R0; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_beq(GPR::T4, GPR::R0, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BEQ $R0, $T4; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_beq(GPR::R0, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BEQ $T4, $T4; NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_beq(GPR::T4, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BEQ $T4, $T4; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_beq(GPR::T4, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BEQL $T4, $R0; NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_beql(GPR::T4, GPR::R0, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BEQL $T4, $R0; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_beql(GPR::T4, GPR::R0, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BEQL $R0, $T4; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_beql(GPR::R0, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BEQL $T4, $T4; NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_beql(GPR::T4, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BEQL $T4, $T4; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_beql(GPR::T4, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BNE $T4, $R0; NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_bne(GPR::T4, GPR::R0, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BNE $T4, $R0; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_bne(GPR::T4, GPR::R0, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BNE $R0, $T4; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_bne(GPR::R0, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BNE $T4, $T4; NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_bne(GPR::T4, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BNE $T4, $T4; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_bne(GPR::T4, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BNEL $T4, $R0; NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_bnel(GPR::T4, GPR::R0, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BNEL $T4, $R0; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_bnel(GPR::T4, GPR::R0, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BNEL $R0, $T4; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_bnel(GPR::R0, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BNEL $T4, $T4; NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_bnel(GPR::T4, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BNEL $T4, $T4; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_bnel(GPR::T4, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BLEZ $T4, NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_blez(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BLEZ $T4, NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 24, GPR::V1),
                    Assembler::make_blez(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BLEZ $R0, NOP [rt=$T4]",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_blez_with_extras(GPR::R0, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BLEZ $T4, NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_blez(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BLEZ $T4, NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 32, GPR::V1),
                    Assembler::make_blez(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BLEZL $T4, NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_blezl(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BLEZL $T4, NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 24, GPR::V1),
                    Assembler::make_blezl(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BLEZL $R0, NOP [rt=$T4]",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_blezl_with_extras(GPR::R0, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BLEZL $T4, NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_blezl(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BLEZL $T4, NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 32, GPR::V1),
                    Assembler::make_blezl(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BGTZ $T4, NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_bgtz(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BGTZ $T4, NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 24, GPR::V1),
                    Assembler::make_bgtz(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BGTZ $R0, NOP [rt=$T4]",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_bgtz_with_extras(GPR::R0, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BGTZ $T4, NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_bgtz(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BGTZ $T4, NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 32, GPR::V1),
                    Assembler::make_bgtz(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BGTZL $T4, NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_bgtzl(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BGTZL $T4, NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 24, GPR::V1),
                    Assembler::make_bgtzl(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BGTZL $R0, NOP [rt=$T4]",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_bgtzl_with_extras(GPR::R0, GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; BGTZL $T4, NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_bgtzl(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; BGTZL $T4, NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 32, GPR::V1),
                    Assembler::make_bgtzl(GPR::T4, 1),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "LD $T4; NOP; JR $T4; NOP",
                4u32,
                vec![
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_nop(),
                    Assembler::make_jr(GPR::T4),
                    Assembler::make_nop(),
                ],
            )),
            Box::new((
                "NOP; LD $T4; JR $T4; NOP",
                5u32,
                vec![
                    Assembler::make_nop(),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_jr(GPR::T4),
                    Assembler::make_nop(),
                ],
            )),
            // JAL does not stall if the previous instruction just loaded $31
            Box::new((
                "LD $T4; JAL (return-after-one-nop); NOP",
                8u32,
                vec![
                    // Backup RA
                    Assembler::make_ori(GPR::A2, GPR::RA, 0),
                    Assembler::make_ld(GPR::T4, 16, GPR::V1),
                    Assembler::make_jal(u26::extract_u32(
                        one_nop_then_return_function as extern "C" fn() as u32,
                        2,
                    )),
                    Assembler::make_nop(),
                    // Restore RA
                    Assembler::make_ori(GPR::RA, GPR::A2, 0),
                ],
            )),
            Box::new((
                "LD $RA; JAL (return-after-one-nop); NOP",
                8u32,
                vec![
                    // Backup RA
                    Assembler::make_ori(GPR::A2, GPR::RA, 0),
                    Assembler::make_ld(GPR::RA, 16, GPR::V1),
                    Assembler::make_jal(u26::extract_u32(
                        one_nop_then_return_function as extern "C" fn() as u32,
                        2,
                    )),
                    Assembler::make_nop(),
                    // Restore RA
                    Assembler::make_ori(GPR::RA, GPR::A2, 0),
                ],
            )),
            // It also doesn't itself create a dependency (not surprising, but was worth checking)
            Box::new((
                "LD JAL (return-after-one-nop); NOP",
                7u32,
                vec![
                    // Backup RA
                    Assembler::make_ori(GPR::A2, GPR::RA, 0),
                    Assembler::make_jal(u26::extract_u32(
                        one_nop_then_return_function as extern "C" fn() as u32,
                        2,
                    )),
                    Assembler::make_nop(),
                    // Restore RA
                    Assembler::make_ori(GPR::RA, GPR::A2, 0),
                ],
            )),
            Box::new((
                "LD JAL (instant-return); NOP",
                6u32,
                vec![
                    // Backup RA
                    Assembler::make_ori(GPR::A2, GPR::RA, 0),
                    Assembler::make_jal(u26::extract_u32(
                        instant_return_function as extern "C" fn() as u32,
                        2,
                    )),
                    Assembler::make_nop(),
                    // Restore RA
                    Assembler::make_ori(GPR::RA, GPR::A2, 0),
                ],
            )),
            // TODO: Some special instructions (e.g. JALR) are missing; but given that even SYSCALL has a register dependency, this is likely universal code for all special instructions
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(&str, u32, u32, u32)>() {
            Some((_context, expected_cycles, instruction0, instruction1)) => {
                // If there are exactly two instructions, we can do a bigger set of tests that involve placing the first instruction
                return test_register_dependency(
                    *expected_cycles,
                    0x123,
                    0x456,
                    *instruction0,
                    *instruction1,
                );
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, Vec<u32>)>() {
            Some((_context, expected_cycles, instructions)) => {
                // If there are exactly two instructions, we can do a bigger set of tests that involve placing the first instruction
                return assert_cycles_with_codegen(
                    *expected_cycles,
                    0x123,
                    0x456,
                    Status::DEFAULT,
                    ExceptionTimingMode::Off,
                    |writer| {
                        for &instruction in instructions {
                            writer.write(instruction);
                        }
                    },
                );
            }
            _ => {}
        }

        Err(format!("Unexpected pattern"))
    }
}

pub struct COP1RegisterDependency {}

impl Test for COP1RegisterDependency {
    fn name(&self) -> &str {
        "Timing: COP1 register dependency"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            // ** Singles **

            // ADD.S doesn't create a dependency on it's destination register (unlike ADDIU, see above)
            Box::new((
                "ADD.S (no dependency)",
                6u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_add(FR::F6, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S (no dependency 2)",
                6u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S (ft)",
                7u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_add(FR::F6, FR::F2, FR::F0).s(),
            )),
            Box::new((
                "ADD.S (fs)",
                7u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_add(FR::F6, FR::F0, FR::F4).s(),
            )),
            Box::new((
                "SUB.S (no dependency)",
                6u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_sub(FR::F6, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S (no dependency 2)",
                6u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "SUB.S (ft)",
                7u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_sub(FR::F6, FR::F2, FR::F0).s(),
            )),
            Box::new((
                "SUB.S (fs)",
                7u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_sub(FR::F6, FR::F0, FR::F4).s(),
            )),
            Box::new((
                "MUL.S (no dependency)",
                10u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_mul(FR::F6, FR::F4, FR::F4).s(),
            )),
            Box::new((
                "MUL.S (no dependency 2)",
                10u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MUL.S (ft)",
                11u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_mul(FR::F6, FR::F2, FR::F0).s(),
            )),
            Box::new((
                "MUL.S (fs)",
                11u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_mul(FR::F6, FR::F0, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (no dependency)",
                58u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (no dependency 2)",
                58u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DIV.S (ft)",
                59u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F0).s(),
            )),
            Box::new((
                "DIV.S (fs)",
                59u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_div(FR::F6, FR::F0, FR::F4).s(),
            )),
            // Instructions with a single input do something odd: They don't use the ft register, but still creates a dependency on it. Let's try this specifically
            Box::new((
                "SQRT.S (no dependency 1)",
                58u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_sqrt(FR::F6, FR::F2).s(),
                Assembler::make_cop1_sqrt(FR::F8, FR::F4).s(),
            )),
            Box::new((
                "SQRT.S (no dependency 2)",
                58u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_sqrt(FR::F6, FR::F2).s(),
                Assembler::make_cop1_sqrt(FR::F6, FR::F4).s(),
            )),
            Box::new((
                "SQRT.S (no dependency 3)",
                58u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_sqrt(FR::F6, FR::F2).s(),
                Assembler::make_cop1_sqrt(FR::F8, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S (dependency)",
                59u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
                Assembler::make_cop1_sqrt(FR::F6, FR::F0).s(),
            )),
            Box::new((
                "SQRT.S (dependency via ft)",
                59u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
                Assembler::make_cop1_sqrt(FR::F6, FR::F2).s(),
            )),
            Box::new((
                "SQRT.S (non-dependency as ft!=0)",
                58u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).s(),
                Assembler::make_cop1_sqrt_with_ft(FR::F6, FR::F2, FR::F2).s(),
            )),
            Box::new((
                "ABS.S (no dependency 1)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_abs(FR::F6, FR::F2).s(),
                Assembler::make_cop1_abs(FR::F8, FR::F4).s(),
            )),
            Box::new((
                "ABS.S (no dependency 2)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_abs(FR::F6, FR::F2).s(),
                Assembler::make_cop1_abs(FR::F6, FR::F4).s(),
            )),
            Box::new((
                "ABS.S (no dependency 3)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_abs(FR::F6, FR::F2).s(),
                Assembler::make_cop1_abs(FR::F8, FR::F2).s(),
            )),
            Box::new((
                "ABS.S (dependency)",
                3u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
                Assembler::make_cop1_abs(FR::F6, FR::F0).s(),
            )),
            Box::new((
                "ABS.S (dependency via ft)",
                3u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
                Assembler::make_cop1_abs(FR::F6, FR::F2).s(),
            )),
            Box::new((
                "ABS.S (non-dependency as ft!=0)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_abs(FR::F0, FR::F2).s(),
                Assembler::make_cop1_abs_with_ft(FR::F6, FR::F2, FR::F2).s(),
            )),
            Box::new((
                "NEG.S (no dependency 1)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_neg(FR::F6, FR::F2).s(),
                Assembler::make_cop1_neg(FR::F8, FR::F4).s(),
            )),
            Box::new((
                "NEG.S (no dependency 2)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_neg(FR::F6, FR::F2).s(),
                Assembler::make_cop1_neg(FR::F6, FR::F4).s(),
            )),
            Box::new((
                "NEG.S (no dependency 3)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_neg(FR::F6, FR::F2).s(),
                Assembler::make_cop1_neg(FR::F8, FR::F2).s(),
            )),
            Box::new((
                "NEG.S (dependency)",
                3u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
                Assembler::make_cop1_neg(FR::F6, FR::F0).s(),
            )),
            Box::new((
                "NEG.S (dependency via ft)",
                3u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
                Assembler::make_cop1_neg(FR::F6, FR::F2).s(),
            )),
            Box::new((
                "NEG.S (non-dependency as ft!=0)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_neg(FR::F0, FR::F2).s(),
                Assembler::make_cop1_neg_with_ft(FR::F6, FR::F2, FR::F2).s(),
            )),
            Box::new((
                "MOV.S (no dependency 1)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_mov(FR::F6, FR::F2).s(),
                Assembler::make_cop1_mov(FR::F8, FR::F4).s(),
            )),
            Box::new((
                "MOV.S (no dependency 2)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_mov(FR::F6, FR::F2).s(),
                Assembler::make_cop1_mov(FR::F6, FR::F4).s(),
            )),
            Box::new((
                "MOV.S (no dependency 3)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_mov(FR::F6, FR::F2).s(),
                Assembler::make_cop1_mov(FR::F8, FR::F2).s(),
            )),
            Box::new((
                "MOV.S (dependency)",
                3u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_mov(FR::F0, FR::F2).s(),
                Assembler::make_cop1_mov(FR::F6, FR::F0).s(),
            )),
            Box::new((
                "MOV.S (dependency via ft)",
                3u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_mov(FR::F0, FR::F2).s(),
                Assembler::make_cop1_mov(FR::F6, FR::F2).s(),
            )),
            Box::new((
                "MOV.S (non-dependency as ft!=0)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_mov(FR::F0, FR::F2).s(),
                Assembler::make_cop1_mov_with_ft(FR::F6, FR::F2, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S (no dependency 1)",
                10u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_w(FR::F6, FR::F2).s(),
                Assembler::make_cop1_cvt_w(FR::F8, FR::F4).s(),
            )),
            Box::new((
                "CVT.W.S (no dependency 2)",
                10u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_w(FR::F6, FR::F2).s(),
                Assembler::make_cop1_cvt_w(FR::F6, FR::F4).s(),
            )),
            Box::new((
                "CVT.W.S (no dependency 3)",
                10u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_w(FR::F6, FR::F2).s(),
                Assembler::make_cop1_cvt_w(FR::F8, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S (dependency)",
                11u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
                Assembler::make_cop1_cvt_w(FR::F6, FR::F0).s(),
            )),
            Box::new((
                "CVT.W.S (dependency via ft)",
                11u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
                Assembler::make_cop1_cvt_w(FR::F6, FR::F2).s(),
            )),
            Box::new((
                "CVT.W.S (non-dependency as ft!=0)",
                10u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).s(),
                Assembler::make_cop1_cvt_w_with_ft(FR::F6, FR::F2, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S (no dependency 1)",
                10u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_l(FR::F6, FR::F2).s(),
                Assembler::make_cop1_cvt_l(FR::F8, FR::F4).s(),
            )),
            Box::new((
                "CVT.L.S (no dependency 2)",
                10u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_l(FR::F6, FR::F2).s(),
                Assembler::make_cop1_cvt_l(FR::F6, FR::F4).s(),
            )),
            Box::new((
                "CVT.L.S (no dependency 3)",
                10u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_l(FR::F6, FR::F2).s(),
                Assembler::make_cop1_cvt_l(FR::F8, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S (dependency)",
                11u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
                Assembler::make_cop1_cvt_l(FR::F6, FR::F0).s(),
            )),
            Box::new((
                "CVT.L.S (dependency via ft)",
                11u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
                Assembler::make_cop1_cvt_l(FR::F6, FR::F2).s(),
            )),
            Box::new((
                "CVT.L.S (non-dependency as ft!=0)",
                10u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).s(),
                Assembler::make_cop1_cvt_l_with_ft(FR::F6, FR::F2, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.S (no dependency 1)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_d(FR::F6, FR::F2).s(),
                Assembler::make_cop1_cvt_d(FR::F8, FR::F4).s(),
            )),
            Box::new((
                "CVT.D.S (no dependency 2)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_d(FR::F6, FR::F2).s(),
                Assembler::make_cop1_cvt_d(FR::F6, FR::F4).s(),
            )),
            Box::new((
                "CVT.D.S (no dependency 3)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_d(FR::F6, FR::F2).s(),
                Assembler::make_cop1_cvt_d(FR::F8, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.S (dependency)",
                3u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
                Assembler::make_cop1_cvt_d(FR::F6, FR::F0).s(),
            )),
            Box::new((
                "CVT.D.S (dependency via ft)",
                3u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
                Assembler::make_cop1_cvt_d(FR::F6, FR::F2).s(),
            )),
            Box::new((
                "CVT.D.S (non-dependency as ft!=0)",
                2u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_cvt_d(FR::F0, FR::F2).s(),
                Assembler::make_cop1_cvt_d_with_ft(FR::F6, FR::F2, FR::F2).s(),
            )),
            // Compare - put it after an addition - we can probably skip the other comparisons - if false uses the registers it can be assumed they all do
            Box::new((
                "ADD.S / C.F.S (no dependency)",
                4u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "ADD.S / C.F.S (dependency 1)",
                5u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F0, FR::F4).s(),
            )),
            Box::new((
                "ADD.S / C.F.S (dependency 2)",
                5u32,
                12345678.0f32,
                0.55f32,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F0).s(),
            )),
            // Combining trivial cases and dependencies is as expected: cycles = (mul-is-trivial ? 2 : 5) + (has-dependency ? 1 : 0) + (div-is-trivial ? 2 : 29)
            Box::new((
                "MUL.S, DIV.S (no dependency)",
                34u32,
                2.5f32,
                0.55f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F2).s(),
                Assembler::make_cop1_div(FR::F6, FR::F4, FR::F4).s(),
            )),
            Box::new((
                "MUL.S, DIV.S (dependency)",
                35u32,
                2.1f32,
                0.55f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F2).s(),
                Assembler::make_cop1_div(FR::F6, FR::F0, FR::F0).s(),
            )),
            Box::new((
                "MUL.S, DIV.S (dependency, first trivial)",
                32u32,
                2.7f32,
                1f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).s(),
                Assembler::make_cop1_div(FR::F6, FR::F0, FR::F0).s(),
            )),
            Box::new((
                "MUL.S, DIV.S (dependency, second trivial)",
                8u32,
                3.5f32,
                0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F2).s(),
                Assembler::make_cop1_div(FR::F6, FR::F0, FR::F4).s(),
            )),
            Box::new((
                "MUL.S, DIV.S (dependency, both trivial)",
                5u32,
                0f32,
                0f32,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F2).s(),
                Assembler::make_cop1_div(FR::F6, FR::F0, FR::F0).s(),
            )),
            // MTC1 never creates a dependency, no matter which register is being loaded
            Box::new((
                "MTC1, ADD.S",
                4u32,
                2.5f32,
                0.55f32,
                Assembler::make_mtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_add(FR::F6, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MTC1, DIV.S",
                30u32,
                2.5f32,
                0.55f32,
                Assembler::make_mtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "MTC1, DIV.S",
                30u32,
                2.1f32,
                0.55f32,
                Assembler::make_mtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F0).s(),
            )),
            Box::new((
                "MTC1, DIV.S (trivial)",
                3u32,
                0f32,
                0f32,
                Assembler::make_mtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F0).s(),
            )),
            Box::new((
                "MTC1, ADD.S",
                4u32,
                2.5f32,
                0.55f32,
                Assembler::make_mtc1(GPR::A0, FR::F30),
                Assembler::make_cop1_add(FR::F6, FR::F2, FR::F4).s(),
            )),
            // DMTC1 never creates a dependency, no matter which register is being used afterwards
            Box::new((
                "DMTC1, ADD.S",
                4u32,
                2.5f32,
                0.55f32,
                Assembler::make_dmtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_add(FR::F6, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DMTC1, DIV.S",
                30u32,
                2.5f32,
                0.55f32,
                Assembler::make_dmtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F4).s(),
            )),
            Box::new((
                "DMTC1, DIV.S",
                30u32,
                2.1f32,
                0.55f32,
                Assembler::make_dmtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F0).s(),
            )),
            Box::new((
                "DMTC1, DIV.S (trivial)",
                3u32,
                0f32,
                0f32,
                Assembler::make_dmtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F0).s(),
            )),
            // ** Doubles **

            // ADD.S doesn't create a dependency on it's destination register (unlike ADDIU, see above)
            Box::new((
                "ADD.D (no dependency)",
                6u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_add(FR::F6, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D (no dependency 2)",
                6u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D (ft)",
                7u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_add(FR::F6, FR::F2, FR::F0).d(),
            )),
            Box::new((
                "ADD.D (fs)",
                7u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_add(FR::F6, FR::F0, FR::F4).d(),
            )),
            Box::new((
                "SUB.D (no dependency)",
                6u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_sub(FR::F6, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D (no dependency 2)",
                6u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "SUB.D (ft)",
                7u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_sub(FR::F6, FR::F2, FR::F0).d(),
            )),
            Box::new((
                "SUB.D (fs)",
                7u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_sub(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_sub(FR::F6, FR::F0, FR::F4).d(),
            )),
            Box::new((
                "MUL.D (no dependency)",
                16u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_mul(FR::F6, FR::F4, FR::F4).d(),
            )),
            Box::new((
                "MUL.D (no dependency 2)",
                16u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "MUL.D (ft)",
                17u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_mul(FR::F6, FR::F2, FR::F0).d(),
            )),
            Box::new((
                "MUL.D (fs)",
                17u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_mul(FR::F6, FR::F0, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (no dependency)",
                116u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (no dependency 2)",
                116u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DIV.D (ft)",
                117u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F0).d(),
            )),
            Box::new((
                "DIV.D (fs)",
                117u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_div(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_div(FR::F6, FR::F0, FR::F4).d(),
            )),
            // Instructions with a single input do something odd: They don't use the ft register, but still creates a dependency on it. Let's try this specifically
            Box::new((
                "SQRT.D (no dependency 1)",
                116u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_sqrt(FR::F6, FR::F2).d(),
                Assembler::make_cop1_sqrt(FR::F8, FR::F4).d(),
            )),
            Box::new((
                "SQRT.D (no dependency 2)",
                116u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_sqrt(FR::F6, FR::F2).d(),
                Assembler::make_cop1_sqrt(FR::F6, FR::F4).d(),
            )),
            Box::new((
                "SQRT.D (no dependency 3)",
                116u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_sqrt(FR::F6, FR::F2).d(),
                Assembler::make_cop1_sqrt(FR::F8, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D (dependency)",
                117u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
                Assembler::make_cop1_sqrt(FR::F6, FR::F0).d(),
            )),
            Box::new((
                "SQRT.D (dependency via ft)",
                117u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
                Assembler::make_cop1_sqrt(FR::F6, FR::F2).d(),
            )),
            Box::new((
                "SQRT.D (non-dependency as ft!=0)",
                116u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_sqrt(FR::F0, FR::F2).d(),
                Assembler::make_cop1_sqrt_with_ft(FR::F6, FR::F2, FR::F2).d(),
            )),
            Box::new((
                "ABS.D (no dependency 1)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_abs(FR::F6, FR::F2).d(),
                Assembler::make_cop1_abs(FR::F8, FR::F4).d(),
            )),
            Box::new((
                "ABS.D (no dependency 2)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_abs(FR::F6, FR::F2).d(),
                Assembler::make_cop1_abs(FR::F6, FR::F4).d(),
            )),
            Box::new((
                "ABS.D (no dependency 3)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_abs(FR::F6, FR::F2).d(),
                Assembler::make_cop1_abs(FR::F8, FR::F2).d(),
            )),
            Box::new((
                "ABS.D (dependency)",
                3u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
                Assembler::make_cop1_abs(FR::F6, FR::F0).d(),
            )),
            Box::new((
                "ABS.D (dependency via ft)",
                3u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
                Assembler::make_cop1_abs(FR::F6, FR::F2).d(),
            )),
            Box::new((
                "ABS.D (non-dependency as ft!=0)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_abs(FR::F0, FR::F2).d(),
                Assembler::make_cop1_abs_with_ft(FR::F6, FR::F2, FR::F2).d(),
            )),
            Box::new((
                "NEG.D (no dependency 1)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_neg(FR::F6, FR::F2).d(),
                Assembler::make_cop1_neg(FR::F8, FR::F4).d(),
            )),
            Box::new((
                "NEG.D (no dependency 2)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_neg(FR::F6, FR::F2).d(),
                Assembler::make_cop1_neg(FR::F6, FR::F4).d(),
            )),
            Box::new((
                "NEG.D (no dependency 3)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_neg(FR::F6, FR::F2).d(),
                Assembler::make_cop1_neg(FR::F8, FR::F2).d(),
            )),
            Box::new((
                "NEG.D (dependency)",
                3u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
                Assembler::make_cop1_neg(FR::F6, FR::F0).d(),
            )),
            Box::new((
                "NEG.D (dependency via ft)",
                3u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
                Assembler::make_cop1_neg(FR::F6, FR::F2).d(),
            )),
            Box::new((
                "NEG.D (non-dependency as ft!=0)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_neg(FR::F0, FR::F2).d(),
                Assembler::make_cop1_neg_with_ft(FR::F6, FR::F2, FR::F2).d(),
            )),
            Box::new((
                "MOV.D (no dependency 1)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_mov(FR::F6, FR::F2).d(),
                Assembler::make_cop1_mov(FR::F8, FR::F4).d(),
            )),
            Box::new((
                "MOV.D (no dependency 2)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_mov(FR::F6, FR::F2).d(),
                Assembler::make_cop1_mov(FR::F6, FR::F4).d(),
            )),
            Box::new((
                "MOV.D (no dependency 3)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_mov(FR::F6, FR::F2).d(),
                Assembler::make_cop1_mov(FR::F8, FR::F2).d(),
            )),
            Box::new((
                "MOV.D (dependency)",
                3u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_mov(FR::F0, FR::F2).d(),
                Assembler::make_cop1_mov(FR::F6, FR::F0).d(),
            )),
            Box::new((
                "MOV.D (dependency via ft)",
                3u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_mov(FR::F0, FR::F2).d(),
                Assembler::make_cop1_mov(FR::F6, FR::F2).d(),
            )),
            Box::new((
                "MOV.D (non-dependency as ft!=0)",
                2u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_mov(FR::F0, FR::F2).d(),
                Assembler::make_cop1_mov_with_ft(FR::F6, FR::F2, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D (no dependency 1)",
                10u32,
                0.0f64,
                0.55f64,
                Assembler::make_cop1_cvt_w(FR::F6, FR::F2).d(),
                Assembler::make_cop1_cvt_w(FR::F8, FR::F4).d(),
            )),
            Box::new((
                "CVT.W.D (no dependency 2)",
                10u32,
                0.0f64,
                0.55f64,
                Assembler::make_cop1_cvt_w(FR::F6, FR::F2).d(),
                Assembler::make_cop1_cvt_w(FR::F6, FR::F4).d(),
            )),
            Box::new((
                "CVT.W.D (no dependency 3)",
                10u32,
                0.0f64,
                0.55f64,
                Assembler::make_cop1_cvt_w(FR::F6, FR::F2).d(),
                Assembler::make_cop1_cvt_w(FR::F8, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D (dependency)",
                11u32,
                0.0f64,
                0.55f64,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
                Assembler::make_cop1_cvt_w(FR::F6, FR::F0).d(),
            )),
            Box::new((
                "CVT.W.D (dependency via ft)",
                11u32,
                0.0f64,
                0.55f64,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
                Assembler::make_cop1_cvt_w(FR::F6, FR::F2).d(),
            )),
            Box::new((
                "CVT.W.D (non-dependency as ft!=0)",
                10u32,
                0.0f64,
                0.55f64,
                Assembler::make_cop1_cvt_w(FR::F0, FR::F2).d(),
                Assembler::make_cop1_cvt_w_with_ft(FR::F6, FR::F2, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D (no dependency 1)",
                10u32,
                0.0f64,
                0.55f64,
                Assembler::make_cop1_cvt_l(FR::F6, FR::F2).d(),
                Assembler::make_cop1_cvt_l(FR::F8, FR::F4).d(),
            )),
            Box::new((
                "CVT.L.D (no dependency 2)",
                10u32,
                0.0f64,
                0.55f64,
                Assembler::make_cop1_cvt_l(FR::F6, FR::F2).d(),
                Assembler::make_cop1_cvt_l(FR::F6, FR::F4).d(),
            )),
            Box::new((
                "CVT.L.D (no dependency 3)",
                10u32,
                0f64,
                0.55f64,
                Assembler::make_cop1_cvt_l(FR::F6, FR::F2).d(),
                Assembler::make_cop1_cvt_l(FR::F8, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D (dependency)",
                11u32,
                0f64,
                0.55f64,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
                Assembler::make_cop1_cvt_l(FR::F6, FR::F0).d(),
            )),
            Box::new((
                "CVT.L.D (dependency via ft)",
                11u32,
                0f64,
                0.55f64,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
                Assembler::make_cop1_cvt_l(FR::F6, FR::F2).d(),
            )),
            Box::new((
                "CVT.L.D (non-dependency as ft!=0)",
                10u32,
                0f64,
                0.55f64,
                Assembler::make_cop1_cvt_l(FR::F0, FR::F2).d(),
                Assembler::make_cop1_cvt_l_with_ft(FR::F6, FR::F2, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D (no dependency 1)",
                4u32,
                0f64,
                0.55f64,
                Assembler::make_cop1_cvt_s(FR::F6, FR::F2).d(),
                Assembler::make_cop1_cvt_s(FR::F8, FR::F4).d(),
            )),
            Box::new((
                "CVT.S.D (no dependency 2)",
                4u32,
                0f64,
                0.55f64,
                Assembler::make_cop1_cvt_s(FR::F6, FR::F2).d(),
                Assembler::make_cop1_cvt_s(FR::F6, FR::F4).d(),
            )),
            Box::new((
                "CVT.S.D (no dependency 3)",
                4u32,
                0f64,
                0.55f64,
                Assembler::make_cop1_cvt_s(FR::F6, FR::F2).d(),
                Assembler::make_cop1_cvt_s(FR::F8, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D (dependency)",
                5u32,
                0f64,
                0.55f64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
                Assembler::make_cop1_cvt_s(FR::F6, FR::F0).d(),
            )),
            Box::new((
                "CVT.S.D (dependency via ft)",
                5u32,
                0f64,
                0.55f64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
                Assembler::make_cop1_cvt_s(FR::F6, FR::F2).d(),
            )),
            Box::new((
                "CVT.S.D (non-dependency as ft!=0)",
                4u32,
                0f64,
                0.55f64,
                Assembler::make_cop1_cvt_s(FR::F0, FR::F2).d(),
                Assembler::make_cop1_cvt_s_with_ft(FR::F6, FR::F2, FR::F2).d(),
            )),
            // Compare - put it after an addition - we can probably skip the other comparisons - if false uses the registers it can be assumed they all do
            Box::new((
                "ADD.D / C.F.D (no dependency)",
                4u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "ADD.D / C.F.D (dependency 1)",
                5u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F0, FR::F4).d(),
            )),
            Box::new((
                "ADD.D / C.F.D (dependency 2)",
                5u32,
                12345678.0f64,
                0.55f64,
                Assembler::make_cop1_add(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_c_cond(Cop1Condition::F, FR::F2, FR::F0).d(),
            )),
            // Combining trivial cases and dependencies is as expected: cycles = (mul-is-trivial ? 2 : 5) + (has-dependency ? 1 : 0) + (div-is-trivial ? 2 : 29)
            Box::new((
                "MUL.D, DIV.D (no dependency)",
                66u32,
                2.5f64,
                0.55f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F2).d(),
                Assembler::make_cop1_div(FR::F6, FR::F4, FR::F4).d(),
            )),
            Box::new((
                "MUL.D, DIV.D (dependency)",
                67u32,
                2.1f64,
                0.55f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F2).d(),
                Assembler::make_cop1_div(FR::F6, FR::F0, FR::F0).d(),
            )),
            Box::new((
                "MUL.D, DIV.D (dependency, first trivial)",
                61u32,
                2.7f64,
                1f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F4).d(),
                Assembler::make_cop1_div(FR::F6, FR::F0, FR::F0).d(),
            )),
            Box::new((
                "MUL.D, DIV.D (dependency, second trivial)",
                11u32,
                3.5f64,
                0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F2).d(),
                Assembler::make_cop1_div(FR::F6, FR::F0, FR::F4).d(),
            )),
            Box::new((
                "MUL.D, DIV.D (dependency, both trivial)",
                5u32,
                0f64,
                0f64,
                Assembler::make_cop1_mul(FR::F0, FR::F2, FR::F2).d(),
                Assembler::make_cop1_div(FR::F6, FR::F0, FR::F0).d(),
            )),
            // DMTC1 never creates a dependency, no matter which register is being used afterwards
            Box::new((
                "DMTC1, ADD.D",
                4u32,
                2.5f64,
                0.55f64,
                Assembler::make_dmtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_add(FR::F6, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DMTC1, DIV.D",
                59u32,
                2.5f64,
                0.55f64,
                Assembler::make_dmtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F4).d(),
            )),
            Box::new((
                "DMTC1, DIV.D",
                59u32,
                2.1f64,
                0.55f64,
                Assembler::make_dmtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F0).d(),
            )),
            Box::new((
                "DMTC1, DIV.D (trivial)",
                3u32,
                0f64,
                0f64,
                Assembler::make_dmtc1(GPR::A0, FR::F0),
                Assembler::make_cop1_div(FR::F6, FR::F2, FR::F0).d(),
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        set_fcsr(
            fcsr()
                .with_enable_invalid_operation(false)
                .with_flush_denorm_to_zero(true),
        );

        match (*value).downcast_ref::<(&str, u32, f32, f32, u32, u32)>() {
            Some((_context, expected_cycles, value2, value4, instruction1, instruction2)) => {
                let value2_u64 = unsafe { transmute::<f32, u32>(*value2) } as u64;
                let value4_u64 = unsafe { transmute::<f32, u32>(*value4) } as u64;
                return test_register_dependency(
                    *expected_cycles,
                    value2_u64,
                    value4_u64,
                    *instruction1,
                    *instruction2,
                );
            }
            _ => {}
        }
        match (*value).downcast_ref::<(&str, u32, f64, f64, u32, u32)>() {
            Some((_context, expected_cycles, value2, value4, instruction1, instruction2)) => {
                let value2_u64 = unsafe { transmute::<f64, u64>(*value2) };
                let value4_u64 = unsafe { transmute::<f64, u64>(*value4) };
                test_register_dependency(
                    *expected_cycles,
                    value2_u64,
                    value4_u64,
                    *instruction1,
                    *instruction2,
                )
            }
            _ => Err(format!("Unexpected pattern")),
        }
    }
}

pub struct LikelyBranchCycleCount {}

impl Test for LikelyBranchCycleCount {
    fn name(&self) -> &str {
        "Timing: Likely branch has to count the skipped instruction"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec![
            // No dependency here
            Box::new((
                "NOP; BEQL; NOP",
                Assembler::make_nop(),
                Assembler::make_beql(GPR::V1, GPR::R0, 1),
                Assembler::make_nop(),
            )),
            Box::new((
                "NOP; BNEL; NOP",
                Assembler::make_nop(),
                Assembler::make_bnel(GPR::R0, GPR::R0, 1),
                Assembler::make_nop(),
            )),
            Box::new((
                "NOP; BGTZL; NOP",
                Assembler::make_nop(),
                Assembler::make_bgtzl(GPR::R0, 1),
                Assembler::make_nop(),
            )),
            Box::new((
                "NOP; BLEZL; NOP",
                Assembler::make_nop(),
                Assembler::make_blezl(GPR::A0, 1),
                Assembler::make_nop(),
            )),
            // This would be a dependency. Ensure this isn't incorrectly counted as one
            Box::new((
                "LB $A2 (from cached); BEQL $V1, $R0; ADDIU $A3, $A2, 0",
                Assembler::make_lb(GPR::A2, 0, GPR::V1),
                Assembler::make_beql(GPR::V1, GPR::R0, 1),
                Assembler::make_addiu(GPR::A3, GPR::A2, 0),
            )),
        ]
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(&str, u32, u32, u32)>() {
            Some((_context, instruction1, instruction2, instruction3)) => {
                assert_cycles_with_codegen(
                    4,
                    0x123,
                    0x456,
                    Status::DEFAULT,
                    ExceptionTimingMode::Off,
                    |writer| {
                        writer.write(*instruction1);
                        writer.write(*instruction2);
                        writer.write(Assembler::make_nop()); // Delay slot (skipped)
                        writer.write(*instruction3);
                    },
                )
            }
            _ => Err(format!("Unexpected pattern")),
        }
    }
}
