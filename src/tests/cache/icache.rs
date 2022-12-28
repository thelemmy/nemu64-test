use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;
use arbitrary_int::{u20, u5};
use crate::assembler::{Assembler, GPR};
use crate::cop0::{CacheOp, RegisterIndex, TagLo, TagLoPState};
use crate::memory_map::MemoryMap;
use crate::tests::{Level, Test};
use crate::tests::soft_asserts::{soft_assert_eq, soft_assert_less, soft_assert_less_or_equal};
use crate::uncached_memory::{UncachedHeapMemory, UncachedHeapMemoryWriter};

// TODO:
// - Have an equivalent to the data cache test CacheLineIndexPtagConflict where the
//   overlapping bits of ptag and CacheLineIndex contradict each other
// - Cacheline is loaded 2 instructions before end of line. What happens if the line ends with a
//   jump? Is the next cacheline loaded anyway?

pub struct ModifyCached {}

impl Test for ModifyCached {
    fn name(&self) -> &str { "icache: Modify cached" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // We'll want to modify memory and execute it. But there's a catch: The code that does
        // the generation must not be on the same cache line as the code being generated,
        // otherwise we invalidate caches when we don't mean to.
        // Therefore, the modifying code has to be in generated as well...yuck

        let mut memory = UncachedHeapMemory::new_with_align(8192, 32);

        // Put some code in the cache line after the one being tested
        memory.write(GENERATED + 4096, Assembler::make_jr(GPR::RA));
        memory.write(GENERATED + 4097, Assembler::make_nop());

        let mut writer = UncachedHeapMemoryWriter::new(&mut memory);

        const GENERATOR: usize = 0;
        const GENERATED: usize = 100;

        fn write(writer: &mut UncachedHeapMemoryWriter<u32>, use_data_cache: bool, offset: i16, value: u32) {
            writer.write(Assembler::make_lui(GPR::A1, (value >> 16) as i16));
            writer.write(Assembler::make_ori(GPR::A1, GPR::A1, value as u16));
            writer.write(Assembler::make_sw(GPR::A1, offset << 2, if use_data_cache { GPR::A0 } else { GPR::V1 }));
        }

        writer.write(Assembler::make_ori(GPR::A2, GPR::RA, 0));

        // Generate main code to test. Execute and write to result1
        write(&mut writer, false, 0, Assembler::make_lui(GPR::T0, 0x1111));
        write(&mut writer, false, 1, Assembler::make_ori(GPR::T0, GPR::T0, 0x1111));
        write(&mut writer, false, 2, Assembler::make_jr(GPR::RA));
        write(&mut writer, false, 3, Assembler::make_nop());
        writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_ori(GPR::T1, GPR::T0, 0));

        // Change first instruction and run again. This should not get picked up. Write to result2
        write(&mut writer, false, 0, Assembler::make_lui(GPR::T0, 0x2222));
        writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_ori(GPR::T2, GPR::T0, 0));

        // Run +512 to flush the cache line. Then run again. This time the changes should be picked up. write to Result3
        writer.write(Assembler::make_bgezal(GPR::R0, (GENERATED + 4096 - writer.index() - 1) as i16));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_ori(GPR::T3, GPR::T0, 0));

        // Change code again, but this time using data cache. Don't write back data yet, but do flush the instruction cache
        write(&mut writer, true, 0, Assembler::make_lui(GPR::T0, 0x3333));
        writer.write(Assembler::make_bgezal(GPR::R0, (GENERATED + 4096 - writer.index() - 1) as i16));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_ori(GPR::T4, GPR::T0, 0));

        // Write back dcache by moving the cache line. Then flush the icache line and run again
        writer.write(Assembler::make_lw(GPR::R0, 8192, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_bgezal(GPR::R0, (GENERATED + 4096 - writer.index() - 1) as i16));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_ori(GPR::T5, GPR::T0, 0));

        writer.write(Assembler::make_ori(GPR::RA, GPR::A2, 0));
        writer.write(Assembler::make_jr(GPR::RA));
        writer.write(Assembler::make_nop());

        soft_assert_less(GENERATOR + (writer.index() as usize), GENERATED, "Error in test: Not enough room for generator")?;

        let result1: u32;
        let result2: u32;
        let result3: u32;
        let result4: u32;
        let result5: u32;
        unsafe {
            asm!("
                .set noat
                .set noreorder
                OR $5, $31, $0
                JALR $2
                NOP
                OR $31, $5, $0
            ",
            in("$2") MemoryMap::physical_to_cached::<u32>(memory.start_physical() + (GENERATOR << 2)),
            in("$3") MemoryMap::physical_to_uncached_mut::<u32>(memory.start_physical() + (GENERATED << 2)),
            in("$4") MemoryMap::physical_to_cached::<u32>(memory.start_physical() + (GENERATED << 2)),
            // RA temp for inline asm
            out("$5") _,
            // RA temp for generator
            out("$6") _,
            // temp for generator
            out("$7") _,
            // temp for return value from inner generated function
            out("$8") _,
            out("$9") result1,
            out("$10") result2,
            out("$11") result3,
            out("$12") result4,
            out("$13") result5,
            )
        }

        // Execute. This will pick up the instructions that were just written
        soft_assert_eq(result1, 0x1111_1111, "Result value 1 (execute dynamically generated (but never modified) code)")?;
        soft_assert_eq(result2, 0x1111_1111, "Result value 2 (run after modify without icache invalidation)")?;
        soft_assert_eq(result3, 0x2222_1111, "Result value 3 (run after icache was invalidated)")?;
        soft_assert_eq(result4, 0x2222_1111, "Result value 4 (run after icache was invalidated, but dcache wasn't written back)")?;
        soft_assert_eq(result5, 0x3333_1111, "Result value 5 (run after dcache WriteBack then icache invalidated)")?;

        Ok(())
    }
}

/// Similar to before, but modifies the second instruction. This should confuse
/// recompilers that only check the beginning of a basic block
pub struct ModifyCached2 {}

impl Test for ModifyCached2 {
    fn name(&self) -> &str { "icache: Modify cached (second instruction)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // We'll want to modify memory and execute it. But there's a catch: The code that does
        // the generation must not be on the same cache line as the code being generated,
        // otherwise we invalidate caches when we don't mean to.
        // Therefore, the modifying code has to be in generated as well...yuck

        let mut memory = UncachedHeapMemory::new_with_align(8192, 8192);

        // Put some code in the cache line after the one being tested
        memory.write(GENERATED + 4096, Assembler::make_jr(GPR::RA));
        memory.write(GENERATED + 4097, Assembler::make_nop());

        let mut writer = UncachedHeapMemoryWriter::new(&mut memory);

        const GENERATOR: usize = 0;
        const GENERATED: usize = 100;

        fn write(writer: &mut UncachedHeapMemoryWriter<u32>, use_data_cache: bool, offset: i16, value: u32) {
            writer.write(Assembler::make_lui(GPR::A1, (value >> 16) as i16));
            writer.write(Assembler::make_ori(GPR::A1, GPR::A1, value as u16));
            writer.write(Assembler::make_sw(GPR::A1, offset << 2, if use_data_cache { GPR::A0 } else { GPR::V1 }));
        }

        writer.write(Assembler::make_ori(GPR::A2, GPR::RA, 0));

        // Generate main code to test. Execute and write to result1
        write(&mut writer, false, 0, Assembler::make_lui(GPR::T0, 0x1111));
        write(&mut writer, false, 1, Assembler::make_ori(GPR::T0, GPR::T0, 0x1111));
        write(&mut writer, false, 2, Assembler::make_jr(GPR::RA));
        write(&mut writer, false, 3, Assembler::make_nop());
        writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_ori(GPR::T1, GPR::T0, 0));

        // Change second instruction and run again. This should not get picked up. Write to result2
        write(&mut writer, false, 1, Assembler::make_ori(GPR::T0, GPR::T0, 0x2222));
        writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_ori(GPR::T2, GPR::T0, 0));

        // Run +512 to flush the cache line. Then run again. This time the changes should be picked up. write to Result3
        writer.write(Assembler::make_bgezal(GPR::R0, (GENERATED + 4096 - writer.index() - 1) as i16));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_ori(GPR::T3, GPR::T0, 0));

        // Change code again, but this time using data cache. Don't write back data yet, but do flush the instruction cache
        write(&mut writer, true, 1, Assembler::make_ori(GPR::T0, GPR::T0, 0x3333));
        writer.write(Assembler::make_bgezal(GPR::R0, (GENERATED + 4096 - writer.index() - 1) as i16));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_ori(GPR::T4, GPR::T0, 0));

        // Write back dcache by moving the cache line. Then flush the icache line and run again
        writer.write(Assembler::make_lw(GPR::R0, 8192, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_bgezal(GPR::R0, (GENERATED + 4096 - writer.index() - 1) as i16));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_ori(GPR::T5, GPR::T0, 0));

        writer.write(Assembler::make_ori(GPR::RA, GPR::A2, 0));
        writer.write(Assembler::make_jr(GPR::RA));
        writer.write(Assembler::make_nop());

        soft_assert_less(GENERATOR + (writer.index() as usize), GENERATED, "Error in test: Not enough room for generator")?;

        let result1: u32;
        let result2: u32;
        let result3: u32;
        let result4: u32;
        let result5: u32;
        unsafe {
            asm!("
                .set noat
                .set noreorder
                OR $5, $31, $0
                JALR $2
                NOP
                OR $31, $5, $0
            ",
            in("$2") MemoryMap::physical_to_cached::<u32>(memory.start_physical() + (GENERATOR << 2)),
            in("$3") MemoryMap::physical_to_uncached_mut::<u32>(memory.start_physical() + (GENERATED << 2)),
            in("$4") MemoryMap::physical_to_cached::<u32>(memory.start_physical() + (GENERATED << 2)),
            // RA temp for inline asm
            out("$5") _,
            // RA temp for generator
            out("$6") _,
            // temp for generator
            out("$7") _,
            // temp for return value from inner generated function
            out("$8") _,
            out("$9") result1,
            out("$10") result2,
            out("$11") result3,
            out("$12") result4,
            out("$13") result5,
            )
        }

        // Execute. This will pick up the instructions that were just written
        soft_assert_eq(result1, 0x1111_1111, "Result value 1 (execute dynamically generated (but never modified) code)")?;
        soft_assert_eq(result2, 0x1111_1111, "Result value 2 (run after modify without icache invalidation)")?;
        soft_assert_eq(result3, 0x1111_2222, "Result value 3 (run after icache was invalidated)")?;
        soft_assert_eq(result4, 0x1111_2222, "Result value 4 (run after icache was invalidated, but dcache wasn't written back)")?;
        soft_assert_eq(result5, 0x1111_3333, "Result value 5 (run after dcache WriteBack then icache invalidated)")?;

        Ok(())
    }
}

fn test_modify_within_basic_block(instruction_at_beginning: u32, instruction_further_down: u32, generator_index: usize, generated_index: usize, expected_modified_code: bool) -> Result<(), String> {
    let mut memory = UncachedHeapMemory::new_with_align(8192, 4096);
    let mut writer = UncachedHeapMemoryWriter::new(&mut memory);

    const NEW_INSTRUCTION: u32 = Assembler::make_ori(GPR::T1, GPR::R0, 0x2222);

    // Write NEW_INSTRUCTION to index generated
    writer.write(Assembler::make_lui(GPR::A1, (NEW_INSTRUCTION >> 16) as i16));
    writer.write(Assembler::make_ori(GPR::A1, GPR::A1, NEW_INSTRUCTION as u16));

    writer.write(instruction_at_beginning);
    soft_assert_less_or_equal(writer.index(), generator_index, "Error in test: Not enough room for setup")?;
    // NOP cascade until save function
    while writer.index() < generator_index {
        writer.write(Assembler::make_nop());
    }

    writer.write(instruction_further_down);

    // NOP cascade until the code to be executed
    soft_assert_less_or_equal(writer.index(), generated_index, "Error in test: Not enough room for generator")?;
    while writer.index() < generated_index {
        writer.write(Assembler::make_nop());
    }

    // Instruction that will be overwritten
    writer.write(Assembler::make_ori(GPR::T1, GPR::R0, 0x1111));

    writer.write(Assembler::make_jr(GPR::RA));
    writer.write(Assembler::make_nop());

    let result1: u32;
    unsafe {
        asm!("
            .set noat
            .set noreorder
            OR $6, $31, $0
            JALR $2
            NOP
            OR $31, $6, $0
        ",
        in("$2") MemoryMap::physical_to_cached::<u32>(memory.start_physical()),
        in("$3") MemoryMap::physical_to_uncached_mut::<u32>(memory.start_physical() + (generated_index << 2)),
        in("$4") MemoryMap::physical_to_cached_mut::<u32>(memory.start_physical() + (generated_index << 2)),
        // Temp for inline asm
        out("$5") _,
        // Backup for RA
        out("$6") _,
        out("$9") result1,
        )
    }

    // Execute. This will pick up the instructions that were just written
    soft_assert_eq(result1, if expected_modified_code { 0x2222 } else { 0x1111}, "Result value")?;

    Ok(())
}

/// Is an instruction that is just below in the instruction stream and that is being overwritten
/// executed as old or new?
/// This is especially tricky for dynarecs as they have to detect the change during a basic block
pub struct ModifyWithinBasicBlockUncachedWrite {}

impl Test for ModifyWithinBasicBlockUncachedWrite {
    fn name(&self) -> &str { "icache: Self-modifying code within basic block (single write)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            // Format: Whether the new instruction will be executed, write operation index, target operation index

            // same icache line
            Box::new((false, 3usize, 7usize)),

            // different icache line
            Box::new((true, 3usize, 8usize)),
            Box::new((true, 4usize, 8usize)),
            Box::new((true, 5usize, 8usize)),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(bool, usize, usize)>() {
            Some((expected_modified_code, generator_index, generated_index)) => {
                test_modify_within_basic_block(
                    Assembler::make_nop(),
                    Assembler::make_sw(GPR::A1, 0 << 2, GPR::V1),
                    *generator_index, *generated_index, *expected_modified_code)?;
            }
            None => {
                Err("Unhandled pattern")?;
            }
        }
        Ok(())
    }
}

pub struct ModifyWithinBasicBlockUncachedWriteCycle {}

impl Test for ModifyWithinBasicBlockUncachedWriteCycle {
    fn name(&self) -> &str { "icache: Self-modifying code within basic block (single write) (cycle accurate)" }

    fn level(&self) -> Level { Level::Cycle }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            Box::new((false, 6usize, 8usize)),
            Box::new((false, 7usize, 8usize)),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(bool, usize, usize)>() {
            Some((expected_modified_code, generator_index, generated_index)) => {
                test_modify_within_basic_block(
                    Assembler::make_nop(),
                    Assembler::make_sw(GPR::A1, 0 << 2, GPR::V1),
                    *generator_index, *generated_index, *expected_modified_code)?;
            }
            None => {
                Err("Unhandled pattern")?;
            }
        }
        Ok(())
    }
}

pub struct ModifyTargetOfBranch {}

impl Test for ModifyTargetOfBranch {
    fn name(&self) -> &str { "icache: Modify target of branch" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        test_modify_within_basic_block(
            Assembler::make_sw(GPR::A1, 0 << 2, GPR::V1),
            Assembler::make_beq(GPR::R0, GPR::R0, 4),
            3, 8, true)
    }
}

pub struct ModifyTargetOfBranchFromDelaySlot {}

impl Test for ModifyTargetOfBranchFromDelaySlot {
    fn name(&self) -> &str { "icache: Modify target of branch (from within delay slot)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        test_modify_within_basic_block(
            Assembler::make_beq(GPR::R0, GPR::R0, 5),
            Assembler::make_sw(GPR::A1, 0 << 2, GPR::V1),
            3, 16, true)
    }
}

pub struct ModifyTargetOfBranchFromDelaySlotCycle {}

impl Test for ModifyTargetOfBranchFromDelaySlotCycle {
    fn name(&self) -> &str { "icache: Modify target of branch (from within delay slot) (cycle accurate)" }

    fn level(&self) -> Level { Level::Cycle }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        test_modify_within_basic_block(
            Assembler::make_beq(GPR::R0, GPR::R0, 5),
            Assembler::make_sw(GPR::A1, 0 << 2, GPR::V1),
            3, 8, false)
    }
}

pub struct ModifyWithinBasicBlockExplicitDCacheHitWriteBack {}

impl Test for ModifyWithinBasicBlockExplicitDCacheHitWriteBack {
    fn name(&self) -> &str { "icache: Self-modifying code within basic block (with explicit dcache HitWriteBack)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            // Format: Whether the new instruction will be executed, write operation index, target operation index

            // different icache line
            Box::new((true, 3usize, 8usize)),
            Box::new((true, 4usize, 8usize)),
            Box::new((true, 5usize, 8usize)),
            Box::new((true, 6usize, 8usize)),
            // 7-->8 is in the next test, which is marked as Level::Cycle
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(bool, usize, usize)>() {
            Some((expected_modified_code, generator_index, generated_index)) => {
                test_modify_within_basic_block(
                    Assembler::make_sw(GPR::A1, 0 << 2, GPR::A0),
                    Assembler::make_cache(CacheOp::DataHitWriteBack, 0 << 2, GPR::A0),
                    *generator_index, *generated_index, *expected_modified_code)?;
            }
            None => {
                Err("Unhandled pattern")?;
            }
        }
        Ok(())
    }
}

pub struct ModifyWithinBasicBlockExplicitDCacheHitWriteBackCycle {}

impl Test for ModifyWithinBasicBlockExplicitDCacheHitWriteBackCycle {
    fn name(&self) -> &str { "icache: Self-modifying code within basic block (with explicit dcache HitWriteBack) (cycle accurate)" }

    fn level(&self) -> Level { Level::Cycle }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            // different icache line, but it's too late and the new line has already been loaded
            Box::new((false, 7usize, 8usize)),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(bool, usize, usize)>() {
            Some((expected_modified_code, generator_index, generated_index)) => {
                test_modify_within_basic_block(
                    Assembler::make_sw(GPR::A1, 0 << 2, GPR::A0),
                    Assembler::make_cache(CacheOp::DataHitWriteBack, 0 << 2, GPR::A0),
                    *generator_index, *generated_index, *expected_modified_code)?;
            }
            None => {
                Err("Unhandled pattern")?;
            }
        }
        Ok(())
    }
}

pub struct ModifyWithinBasicBlockImplicitDCacheWriteBack {}

impl Test for ModifyWithinBasicBlockImplicitDCacheWriteBack {
    fn name(&self) -> &str { "icache: Self-modifying code within basic block (with implicit dcache write back)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            Box::new((true, 3usize, 8usize)),
            Box::new((true, 4usize, 8usize)),
            Box::new((true, 5usize, 8usize)),
            Box::new((true, 6usize, 8usize)),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(bool, usize, usize)>() {
            Some((expected_modified_code, generator_index, generated_index)) => {
                test_modify_within_basic_block(
                    Assembler::make_sw(GPR::A1, 0 << 2, GPR::A0),
                    Assembler::make_sw(GPR::A1, 8192 << 2, GPR::A0),
                    *generator_index, *generated_index, *expected_modified_code)?;
            }
            None => {
                Err("Unhandled pattern")?;
            }
        }
        Ok(())
    }
}

pub struct ModifyWithinBasicBlockImplicitDCacheWriteBackCycle {}

impl Test for ModifyWithinBasicBlockImplicitDCacheWriteBackCycle {
    fn name(&self) -> &str { "icache: Self-modifying code within basic block (with implicit dcache write back) (cycle accurate)" }

    fn level(&self) -> Level { Level::Cycle }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            // different icache line, but it's too late and the new line has already been loaded
            Box::new((false, 7usize, 8usize)),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(bool, usize, usize)>() {
            Some((expected_modified_code, generator_index, generated_index)) => {
                test_modify_within_basic_block(
                    Assembler::make_sw(GPR::A1, 0 << 2, GPR::A0),
                    Assembler::make_sw(GPR::A1, 8192 << 2, GPR::A0),
                    *generator_index, *generated_index, *expected_modified_code)?;
            }
            None => {
                Err("Unhandled pattern")?;
            }
        }
        Ok(())
    }
}

/// Same test as before, but this time using strategic placement of the CACHE instruction to invalidate the icache
pub struct ModifyWithinBasicBlockICacheInvalidate {}

impl Test for ModifyWithinBasicBlockICacheInvalidate {
    fn name(&self) -> &str { "icache: Self-modifying code within basic block (with i-cache invalidation)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            // Format: Whether the new instruction will be executed, write operation index, target operation index

            // different icache line. Unlike before, invalidating ICACHE always works
            Box::new((true, 3usize, 8usize)),
            Box::new((true, 4usize, 8usize)),
            Box::new((true, 5usize, 8usize)),
            Box::new((true, 6usize, 8usize)),
            Box::new((true, 7usize, 8usize)),

            // Even in the same block
            Box::new((true, 5usize, 7usize)),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(bool, usize, usize)>() {
            Some((expected_modified_code, generator_index, generated_index)) => {
                test_modify_within_basic_block(
                    Assembler::make_sw(GPR::A1, 0 << 2, GPR::V1),
                    Assembler::make_cache(CacheOp::InstructionIndexInvalidate, 0 << 2, GPR::A0),
                    *generator_index, *generated_index, *expected_modified_code)?;
            }
            None => {
                Err("Unhandled pattern")?;
            }
        }
        Ok(())
    }
}

pub struct ModifyWithinBasicBlockICacheInvalidateCycle {}

impl Test for ModifyWithinBasicBlockICacheInvalidateCycle {
    fn name(&self) -> &str { "icache: Self-modifying code within basic block (with i-cache invalidation) (cycle accurate)" }

    fn level(&self) -> Level { Level::Cycle }

    fn values(&self) -> Vec<Box<dyn Any>> {
        vec! {
            // Invalidating the immediately next instruction WITHIN the same icache line doesn't work.
            Box::new((false, 6usize, 7usize)),
        }
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<(bool, usize, usize)>() {
            Some((expected_modified_code, generator_index, generated_index)) => {
                test_modify_within_basic_block(
                    Assembler::make_sw(GPR::A1, 0 << 2, GPR::V1),
                    Assembler::make_cache(CacheOp::InstructionIndexInvalidate, 0 << 2, GPR::A0),
                    *generator_index, *generated_index, *expected_modified_code)?;
            }
            None => {
                Err("Unhandled pattern")?;
            }
        }
        Ok(())
    }
}

/// Essentially the same as before, but using multiple SWs in a row. This is to see whether
/// the write-cache makes a difference (it does not)
pub struct ModifyWithinBasicBlockMultipleSW {}

impl Test for ModifyWithinBasicBlockMultipleSW {
    fn name(&self) -> &str { "icache: Self-modifying code within basic block (multiple writes)" }

    fn level(&self) -> Level { Level::Cycle }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let mut memory = UncachedHeapMemory::new_with_align(8192, 4096);
        let mut writer = UncachedHeapMemoryWriter::new(&mut memory);

        // Put a bunch of SW that will all overwrite the first instruction of the next ICACHE line
        for i in 0..8 {
            writer.write(Assembler::make_sw(GPR::new_with_raw_value(GPR::S0.raw_value() + u5::new(i)), (i << 2) as i16, GPR::V1));
        }

        // Instructions that will be overwritten
        for _ in 0..8 {
            writer.write(Assembler::make_nop());
        }

        writer.write(Assembler::make_jr(GPR::RA));
        writer.write(Assembler::make_nop());

        let result1: u32;
        unsafe {
            asm!("
                .set noat
                .set noreorder
                OR $6, $31, $0
                JALR $2
                NOP
                OR $31, $6, $0
            ",
            in("$2") MemoryMap::physical_to_cached::<u32>(memory.start_physical()),
            in("$3") MemoryMap::physical_to_uncached_mut::<u32>(memory.start_physical() + (8 << 2)),
            in("$16") Assembler::make_ori(GPR::T1, GPR::T1, 0x0001),
            in("$17") Assembler::make_ori(GPR::T1, GPR::T1, 0x0002),
            in("$18") Assembler::make_ori(GPR::T1, GPR::T1, 0x0004),
            in("$19") Assembler::make_ori(GPR::T1, GPR::T1, 0x0008),
            in("$20") Assembler::make_ori(GPR::T1, GPR::T1, 0x0010),
            in("$21") Assembler::make_ori(GPR::T1, GPR::T1, 0x0020),
            in("$22") Assembler::make_ori(GPR::T1, GPR::T1, 0x0040),
            in("$23") Assembler::make_ori(GPR::T1, GPR::T1, 0x0080),
            // Temp for inline asm
            out("$5") _,
            // Backup for RA
            out("$6") _,
            inout("$9") 0 => result1,
            )
        }

        // Execute. This will pick up the instructions that were just written
        soft_assert_eq(result1, 0x3F, "Result value")?;
        Ok(())
    }
}

fn test_invalidate<F1: Fn(u20, u20) -> TagLo, F2: Fn(u20, u20) -> TagLo, F3: Fn(u20, u20) -> TagLo>(
    cache_op: CacheOp, expected_after_cache: F1, expected_cache_after_ptag_mismatch1: F2, expected_cache_after_mismatch2: F3,
    result_after_cache_and_write: u32,
) -> Result<(), String> {
    let mut memory = UncachedHeapMemory::new_with_align(8192, 16384);

    let mut writer = UncachedHeapMemoryWriter::new(&mut memory);

    const GENERATOR: usize = 0;
    const GENERATED: usize = 100;

    fn write(writer: &mut UncachedHeapMemoryWriter<u32>, use_data_cache: bool, offset: i16, value: u32) {
        writer.write(Assembler::make_lui(GPR::A1, (value >> 16) as i16));
        writer.write(Assembler::make_ori(GPR::A1, GPR::A1, value as u16));
        writer.write(Assembler::make_sw(GPR::A1, offset << 2, if use_data_cache { GPR::A0 } else { GPR::V1 }));
    }

    writer.write(Assembler::make_ori(GPR::A2, GPR::RA, 0));

    // Result1: Generate main code to test. Execute
    write(&mut writer, false, 0, Assembler::make_lui(GPR::T0, 0x1111));
    write(&mut writer, false, 1, Assembler::make_ori(GPR::T0, GPR::T0, 0x1111));
    write(&mut writer, false, 2, Assembler::make_jr(GPR::RA));
    write(&mut writer, false, 3, Assembler::make_nop());
    writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_ori(GPR::T1, GPR::T0, 0));

    // Result2: Change second instruction and run again. This should not get picked up
    write(&mut writer, false, 1, Assembler::make_ori(GPR::T0, GPR::T0, 0x2222));
    writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_ori(GPR::T2, GPR::T0, 0));

    // Result3: Run Cache to invalidate the cache line with ptag hitting. Then run again. This time the changes should be picked up
    writer.write(Assembler::make_cache(CacheOp::InstructionIndexLoadTag, 0, GPR::V1));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_mfc0(GPR::T4, RegisterIndex::TagLo));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_cache(cache_op, 0, GPR::V1));
    writer.write(Assembler::make_cache(CacheOp::InstructionIndexLoadTag, 0, GPR::V1));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_mfc0(GPR::T5, RegisterIndex::TagLo));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_ori(GPR::T3, GPR::T0, 0));

    // Result4: Run Cache to invalidate while missing ptag. Then run cache again (while still invalid), and again move ptag
    writer.write(Assembler::make_cache(cache_op, 16384, GPR::V1));
    writer.write(Assembler::make_cache(CacheOp::InstructionIndexLoadTag, 0, GPR::V1));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_mfc0(GPR::T6, RegisterIndex::TagLo));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_nop());
    write(&mut writer, false, 1, Assembler::make_ori(GPR::T0, GPR::T0, 0x3333));
    writer.write(Assembler::make_cache(cache_op, 0, GPR::V1));
    writer.write(Assembler::make_cache(CacheOp::InstructionIndexLoadTag, 0, GPR::V1));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_mfc0(GPR::T7, RegisterIndex::TagLo));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_nop());
    write(&mut writer, false, 1, Assembler::make_ori(GPR::T0, GPR::T0, 0x4444));
    writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
    writer.write(Assembler::make_nop());
    writer.write(Assembler::make_ori(GPR::S0, GPR::T0, 0));

    writer.write(Assembler::make_ori(GPR::RA, GPR::A2, 0));
    writer.write(Assembler::make_jr(GPR::RA));
    writer.write(Assembler::make_nop());

    soft_assert_less(GENERATOR + (writer.index() as usize), GENERATED, "Error in test: Not enough room for generator")?;

    let result1: u32;
    let result2: u32;
    let result3: u32;
    let line_before3: u32;
    let line_after3: u32;
    let invalidate_ptag_mismatch: u32;
    let invalidate_ptag_mismatch_while_invalid: u32;
    let result4: u32;
    unsafe {
        asm!("
            .set noat
            .set noreorder
            OR $5, $31, $0
            JALR $2
            NOP
            OR $31, $5, $0
        ",
        in("$2") MemoryMap::physical_to_cached::<u32>(memory.start_physical() + (GENERATOR << 2)),
        in("$3") MemoryMap::physical_to_uncached_mut::<u32>(memory.start_physical() + (GENERATED << 2)),
        in("$4") MemoryMap::physical_to_cached::<u32>(memory.start_physical() + (GENERATED << 2)),
        // RA temp for inline asm
        out("$5") _,
        // RA temp for generator
        out("$6") _,
        // temp for generator
        out("$7") _,
        // temp for return value from inner generated function
        out("$8") _,
        out("$9") result1,
        out("$10") result2,
        out("$11") result3,
        out("$12") line_before3,
        out("$13") line_after3,
        out("$14") invalidate_ptag_mismatch,
        out("$15") invalidate_ptag_mismatch_while_invalid,
        out("$16") result4,
        )
    }

    let ptag = u20::extract_u32((memory.start_physical() + (GENERATED << 2)) as u32, 12);
    let ptag_next = u20::extract_u32((memory.start_physical() + (GENERATED << 2) + 16384) as u32, 12);
    soft_assert_eq(result1, 0x1111_1111, "Result value 1 (execute dynamically generated (but never modified) code)")?;
    soft_assert_eq(result2, 0x1111_1111, "Result value 2 (run before cache instruction)")?;
    soft_assert_eq(line_before3, TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Clean).raw_value(), "Instruction Cache line before cache instruction")?;
    soft_assert_eq(result3, 0x1111_2222, "Result value 3 (run after cache instruction)")?;
    soft_assert_eq(line_after3, expected_after_cache(ptag, ptag_next).raw_value(), "Instruction Cache line after cache instruction")?;

    soft_assert_eq(invalidate_ptag_mismatch, expected_cache_after_ptag_mismatch1(ptag, ptag_next).raw_value(), "Instruction Cache line after cache instruction with ptag mismatch")?;
    soft_assert_eq(invalidate_ptag_mismatch_while_invalid, expected_cache_after_mismatch2(ptag, ptag_next).raw_value(), "Instruction Cache line after cache instruction with ptag mismatch (2nd)")?;

    soft_assert_eq(result4, result_after_cache_and_write, "Result value 4 (run after cache instruction followed by write)")?;

    Ok(())
}

pub struct InstructionCacheIndexInvalidate {}

impl Test for InstructionCacheIndexInvalidate {
    fn name(&self) -> &str { "icache: Cache(IndexInvalidate)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        test_invalidate(
            CacheOp::InstructionIndexInvalidate,
            |ptag, _ptag_next| TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Invalid),
            |_ptag, ptag_next| TagLo::new().with_p_tag_lo(ptag_next).with_p_state(TagLoPState::Invalid),
            |ptag, _ptag_next| TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Invalid),
            0x1111_4444
        )
    }
}

pub struct InstructionCacheHitInvalidate {}

impl Test for InstructionCacheHitInvalidate {
    fn name(&self) -> &str { "icache: Cache(HitInvalidate)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        test_invalidate(
            CacheOp::InstructionHitInvalidate,
            |ptag, _ptag_next| TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Invalid),
            |ptag, _ptag_next| TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Clean),
            |ptag, _ptag_next| TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Invalid),
            0x1111_4444
        )
    }
}

pub struct InstructionCacheFill {}

impl Test for InstructionCacheFill {
    fn name(&self) -> &str { "icache: Cache(Fill)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        test_invalidate(
            CacheOp::InstructionFill,
            |ptag, _ptag_next| TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Clean),
            |_ptag, ptag_next| TagLo::new().with_p_tag_lo(ptag_next).with_p_state(TagLoPState::Clean),
            |ptag, _ptag_next| TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Clean),
            0x1111_3333
        )
    }
}

pub struct InstructionCacheIndexStoreTag {}

impl Test for InstructionCacheIndexStoreTag {
    fn name(&self) -> &str { "icache: Cache(IndexStoreTag)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let mut memory = UncachedHeapMemory::new_with_align(8192, 16384);

        let ptag = u20::extract_u32((memory.start_physical() + (GENERATED << 2)) as u32, 12);
        let ptag_next = u20::extract_u32((memory.start_physical() + (GENERATED << 2) + 16384) as u32, 12);

        let mut writer = UncachedHeapMemoryWriter::new(&mut memory);

        const GENERATOR: usize = 0;
        const GENERATED: usize = 100;

        writer.write(Assembler::make_ori(GPR::A2, GPR::RA, 0));

        for i in 0..4 {
            writer.write(Assembler::make_mtc0( GPR::new_with_raw_value(GPR::T1.raw_value() + u5::new(i * 2)), RegisterIndex::TagLo));
            writer.write(Assembler::make_nop());
            writer.write(Assembler::make_nop());
            writer.write(Assembler::make_cache(CacheOp::InstructionIndexStoreTag, 0, GPR::V1));
            writer.write(Assembler::make_nop());
            writer.write(Assembler::make_nop());
            writer.write(Assembler::make_cache(CacheOp::InstructionIndexLoadTag, 16384, GPR::V1));
            writer.write(Assembler::make_nop());
            writer.write(Assembler::make_nop());
            writer.write(Assembler::make_mfc0(GPR::new_with_raw_value(GPR::T2.raw_value() + u5::new(i * 2)), RegisterIndex::TagLo));
            writer.write(Assembler::make_nop());
            writer.write(Assembler::make_nop());
        }

        writer.write(Assembler::make_ori(GPR::RA, GPR::A2, 0));
        writer.write(Assembler::make_jr(GPR::RA));
        writer.write(Assembler::make_nop());

        soft_assert_less(GENERATOR + (writer.index() as usize), GENERATED, "Error in test: Not enough room for generator")?;

        let result1: u32;
        let result2: u32;
        let result3: u32;
        let result4: u32;
        unsafe {
            asm!("
            .set noat
            .set noreorder
            OR $5, $31, $0
            JALR $2
            NOP
            OR $31, $5, $0
        ",
            in("$2") MemoryMap::physical_to_cached::<u32>(memory.start_physical() + (GENERATOR << 2)),
            in("$3") MemoryMap::physical_to_uncached_mut::<u32>(memory.start_physical() + (GENERATED << 2)),
            in("$4") MemoryMap::physical_to_cached::<u32>(memory.start_physical() + (GENERATED << 2)),
            // RA temp for inline asm
            out("$5") _,
            // RA temp for generator
            out("$6") _,
            // temp for generator
            out("$7") _,
            // temp for return value from inner generated function
            out("$8") _,
            in("$9") TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Invalid).raw_value(),
            out("$10") result1,
            in("$11") TagLo::new().with_p_tag_lo(ptag_next).with_p_state(TagLoPState::_Unused01).raw_value(),
            out("$12") result2,
            in("$13") TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Clean).raw_value(),
            out("$14") result3,
            in("$15") TagLo::new().with_p_tag_lo(ptag_next).with_p_state(TagLoPState::Dirty).raw_value(),
            out("$16") result4,
            )
        }

        soft_assert_eq(result1, TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Invalid).raw_value(), "TagLo after writing Invalid")?;
        soft_assert_eq(result2, TagLo::new().with_p_tag_lo(ptag_next).with_p_state(TagLoPState::Invalid).raw_value(), "TagLo after writing 0b01")?;
        soft_assert_eq(result3, TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Clean).raw_value(), "TagLo after writing Clean")?;
        soft_assert_eq(result4, TagLo::new().with_p_tag_lo(ptag_next).with_p_state(TagLoPState::Clean).raw_value(), "TagLo after writing Dirty")?;

        Ok(())
    }
}

pub struct InstructionCacheHitWriteBack {}

impl Test for InstructionCacheHitWriteBack {
    fn name(&self) -> &str { "icache: Cache(HitWriteBack)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let mut memory = UncachedHeapMemory::new_with_align(8192, 16384);

        let ptag = u20::extract_u32((memory.start_physical() + (GENERATED << 2)) as u32, 12);

        let mut writer = UncachedHeapMemoryWriter::new(&mut memory);

        const GENERATOR: usize = 0;
        const GENERATED: usize = 100;

        writer.write(Assembler::make_ori(GPR::A2, GPR::RA, 0));

        fn write(writer: &mut UncachedHeapMemoryWriter<u32>, use_data_cache: bool, offset: i16, value: u32) {
            writer.write(Assembler::make_lui(GPR::A1, (value >> 16) as i16));
            writer.write(Assembler::make_ori(GPR::A1, GPR::A1, value as u16));
            writer.write(Assembler::make_sw(GPR::A1, offset << 2, if use_data_cache { GPR::A0 } else { GPR::V1 }));
        }

        // Generate main code to test and execute
        write(&mut writer, false, 0, Assembler::make_lui(GPR::T0, 0x1111));
        write(&mut writer, false, 1, Assembler::make_ori(GPR::T0, GPR::T0, 0x1111));
        write(&mut writer, false, 2, Assembler::make_jr(GPR::RA));
        write(&mut writer, false, 3, Assembler::make_nop());
        write(&mut writer, false, 4, Assembler::make_nop());
        write(&mut writer, false, 5, Assembler::make_nop());
        write(&mut writer, false, 6, Assembler::make_nop());
        write(&mut writer, false, 7, Assembler::make_nop());
        write(&mut writer, false, 8, Assembler::make_nop());
        writer.write(Assembler::make_jalr(GPR::RA, GPR::A0));
        writer.write(Assembler::make_nop());

        // Change a bunch of things in the block and the int after it
        write(&mut writer, false, 0, Assembler::make_lui(GPR::T0, 0x2222));
        write(&mut writer, false, 8, Assembler::make_lui(GPR::T0, 0x6666));

        // Write instruction cache back to memory, but miss index
        writer.write(Assembler::make_cache(CacheOp::InstructionHitWriteBack, 16384, GPR::A0));

        // Read back the first (it shouldn't have changed as we missed)
        writer.write(Assembler::make_lw(GPR::T1, 0 << 2, GPR::V1 ));

        // Write instruction cache back to memory, this time hitting
        writer.write(Assembler::make_cache(CacheOp::InstructionHitWriteBack, 12, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_cache(CacheOp::InstructionIndexLoadTag, 12, GPR::A0));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_mfc0(GPR::T2, RegisterIndex::TagLo));
        writer.write(Assembler::make_nop());
        writer.write(Assembler::make_nop());

        // Read back the first (it shouldn't have changed as we missed) and the first of the next block
        writer.write(Assembler::make_lw(GPR::T3, 0 << 2, GPR::V1 ));
        writer.write(Assembler::make_lw(GPR::T4, 8 << 2, GPR::V1 ));

        // Set up another test to see if invalid is also written back
        write(&mut writer, false, 0, Assembler::make_lui(GPR::T0, 0x2323));
        writer.write(Assembler::make_cache(CacheOp::InstructionHitInvalidate, 12, GPR::A0));
        writer.write(Assembler::make_cache(CacheOp::InstructionHitWriteBack, 12, GPR::A0));
        writer.write(Assembler::make_lw(GPR::T5, 0 << 2, GPR::V1 ));

        writer.write(Assembler::make_ori(GPR::RA, GPR::A2, 0));
        writer.write(Assembler::make_jr(GPR::RA));
        writer.write(Assembler::make_nop());

        soft_assert_less(GENERATOR + (writer.index() as usize), GENERATED, "Error in test: Not enough room for generator")?;

        let result1: u32;
        let result2_tag_lo: u32;
        let result2: u32;
        let result3: u32;
        let result4: u32;
        unsafe {
            asm!("
            .set noat
            .set noreorder
            OR $5, $31, $0
            JALR $2
            NOP
            OR $31, $5, $0
        ",
            in("$2") MemoryMap::physical_to_cached::<u32>(memory.start_physical() + (GENERATOR << 2)),
            in("$3") MemoryMap::physical_to_uncached_mut::<u32>(memory.start_physical() + (GENERATED << 2)),
            in("$4") MemoryMap::physical_to_cached::<u32>(memory.start_physical() + (GENERATED << 2)),
            // RA temp for inline asm
            out("$5") _,
            // RA temp for generator
            out("$6") _,
            // temp for generator
            out("$7") _,
            // temp for return value from inner generated function
            out("$8") _,
            out("$9") result1,
            out("$10") result2_tag_lo,
            out("$11") result2,
            out("$12") result3,
            out("$13") result4,
            )
        }

        soft_assert_eq(result1, Assembler::make_lui(GPR::T0, 0x2222), "Cache(InstructionHitWriteBack) shouldn't do anything if the tag doesn't match")?;
        soft_assert_eq(result2_tag_lo, TagLo::new().with_p_tag_lo(ptag).with_p_state(TagLoPState::Clean).raw_value(), "TagLo after Cache(InstructionHitWriteBack)")?;
        soft_assert_eq(result2, Assembler::make_lui(GPR::T0, 0x1111), "Cache(InstructionHitWriteBack) should write back if the tag matches")?;
        soft_assert_eq(result3, Assembler::make_lui(GPR::T0, 0x6666), "Cache(InstructionHitWriteBack) should not write back to the address after the block")?;
        soft_assert_eq(result4, Assembler::make_lui(GPR::T0, 0x2323), "Cache(InstructionHitWriteBack) should not write back if invalid")?;

        Ok(())
    }
}

