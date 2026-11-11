// Base timing of every CACHE operation (except Fill, which is memory bound by definition).

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;

use super::{effective_cycles, ExceptionTimingMode, MeasurementProgram};
use crate::assembler::{Assembler, GPR};
use crate::cop0::{dcache_index_writeback_invalidate_range, CacheOp, RegisterIndex, Status};
use crate::memory_map::MemoryMap;
use crate::tests::soft_asserts::{soft_assert_eq_decimal, soft_assert_range};
use crate::tests::{Level, Test};

/// Offset from $3 (V1, cached pointer to the harness memory) of a 16 byte D-cache line that
/// nothing else uses (u64 slots 4 and 5; the harness memory is 64 byte aligned). A fresh harness
/// memory is created for every run, so touching it is fine.
const D_OFFSET: i16 = 32;

/// The preconditions are generated in two passes (the I-cache target depends on where the program
/// ended up in memory), so they always have this exact length (padded with NOPs).
const PRE_LEN: usize = 16;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Op {
    IIndexInvalidate,
    DIndexWriteBackInvalidate,
    IIndexLoadTag,
    DIndexLoadTag,
    IIndexStoreTag,
    DIndexStoreTag,
    DCreateDirtyExclusive,
    IHitInvalidate,
    DHitInvalidate,
    DHitWriteBackInvalidate,
    IHitWriteBack,
    DHitWriteBack,
}

impl Op {
    fn cache_op(self) -> CacheOp {
        match self {
            Op::IIndexInvalidate => CacheOp::InstructionIndexInvalidate,
            Op::DIndexWriteBackInvalidate => CacheOp::DataIndexWriteBackInvalidate,
            Op::IIndexLoadTag => CacheOp::InstructionIndexLoadTag,
            Op::DIndexLoadTag => CacheOp::DataIndexLoadTag,
            Op::IIndexStoreTag => CacheOp::InstructionIndexStoreTag,
            Op::DIndexStoreTag => CacheOp::DataIndexStoreTag,
            Op::DCreateDirtyExclusive => CacheOp::DataCreateDirtyExclusive,
            Op::IHitInvalidate => CacheOp::InstructionHitInvalidate,
            Op::DHitInvalidate => CacheOp::DataHitInvalidate,
            Op::DHitWriteBackInvalidate => CacheOp::DataHitWriteBackInvalidate,
            Op::IHitWriteBack => CacheOp::InstructionHitWriteBack,
            Op::DHitWriteBack => CacheOp::DataHitWriteBack,
        }
    }

    fn is_instruction_cache(self) -> bool {
        matches!(
            self,
            Op::IIndexInvalidate
                | Op::IIndexLoadTag
                | Op::IIndexStoreTag
                | Op::IHitInvalidate
                | Op::IHitWriteBack
        )
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Setup {
    /// No preparation at all.
    AsIs,
    /// Targeted line invalid before every iteration: Hit ops miss, nothing to write back
    Invalid,
}

#[derive(Clone)]
struct Row {
    name: &'static str,
    op: Op,
    setup: Setup,
    /// None = not known yet. The test then fails and reports what it measured
    expected: Option<u32>,
}

use Op::*;
use Setup::*;

const ROWS: &[(&str, Op, Setup, Option<u32>)] = &[
    ("D Index Load Tag", DIndexLoadTag, AsIs, Some(6)),
    ("D Index Store Tag", DIndexStoreTag, AsIs, Some(6)),
    (
        "D Index WriteBack Invalidate",
        DIndexWriteBackInvalidate,
        AsIs,
        Some(5),
    ),
    (
        "D Create Dirty Exclusive",
        DCreateDirtyExclusive,
        AsIs,
        Some(7),
    ),
    ("D Hit Invalidate (miss)", DHitInvalidate, AsIs, Some(5)),
    (
        "D Hit WriteBack Invalidate (miss)",
        DHitWriteBackInvalidate,
        AsIs,
        Some(5),
    ),
    ("D Hit WriteBack (miss)", DHitWriteBack, AsIs, Some(5)),
    ("I Index Invalidate", IIndexInvalidate, AsIs, Some(6)),
    ("I Index Load Tag", IIndexLoadTag, AsIs, Some(7)),
    ("I Index Store Tag", IIndexStoreTag, AsIs, Some(7)),
    ("I Hit Invalidate (miss)", IHitInvalidate, AsIs, Some(6)),
    ("I Hit WriteBack (miss)", IHitWriteBack, AsIs, Some(6)),
];

/// Picks an offset from T9 (the body address) whose I-cache index (16 KB, 32 byte lines, 512
/// lines) is far away from the measurement program and from the exception vector lines
/// (indexes 0..16). Index and Hit operations on a line of the measurement program itself would
/// make the measurement wrong.
fn pick_icache_offset(body_addr: usize) -> Option<i16> {
    let body_line = (body_addr >> 5) & 0x1FF;
    for offset in [-0x3000i32, -0x2800, -0x2000, -0x1800, -0x1000, -0x0800] {
        let target = (body_addr as i64 + offset as i64) as usize;
        if (target & 0x1FFF_FFFF) < 0x1000 {
            continue;
        }
        let line = (target >> 5) & 0x1FF;
        let distance = (line + 512 - body_line) % 512;
        if line >= 16 && distance > 16 && distance < 512 - 16 {
            return Some(offset as i16);
        }
    }
    None
}

/// Runs before every measured iteration, outside the measured window. It is called via
/// JALR RA, S3, so RA is free; V0/V1/T9 must stay untouched.
fn preconditions(op: Op, setup: Setup, i_offset: i16) -> Vec<u32> {
    let nop = Assembler::make_nop();
    let mut c: Vec<u32> = Vec::with_capacity(PRE_LEN);
    if setup == Invalid {
        if op.is_instruction_cache() {
            c.push(Assembler::make_cache(
                CacheOp::InstructionIndexInvalidate,
                i_offset,
                GPR::T9,
            ));
            c.extend_from_slice(&[nop; 3]);
        } else {
            // TagLo/TagHi = 0 (invalid state), then store that tag into the line
            c.push(Assembler::make_mtc0(GPR::R0, RegisterIndex::TagLo));
            c.push(Assembler::make_mtc0(GPR::R0, RegisterIndex::TagHi));
            c.extend_from_slice(&[nop; 3]); // CP0 hazard before the CACHE below
            c.push(Assembler::make_cache(
                CacheOp::DataIndexStoreTag,
                D_OFFSET,
                GPR::V1,
            ));

            c.extend_from_slice(&[nop; 3]);
        }
    }
    assert!(c.len() <= PRE_LEN);
    c.resize(PRE_LEN, nop);
    c
}

fn run_row(row: &Row) -> Result<(), String> {
    let nop = Assembler::make_nop();

    // Pass 1: build the program with placeholders of the final size, to learn where it lives
    let placeholder = vec![nop; PRE_LEN];
    let mut program = MeasurementProgram::new(Some(&placeholder), &[nop]);
    let program_addr = program.word_offset * 4
        + MemoryMap::physical_to_cached::<u8>(program.buffer.start_physical()) as usize;
    let body_addr = program_addr + program.body_offset * 4;

    // Pass 2: the real instruction and preconditions. Nothing has been executed yet, so nothing
    // from the buffer can be in the icache.
    let i_offset = if row.op.is_instruction_cache() {
        pick_icache_offset(body_addr)
            .ok_or_else(|| String::from("No I-cache index away from the program found"))?
    } else {
        0
    };
    let (offset, base) = if row.op.is_instruction_cache() {
        (i_offset, GPR::T9)
    } else if row.setup == AsIs {
        (0, GPR::V1)
    } else {
        (D_OFFSET, GPR::V1)
    };
    let body = Assembler::make_cache(row.op.cache_op(), offset, base);
    let pre = preconditions(row.op, row.setup, i_offset);

    // Layout: [loop][body][JR RA][NOP][preconditions][JR RA][NOP]
    let body_index = program.word_offset + program.body_offset;
    let pre_index = body_index + 1 + 2;
    program.buffer.write(body_index, body);
    for (i, word) in pre.iter().enumerate() {
        program.buffer.write(pre_index + i, *word);
    }

    let (ticks1, ticks2) = program.run(0, 0, Status::DEFAULT, ExceptionTimingMode::Off);

    // "Create Dirty Exclusive" leaves a valid *dirty* line for the harness memory behind. That
    // memory is freed when `program` is dropped, so a later eviction of the line would write stale
    // data over whatever gets allocated there next. Write it back while the memory is still ours.
    // CHECK: the range helper's semantics (index write back + invalidate, start = cached address)
    unsafe {
        dcache_index_writeback_invalidate_range(
            MemoryMap::physical_to_cached::<u8>(program.memory.start_physical()) as usize,
            10 * 8, // the 10 u64 slots of the harness memory
        );
    }

    // If the state isn't restored identically in every iteration, these two differ
    soft_assert_range(
        ticks2,
        ticks1,
        ticks1 + 1,
        "Two measurements aren't close. The preconditions don't restore the cache state",
    )?;

    let measured = effective_cycles(ticks1, ticks2, ExceptionTimingMode::Off);
    match row.expected {
        Some(expected) => soft_assert_eq_decimal(measured, expected, "Measured cycles"),
        None => Err(format!(
            "No expected value yet. Measured {} cycles",
            measured
        )),
    }
}

pub struct CacheInstructionTiming {}

impl Test for CacheInstructionTiming {
    fn name(&self) -> &str {
        "Timing: CACHE instruction operations"
    }

    fn level(&self) -> Level {
        Level::Timing
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        ROWS.iter()
            .map(|&(name, op, setup, expected)| {
                Box::new(Row {
                    name,
                    op,
                    setup,
                    expected,
                }) as Box<dyn Any>
            })
            .collect()
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        match (*value).downcast_ref::<Row>() {
            Some(row) => run_row(row).map_err(|e| format!("{}:{}", row.name, e)),
            None => Err(String::from("Unexpected pattern")),
        }
    }
}
