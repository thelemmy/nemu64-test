use arbitrary_int::{u20, u29};
use alloc::alloc::{alloc, dealloc, Layout};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;
use crate::assembler::{Assembler, GPR};
use crate::cop0::{cache, cache_data_index_load_tag, CacheOp, RegisterIndex, set_taglo, set_taglo64, TagLo, TagLoPState};
use crate::math::KB;

use crate::memory_map::MemoryMap;
use crate::tests::{Level, Test};
use crate::tests::soft_asserts::{soft_assert_eq, soft_assert_eq2};
use crate::uncached_memory::UncachedHeapMemory;

const GPR_CACHED: GPR = GPR::V1;
const GPR_UNCACHED: GPR = GPR::A0;

pub struct ReadCachedVsUncached {}

impl Test for ReadCachedVsUncached {
    fn name(&self) -> &str { "cache: Read cached vs uncached" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let layout = Layout::from_size_align(16 * KB, 16).unwrap();
        let data = unsafe { alloc(layout) } as *mut u8;

        unsafe {
            asm!("
            .set noat
            .set noreorder
.macro StashRegisters index
            SD {lb}, 1024 + \\index * 88 + 0($4)
            SD {lbu}, 1024 + \\index * 88 + 8($4)
            SD {lh}, 1024 + \\index * 88 + 16($4)
            SD {lhu}, 1024 + \\index * 88 + 24($4)
            SD {lw}, 1024 + \\index * 88 + 32($4)
            SD {lwu}, 1024 + \\index * 88 + 40($4)
            SD {ld}, 1024 + \\index * 88 + 48($4)
            SD {ll}, 1024 + \\index * 88 + 56($4)
            SD {lld}, 1024 + \\index * 88 + 64($4)
            SDC1 $f0, 1024 + \\index * 88 + 72($4)
            SDC1 $f2, 1024 + \\index * 88 + 80($4)
.endm
.macro LA64 reg, value_3, value_2, value_1, value_0
            LUI \\reg, \\value_3
            ORI \\reg, \\reg, \\value_2
            DSLL \\reg, \\reg, 16
            ORI \\reg, \\reg, \\value_1
            DSLL \\reg, \\reg, 16
            ORI \\reg, \\reg, \\value_0
.endm
.macro PerformStores reg, base_register, value_3, value_2, value_1, value_0
            LA64 \\reg, \\value_3, \\value_2, \\value_1, \\value_0
            DMTC1 \\reg, $f0
            SB \\reg, 0 * 16 + 3(\\base_register)
            SB \\reg, 1 * 16 + 1(\\base_register)
            SH \\reg, 2 * 16 + 2(\\base_register)
            SH \\reg, 3 * 16 + 0(\\base_register)
            SW \\reg, 4 * 16 + 4(\\base_register)
            SW \\reg, 5 * 16 + 0(\\base_register)
            SD \\reg, 6 * 16 + 0(\\base_register)
            SC \\reg, 7 * 16 + 4(\\base_register)
            LA64 \\reg, \\value_3, \\value_2, \\value_1, \\value_0
            SCD \\reg, 8 * 16 + 0(\\base_register)
            SWC1 $f0, 9 * 16 + 4(\\base_register)
            SDC1 $f0, 10 * 16 + 0(\\base_register)
.endm
.macro PerformLoads base_register
            LB {lb}, 0 * 16 + 3(\\base_register)
            LBU {lbu}, 1 * 16 + 1(\\base_register)
            LH {lh}, 2 * 16 + 2(\\base_register)
            LHU {lhu}, 3 * 16 + 0(\\base_register)
            LW {lw}, 4 * 16 + 4(\\base_register)
            LWU {lwu}, 5 * 16 + 0(\\base_register)
            LD {ld}, 6 * 16 + 0(\\base_register)
            LL {ll}, 7 * 16 + 4(\\base_register)
            LLD {lld}, 8 * 16 + 0(\\base_register)
            DMTC1 $0, $f0
            LWC1 $f0, 9 * 16 + 4(\\base_register)
            LDC1 $f2, 10 * 16 + 0(\\base_register)
.endm

            // Make sure LLBit is set
            LL $0, 0($3)

            // Write to cached
            PerformStores {scratch}, $3, 0x0011, 0x2233, 0x4455, 0x6677

            // Write to uncached
            PerformStores {scratch}, $4, 0xAABB, 0xCCDD, 0xABCD, 0xEFED

            // Read cached and uncached: These should now see different values
            PerformLoads $3
            StashRegisters 0
            PerformLoads $4
            StashRegisters 1

            // Invalidate the the cache lines. This will write them back, overwriting the uncached writes
            CACHE 1, 0 * 16($3)
            CACHE 1, 1 * 16($3)
            CACHE 1, 2 * 16($3)
            CACHE 1, 3 * 16($3)
            CACHE 1, 4 * 16($3)
            CACHE 1, 5 * 16($3)
            CACHE 1, 6 * 16($3)
            CACHE 1, 7 * 16($3)
            CACHE 1, 8 * 16($3)
            CACHE 1, 9 * 16($3)
            CACHE 1, 10 * 16($3)

            // Read cached and uncached: These should now see the same values (the uncached ones being dropped)
            PerformLoads $3
            StashRegisters 2
            PerformLoads $4
            StashRegisters 3

            // Read from uncached - these should see the uncached writes
            ",
            lb = out(reg) _,
            lbu = out(reg) _,
            lh = out(reg) _,
            lhu = out(reg) _,
            lw = out(reg) _,
            lwu = out(reg) _,
            ld = out(reg) _,
            ll = out(reg) _,
            lld = out(reg) _,

            scratch = out(reg) _,

            out("$f0") _,
            out("$f2") _,
            out("$f4") _,
            out("$f5") _,

            in("$3") data,
            in("$4") MemoryMap::uncached_mut(data),
            )
        }

        unsafe { dealloc(data, layout); }

        fn get64(v: *mut u8, offset: usize) -> u64 {
            unsafe { *(v.add(offset) as *const u64) }
        }

        fn assert_batch(data: *mut u8, index: usize, is_cached: bool) -> Result<(), String> {
            soft_assert_eq2(get64(data, 1024 + index * 88 + 0), if is_cached { 0x77 } else { 0xFFFFFFFF_FFFFFFED }, || format!("LB {}", index))?;
            soft_assert_eq2(get64(data, 1024 + index * 88 + 8), if is_cached { 0x77 } else { 0xED }, || format!("LBU {}", index))?;
            soft_assert_eq2(get64(data, 1024 + index * 88 + 16), if is_cached { 0x6677 } else { 0xFFFFFFFF_FFFFEFED }, || format!("LH {}", index))?;
            soft_assert_eq2(get64(data, 1024 + index * 88 + 24), if is_cached { 0x6677 } else { 0xEFED }, || format!("LHU {}", index))?;
            soft_assert_eq2(get64(data, 1024 + index * 88 + 32), if is_cached { 0x44556677 } else { 0xFFFFFFFF_ABCDEFED }, || format!("LW {}", index))?;
            soft_assert_eq2(get64(data, 1024 + index * 88 + 40), if is_cached { 0x44556677 } else { 0xABCDEFED }, || format!("LWU {}", index))?;
            soft_assert_eq2(get64(data, 1024 + index * 88 + 48), if is_cached { 0x00112233_44556677 } else { 0xAABBCCDD_ABCDEFED }, || format!("LD {}", index))?;
            soft_assert_eq2(get64(data, 1024 + index * 88 + 56), if is_cached { 0x44556677 } else { 0xFFFFFFFF_ABCDEFED }, || format!("LL {}", index))?;
            soft_assert_eq2(get64(data, 1024 + index * 88 + 64), if is_cached { 0x00112233_44556677 } else { 0xAABBCCDD_ABCDEFED }, || format!("LLD {}", index))?;
            soft_assert_eq2(get64(data, 1024 + index * 88 + 72), if is_cached { 0x44556677 } else { 0xABCDEFED }, || format!("LWC1 {}", index))?;
            soft_assert_eq2(get64(data, 1024 + index * 88 + 80), if is_cached { 0x00112233_44556677 } else { 0xAABBCCDD_ABCDEFED }, || format!("LDC1 {}", index))?;

            Ok(())
        }


        assert_batch(data, 0, true)?;
        assert_batch(data, 1, false)?;
        assert_batch(data, 2, true)?;
        assert_batch(data, 3, true)?;

        Ok(())
    }
}

fn test_writeback<const INSTRUCTION: u32>(expect_writeback: bool) -> Result<(), String>{
    let layout = Layout::from_size_align(16 * KB, 16).unwrap();
    let data = unsafe { alloc(layout) } as *mut u8;

    let out0_0: u32;
    let out0_4: u32;
    let out0_8: u32;
    let out0_12: u32;
    let out0_16: u32;
    let out1_0: u32;
    let out1_4: u32;
    let out1_8: u32;
    let out1_12: u32;
    let out1_16: u32;
    unsafe {
        asm!("
            .set noat
            .set noreorder
            // Clear uncached
            SW $0, 0($4)
            SW $0, 4($4)
            SW $0, 8($4)
            SW $0, 12($4)
            SW $0, 16($4)

            // Write to cached
            ORI {scratch}, $0, 0x1234
            SW {scratch}, 0($3)
            ORI {scratch}, $0, 0x2345
            SW {scratch}, 4($3)
            ORI {scratch}, $0, 0x3456
            SW {scratch}, 8($3)
            ORI {scratch}, $0, 0x4567
            SW {scratch}, 12($3)
            ORI {scratch}, $0, 0x5678
            SW {scratch}, 16($3)

            // Read back from uncached
            LW {out0_0}, 0($4)
            LW {out0_4}, 4($4)
            LW {out0_8}, 8($4)
            LW {out0_12}, 12($4)
            LW {out0_16}, 16($4)

            // Perform the writeback
            .word {INSTRUCTION}

            // Read from uncached again - this time we should actually get the value,
            // but only for the first 4 as they are within the same cache line
            LW {out1_0}, 0($4)
            LW {out1_4}, 4($4)
            LW {out1_8}, 8($4)
            LW {out1_12}, 12($4)
            LW {out1_16}, 16($4)
            ",
        out0_0 = out(reg) out0_0,
        out0_4 = out(reg) out0_4,
        out0_8 = out(reg) out0_8,
        out0_12 = out(reg) out0_12,
        out0_16 = out(reg) out0_16,
        out1_0 = out(reg) out1_0,
        out1_4 = out(reg) out1_4,
        out1_8 = out(reg) out1_8,
        out1_12 = out(reg) out1_12,
        out1_16 = out(reg) out1_16,
        scratch = out(reg) _,
        INSTRUCTION = const INSTRUCTION,
        in("$3") data,
        in("$4") MemoryMap::uncached_mut(data),
        )
    }

    unsafe { dealloc(data, layout); }

    soft_assert_eq(out0_0, 0, "Read from uncached shouldn't see the cached value until cache line is written back (0)")?;
    soft_assert_eq(out0_4, 0, "Read from uncached shouldn't see the cached value until cache line is written back (4)")?;
    soft_assert_eq(out0_8, 0, "Read from uncached shouldn't see the cached value until cache line is written back (8)")?;
    soft_assert_eq(out0_12, 0, "Read from uncached shouldn't see the cached value until cache line is written back (12)")?;
    soft_assert_eq(out0_16, 0, "Read from uncached shouldn't see the cached value until cache line is written back (16)")?;
    if expect_writeback {
        soft_assert_eq(out1_0, 0x1234, "Cache line should have been written back by now (0)")?;
        soft_assert_eq(out1_4, 0x2345, "Cache line should have been written back by now (4)")?;
        soft_assert_eq(out1_8, 0x3456, "Cache line should have been written back by now (8)")?;
        soft_assert_eq(out1_12, 0x4567, "Cache line should have been written back by now (12)")?;
    } else {
        soft_assert_eq(out1_0, 0, "Cache line should not have been written back by now (0)")?;
        soft_assert_eq(out1_4, 0, "Cache line should not have been written back by now (4)")?;
        soft_assert_eq(out1_8, 0, "Cache line should not have been written back by now (8)")?;
        soft_assert_eq(out1_12, 0, "Cache line should not have been written back by now (12)")?;
    }
    soft_assert_eq(out1_16, 0, "Second cache line shouldn't have been written back (16)")?;

    Ok(())
}

pub struct WriteBackLB {}

impl Test for WriteBackLB {
    fn name(&self) -> &str { "data cache: Write back (Using LB 8kb+15 later)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_lb(GPR::R0, (8 * KB + 15) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackLBU {}

impl Test for WriteBackLBU {
    fn name(&self) -> &str { "data cache: Write back (Using LBU 8kb+15 later)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_lbu(GPR::R0, (8 * KB + 15) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackLH {}

impl Test for WriteBackLH {
    fn name(&self) -> &str { "data cache: Write back (Using LH 8kb+14 later)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_lh(GPR::R0, (8 * KB + 14) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackLHU {}

impl Test for WriteBackLHU {
    fn name(&self) -> &str { "data cache: Write back (Using LW 8kb+14 later)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_lhu(GPR::R0, (8 * KB + 14) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackLW {}

impl Test for WriteBackLW {
    fn name(&self) -> &str { "data cache: Write back (Using LW 8kb+12 later)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_lw(GPR::R0, (8 * KB + 12) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackLWU {}

impl Test for WriteBackLWU {
    fn name(&self) -> &str { "data cache: Write back (Using LWU 8kb+12 later)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_lwu(GPR::R0, (8 * KB + 12) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackLD {}

impl Test for WriteBackLD {
    fn name(&self) -> &str { "data cache: Write back (Using LD 8kb+8 later)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_ld(GPR::R0, (8 * KB + 8) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackSB {}

impl Test for WriteBackSB {
    fn name(&self) -> &str { "data cache: Write back (Using SB 8kb+15 later)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_sb(GPR::R0, (8 * KB + 15) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackSH {}

impl Test for WriteBackSH {
    fn name(&self) -> &str { "data cache: Write back (Using SH 8kb+14 later)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_sh(GPR::R0, (8 * KB + 14) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackSW {}

impl Test for WriteBackSW {
    fn name(&self) -> &str { "data cache: Write back (Using SW 8kb+12 later)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_sw(GPR::R0, (8 * KB + 12) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackSD {}

impl Test for WriteBackSD {
    fn name(&self) -> &str { "data cache: Write back (Using SD 8kb+8 later)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_sd(GPR::R0, (8 * KB + 8) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackCacheDataIndexWriteBackInvalidate {}

impl Test for WriteBackCacheDataIndexWriteBackInvalidate {
    fn name(&self) -> &str { "data cache: Write back (Using CACHE with Data-Index Write Back Invalidate)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_cache(CacheOp::DataIndexWriteBackInvalidate, 12 as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackCacheDataIndexWriteBackInvalidateUncachedAddress {}

impl Test for WriteBackCacheDataIndexWriteBackInvalidateUncachedAddress {
    fn name(&self) -> &str { "data cache: Write back (Using CACHE with Data-Index Write Back Invalidate with uncached address)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_cache(CacheOp::DataIndexWriteBackInvalidate, 12 as i16, GPR_UNCACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct WriteBackCacheDataIndexWriteBackInvalidateNextBlock {}

impl Test for WriteBackCacheDataIndexWriteBackInvalidateNextBlock {
    fn name(&self) -> &str { "data cache: Write back (Using CACHE with Data-Index Write Back Invalidate)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const INSTRUCTION: u32 = Assembler::make_cache(CacheOp::DataIndexWriteBackInvalidate, (8 * KB + 8) as i16, GPR_CACHED);
        test_writeback::<INSTRUCTION>(true)
    }
}

pub struct DataCacheIndexLoadTag {}

impl Test for DataCacheIndexLoadTag {
    fn name(&self) -> &str { "data cache: Cache Index Load Tag" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let mut data = UncachedHeapMemory::<u8>::new_with_align(16 * KB, 8 * KB);
        let physical = data.start_phyiscal();
        let cached = MemoryMap::physical_to_cached_mut::<u8>(physical);

        const HIT_WRITE_BACK_INVALIDATE_OP: u8 = CacheOp::DataHitWriteBackInvalidate.raw_value().value();

        for offset in [0, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 1024 * 2, 1024 * 3, 1024 * 4, 1024 * 5, 1024 * 6, 1024 * 7, 1024 * 8, 1024 * 8 + 4, 1024 * 9, 1024 * 13] {
            unsafe {
                set_taglo64(0x12345678_FFFFFFFF);
                let expected_ptaglo = u20::extract_u32((physical + offset) as u32, 12);
                let p = cached.add(offset);

                // Load from the address (to ensure cache line is read) then invalidate to make it invalid
                let _ = p.read_volatile();
                cache::<HIT_WRITE_BACK_INVALIDATE_OP, 0>(p as usize);
                let tag1 = cache_data_index_load_tag::<0>(p as usize);

                soft_assert_eq(tag1, TagLo::new().with_p_state(TagLoPState::Invalid).with_p_tag_lo(expected_ptaglo), "TagLo after CACHE (invalid)")?;

                // Read - Even though it is clear, it will come back as dirty
                let _ = p.read_volatile();
                let tag2 = cache_data_index_load_tag::<0>(p as usize);
                soft_assert_eq(tag2, TagLo::new().with_p_state(TagLoPState::Dirty).with_p_tag_lo(expected_ptaglo), "TagLo after CACHE (clean)")?;

                // Write to make it dirty - TagLo won't tell us though
                p.write_volatile(0);
                let tag3 = cache_data_index_load_tag::<0>(p as usize);
                soft_assert_eq(tag3, TagLo::new().with_p_state(TagLoPState::Dirty).with_p_tag_lo(expected_ptaglo), "TagLo after CACHE (dirty)")?;
            }
        }

        Ok(())
    }
}

fn test_cache_instruction<const CACHE_OP: u8, const N: usize>(expected_tag_offset_and_status: [u32; N], expected_result0: [u32; 6], expected_result1: [u32; 6], expected_result2: [u32; 6]) -> Result<(), String> {
    let mut data = UncachedHeapMemory::<u8>::new_with_align(24 * KB, 8 * KB);
    let physical = data.start_phyiscal();
    let cached = MemoryMap::physical_to_cached_mut::<u8>(physical);
    let uncached = MemoryMap::physical_to_uncached_mut::<u8>(physical);

    const INDEX_LOAD_TAG_OP: u8 = CacheOp::DataIndexLoadTag.raw_value().value();
    const INDEX_WRITE_BACK_INVALIDATE_OP: u8 = CacheOp::DataIndexWriteBackInvalidate.raw_value().value();

    let mut result_tag = [0u32; 6];
    let mut result_0 = [0u32; 6];
    let mut result_1 = [0u32; 6];
    let mut result_2 = [0u32; 6];
    unsafe {
        asm!("
            .set noat

            // Ensure nothing is cached right now
            CACHE {INDEX_WRITE_BACK_INVALIDATE_OP}, 0 * 16 ({cached})
            CACHE {INDEX_WRITE_BACK_INVALIDATE_OP}, 1 * 16 ({cached})
            CACHE {INDEX_WRITE_BACK_INVALIDATE_OP}, 2 * 16 ({cached})
            CACHE {INDEX_WRITE_BACK_INVALIDATE_OP}, 3 * 16 ({cached})
            CACHE {INDEX_WRITE_BACK_INVALIDATE_OP}, 4 * 16 ({cached})
            CACHE {INDEX_WRITE_BACK_INVALIDATE_OP}, 5 * 16 ({cached})

            // Zero memory beforehand
            SW $0, 0 * 16 ({uncached})
            SW $0, 1 * 16 ({uncached})
            SW $0, 2 * 16 ({uncached})
            SW $0, 3 * 16 ({uncached})
            SW $0, 4 * 16 ({uncached})
            SW $0, 5 * 16 ({uncached})
            SW $0, 8 * 1024 + 0 * 16 ({uncached})
            SW $0, 8 * 1024 + 1 * 16 ({uncached})
            SW $0, 8 * 1024 + 2 * 16 ({uncached})
            SW $0, 8 * 1024 + 3 * 16 ({uncached})
            SW $0, 8 * 1024 + 4 * 16 ({uncached})
            SW $0, 8 * 1024 + 5 * 16 ({uncached})

            // Prepare cache lines: Dirty/Clean/Invalid, Clean/Dirty/Invalid
            ORI {scratch}, $0, 0x222
            SW {scratch}, 0 * 16 ({cached})
            LW {scratch}, 1 * 16 ({cached})

            SW {scratch}, 3 * 16 ({cached})
            LW {scratch}, 4 * 16 ({cached})

            // Prepare uncached memory
            ORI {scratch}, $0, 0x111
            SW {scratch}, 0 * 16 ({uncached})
            SW {scratch}, 1 * 16 ({uncached})
            SW {scratch}, 2 * 16 ({uncached})
            SW {scratch}, 3 * 16 ({uncached})
            SW {scratch}, 4 * 16 ({uncached})
            SW {scratch}, 5 * 16 ({uncached})

            // Run cache: Three hits, three misses
            CACHE {CACHE_OP}, 0 * 16 ({cached})
            CACHE {CACHE_OP}, 1 * 16 ({cached})
            CACHE {CACHE_OP}, 2 * 16 ({cached})
            CACHE {CACHE_OP}, 8 * 1024 + 3 * 16 ({cached})
            CACHE {CACHE_OP}, 8 * 1024 + 4 * 16 ({cached})
            CACHE {CACHE_OP}, 8 * 1024 + 5 * 16 ({cached})

            // Read the six cache lines
            cache {INDEX_LOAD_TAG_OP}, 0 * 16 ({cached}); nop; mfc0 {result_tag0}, ${COP0REG}
            cache {INDEX_LOAD_TAG_OP}, 1 * 16 ({cached}); nop; mfc0 {result_tag1}, ${COP0REG}
            cache {INDEX_LOAD_TAG_OP}, 2 * 16 ({cached}); nop; mfc0 {result_tag2}, ${COP0REG}
            cache {INDEX_LOAD_TAG_OP}, 3 * 16 ({cached}); nop; mfc0 {result_tag3}, ${COP0REG}
            cache {INDEX_LOAD_TAG_OP}, 4 * 16 ({cached}); nop; mfc0 {result_tag4}, ${COP0REG}
            cache {INDEX_LOAD_TAG_OP}, 5 * 16 ({cached}); nop; mfc0 {result_tag5}, ${COP0REG}

            // Read the memory values to see if the cache line was written back
            LW {r0_0}, 0 * 16 ({uncached})
            LW {r0_1}, 1 * 16 ({uncached})
            LW {r0_2}, 2 * 16 ({uncached})
            LW {r0_3}, 3 * 16 ({uncached})
            LW {r0_4}, 4 * 16 ({uncached})
            LW {r0_5}, 5 * 16 ({uncached})

            // Still need to figure out if cache line is now dirty or clean. Test that by clearing memory
            // and then moving the cache line
            ORI {scratch}, $0, 1
            SW {scratch}, 0 * 16 ({uncached})
            SW {scratch}, 1 * 16 ({uncached})
            SW {scratch}, 2 * 16 ({uncached})
            SW {scratch}, 3 * 16 ({uncached})
            SW {scratch}, 4 * 16 ({uncached})
            SW {scratch}, 5 * 16 ({uncached})
            SW {scratch}, 8 * 1024 + 0 * 16 ({uncached})
            SW {scratch}, 8 * 1024 + 1 * 16 ({uncached})
            SW {scratch}, 8 * 1024 + 2 * 16 ({uncached})
            SW {scratch}, 8 * 1024 + 3 * 16 ({uncached})
            SW {scratch}, 8 * 1024 + 4 * 16 ({uncached})
            SW {scratch}, 8 * 1024 + 5 * 16 ({uncached})
            LW $0, 16 * 1024 + 0 * 16 ({cached})
            LW $0, 16 * 1024 + 1 * 16 ({cached})
            LW $0, 16 * 1024 + 2 * 16 ({cached})
            LW $0, 16 * 1024 + 3 * 16 ({cached})
            LW $0, 16 * 1024 + 4 * 16 ({cached})
            LW $0, 16 * 1024 + 5 * 16 ({cached})
            LW {r1_0}, 0 * 16 ({uncached})
            LW {r1_1}, 1 * 16 ({uncached})
            LW {r1_2}, 2 * 16 ({uncached})
            LW {r1_3}, 3 * 16 ({uncached})
            LW {r1_4}, 4 * 16 ({uncached})
            LW {r1_5}, 5 * 16 ({uncached})
            LW {r2_0}, 8 * 1024 + 0 * 16 ({uncached})
            LW {r2_1}, 8 * 1024 + 1 * 16 ({uncached})
            LW {r2_2}, 8 * 1024 + 2 * 16 ({uncached})
            LW {r2_3}, 8 * 1024 + 3 * 16 ({uncached})
            LW {r2_4}, 8 * 1024 + 4 * 16 ({uncached})
            LW {r2_5}, 8 * 1024 + 5 * 16 ({uncached})
            ",
        cached = in(reg) cached,
        uncached = in(reg) uncached,
        scratch = out(reg) _,
        result_tag0 = out(reg) result_tag[0],
        result_tag1 = out(reg) result_tag[1],
        result_tag2 = out(reg) result_tag[2],
        result_tag3 = out(reg) result_tag[3],
        result_tag4 = out(reg) result_tag[4],
        result_tag5 = out(reg) result_tag[5],
        r0_0 = out(reg) result_0[0],
        r0_1 = out(reg) result_0[1],
        r0_2 = out(reg) result_0[2],
        r0_3 = out(reg) result_0[3],
        r0_4 = out(reg) result_0[4],
        r0_5 = out(reg) result_0[5],
        r1_0 = out(reg) result_1[0],
        r1_1 = out(reg) result_1[1],
        r1_2 = out(reg) result_1[2],
        r1_3 = out(reg) result_1[3],
        r1_4 = out(reg) result_1[4],
        r1_5 = out(reg) result_1[5],
        r2_0 = out(reg) result_2[0],
        r2_1 = out(reg) result_2[1],
        r2_2 = out(reg) result_2[2],
        r2_3 = out(reg) result_2[3],
        r2_4 = out(reg) result_2[4],
        r2_5 = out(reg) result_2[5],
        COP0REG = const RegisterIndex::TagLo.raw_value().value(),
        CACHE_OP = const CACHE_OP,
        INDEX_WRITE_BACK_INVALIDATE_OP = const INDEX_WRITE_BACK_INVALIDATE_OP,
        INDEX_LOAD_TAG_OP = const INDEX_LOAD_TAG_OP,
        )
    }

    let base_result = ((cached as u32) & u29::MAX.value()) >> 4;
    // crate::println!("tag: {:x?}", result_tag);
    // crate::println!("r0: {:x?}", result_0);
    // crate::println!("r1: {:x?}", result_1);
    // crate::println!("r2: {:x?}", result_2);

    let expected_tag = expected_tag_offset_and_status.map(|x| x + base_result);
    assert!(N <= 6);
    soft_assert_eq(&result_tag[0..N], &expected_tag, "Tag of cache line after cache instruction")?;
    soft_assert_eq(result_0, expected_result0, "Value in memory after cache instruction")?;
    soft_assert_eq(result_1, expected_result1, "Value in memory (1) after cache was invalidated")?;
    soft_assert_eq(result_2, expected_result2, "Value in memory (2) after cache was invalidated")?;

    Ok(())
}

pub struct DataCacheHitWriteBackInvalidate {}

impl Test for DataCacheHitWriteBackInvalidate {
    fn name(&self) -> &str { "data cache: Cache Hit Write Back Invalidate)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const OP: u8 = CacheOp::DataHitWriteBackInvalidate.raw_value().value();
        // The last tag_offset_and_status is undefined in this test (it depends on what happened before),
        // so ignore it
        test_cache_instruction::<OP, 5>(
            [0, 0, 0, 0xc0, 0xc0],
            [0x222, 0x111, 0x111, 0x111, 0x111, 0x111],
            [1, 1, 1, 0, 1, 1],
            [1, 1, 1, 1, 1, 1])
    }
}

pub struct DataCacheHitWriteBack {}

impl Test for DataCacheHitWriteBack {
    fn name(&self) -> &str { "data cache: Cache Hit Write Back)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const OP: u8 = CacheOp::DataHitWriteBack.raw_value().value();
        test_cache_instruction::<OP, 6>(
            [0xc0, 0xc0, 0, 0xc0, 0xc0, 0],
            [0x222, 0x111, 0x111, 0x111, 0x111, 0x111],
            [1, 1, 1, 0, 1, 1],
            [1, 1, 1, 1, 1, 1])
    }
}

pub struct DataCacheHitInvalidate {}

impl Test for DataCacheHitInvalidate {
    fn name(&self) -> &str { "data cache: Cache Hit Invalidate)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const OP: u8 = CacheOp::DataHitInvalidate.raw_value().value();
        test_cache_instruction::<OP, 6>(
            [0, 0, 0, 0xc0, 0xc0, 0],
            [0x111, 0x111, 0x111, 0x111, 0x111, 0x111],
            [1, 1, 1, 0, 1, 1],
            [1, 1, 1, 1, 1, 1])
    }
}

pub struct DataCacheIndexWriteBackInvalidate {}

impl Test for DataCacheIndexWriteBackInvalidate {
    fn name(&self) -> &str { "data cache: Index Write Back Invalidate)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const OP: u8 = CacheOp::DataIndexWriteBackInvalidate.raw_value().value();
        test_cache_instruction::<OP, 6>(
            [0, 0, 0, 0x200, 0x200, 0],
            [0x222, 0x111, 0x111, 0, 0x111, 0x111],
            [1, 1, 1, 1, 1, 1],
            [1, 1, 1, 1, 1, 1])
    }
}

pub struct DataCacheCreateDirtyExclusive {}

impl Test for DataCacheCreateDirtyExclusive {
    fn name(&self) -> &str { "data cache: Create Dirty Exclusive)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const OP: u8 = CacheOp::DataCreateDirtyExclusive.raw_value().value();
        test_cache_instruction::<OP, 6>(
            [0xc0, 0xc0, 0xc0, 0x2c0, 0x2c0, 0x2c0],
            [0x111, 0x111, 0x111, 0x0, 0x111, 0x111],
            [0x222, 1, 1, 1, 1, 1],
            [1, 1, 1, 1, 1, 1])
    }
}

fn test_write_cache_line_manually(p_state: TagLoPState) -> Result<(), String> {
    let layout = Layout::from_size_align(64, 8 * 1024).unwrap();
    let data = unsafe { alloc(layout) } as *mut u8;

    set_taglo(TagLo::new().with_p_state(p_state).with_p_tag_lo(u20::extract_u32(MemoryMap::cached_to_physical_mut(data) as u32, 12)));

    const CACHED_VALUE: u32 = 1;
    const UNCACHED_VALUE: u32 = 2;

    let cached: u32;
    let uncached: u32;
    let cached16: u32;
    let uncached16: u32;
    let cached32: u32;
    let uncached32: u32;
    unsafe {
        const STORE_CACHE_OP: u8 = CacheOp::DataIndexStoreTag.raw_value().value();
        const HIT_WRITE_BACK_CACHE_OP: u8 = CacheOp::DataHitWriteBack.raw_value().value();
        const HIT_WRITE_BACK_INVALIDATE_CACHE_OP: u8 = CacheOp::DataHitWriteBackInvalidate.raw_value().value();
        asm!("
            .set noat
            .set noreorder

            // Test 0: Write cached and uncached, CACHE (IndexStoreTag) and load both
            SW {cached_value}, 0 ($3)
            SW {uncached_value}, 0 ($4)
            CACHE {STORE_CACHE_OP}, 0($3)
            LW {cached}, 0 ($3)
            LW {uncached}, 0 ($4)

            // Test 16: Write cached and uncached, CACHE (IndexStoreTag), CACHE (HitWriteBack) and load both
            SW {cached_value}, 16 ($3)
            SW {uncached_value}, 16 ($4)
            CACHE {STORE_CACHE_OP}, 16($3)
            CACHE {HIT_WRITE_BACK_CACHE_OP}, 16($3)
            LW {cached16}, 16 ($3)
            LW {uncached16}, 16 ($4)

            // Test 32: Previous test showed we can't just mark a cache line as non-dirty. Try marking a non-dirty as dirty instead
            SW {cached_value}, 32 ($3)
            CACHE {HIT_WRITE_BACK_INVALIDATE_CACHE_OP}, 32($3)
            LW $0, 32 ($3)
            SW {uncached_value}, 32 ($4)
            CACHE {STORE_CACHE_OP}, 32($3)
            CACHE {HIT_WRITE_BACK_CACHE_OP}, 32($3)
            LW {cached32}, 32 ($3)
            LW {uncached32}, 32 ($4)
            ",
        cached_value = in(reg) 1,
        uncached_value = in(reg) 2,

        cached = out(reg) cached,
        uncached = out(reg) uncached,

        cached16 = out(reg) cached16,
        uncached16 = out(reg) uncached16,

        cached32 = out(reg) cached32,
        uncached32 = out(reg) uncached32,

        STORE_CACHE_OP = const STORE_CACHE_OP,
        HIT_WRITE_BACK_CACHE_OP = const HIT_WRITE_BACK_CACHE_OP,
        HIT_WRITE_BACK_INVALIDATE_CACHE_OP = const HIT_WRITE_BACK_INVALIDATE_CACHE_OP,

        in("$3") data,
        in("$4") MemoryMap::uncached_mut(data),
        )
    }

    unsafe { dealloc(data, layout); }

    // It seems that writing Clean or Dirty is exactly the same; so is Invalid and _Unused01

    if p_state == TagLoPState::Invalid || p_state == TagLoPState::_Unused01 {
        // Writing invalid to the cache line always drops the cache line
        soft_assert_eq(cached, UNCACHED_VALUE, "Cached value after writing to cache line manually (0)")?;
        soft_assert_eq(uncached, UNCACHED_VALUE, "Uncached value after writing to cache line manually (0)")?;
        soft_assert_eq(cached16, UNCACHED_VALUE, "Cached value after writing to cache line manually (16)")?;
        soft_assert_eq(uncached16, UNCACHED_VALUE, "Uncached value after writing to cache line manually (16)")?;
        soft_assert_eq(cached32, UNCACHED_VALUE, "Cached value after writing to cache line manually (32)")?;
        soft_assert_eq(uncached32, UNCACHED_VALUE, "Uncached value after writing to cache line manually (32)")?;
    } else {
        soft_assert_eq(cached, CACHED_VALUE, "Cached value after writing to cache line manually (0)")?;
        soft_assert_eq(uncached, UNCACHED_VALUE, "Uncached value after writing to cache line manually (0)")?;
        soft_assert_eq(cached16, CACHED_VALUE, "Cached value after writing to cache line manually (16)")?;
        soft_assert_eq(uncached16, CACHED_VALUE, "Uncached value after writing to cache line manually (16)")?;
        soft_assert_eq(cached32, CACHED_VALUE, "Cached value after writing to cache line manually (32)")?;
        soft_assert_eq(uncached32, UNCACHED_VALUE, "Uncached value after writing to cache line manually (32)")?;
    }

    Ok(())
}

pub struct WriteCacheLineManuallyInvalid {}

impl Test for WriteCacheLineManuallyInvalid {
    fn name(&self) -> &str { "cache: Write cache line manually (invalid)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        test_write_cache_line_manually(TagLoPState::Invalid)
    }
}

pub struct WriteCacheLineManuallyClean {}

impl Test for WriteCacheLineManuallyClean {
    fn name(&self) -> &str { "cache: Write cache line manually (clean)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        test_write_cache_line_manually(TagLoPState::Clean)
    }
}

pub struct WriteCacheLineManuallyDirty {}

impl Test for WriteCacheLineManuallyDirty {
    fn name(&self) -> &str { "cache: Write cache line manually (dirty)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        test_write_cache_line_manually(TagLoPState::Dirty)
    }
}

pub struct WriteCacheLineManually01 {}

impl Test for WriteCacheLineManually01 {
    fn name(&self) -> &str { "cache: Write cache line manually (illegal value 0b01)" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        test_write_cache_line_manually(TagLoPState::_Unused01)
    }
}

/// Implements memcpy by moving a cache line exactly +8kb forward
pub struct MoveCacheLine {}

impl Test for MoveCacheLine {
    fn name(&self) -> &str { "cache: Move cache line" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let layout = Layout::from_size_align(16 * 1024, 8 * 1024).unwrap();
        let data = unsafe { alloc(layout) } as *mut u8;

        let taglo_dirty = TagLo::new().with_p_state(TagLoPState::Dirty).with_p_tag_lo(u20::extract_u32(MemoryMap::cached_to_physical_mut(data) as u32 + 8*1024, 12));
        let taglo_invalid = taglo_dirty.with_p_state(TagLoPState::Invalid);

        let moved0: u32;
        let moved1: u32;
        let moved2: u32;
        unsafe {
            const STORE_CACHE_OP: u8 = CacheOp::DataIndexStoreTag.raw_value().value();
            const INVALIDATE_CACHE_OP: u8 = CacheOp::DataIndexWriteBackInvalidate.raw_value().value();
            asm!("
            .set noat
            .set noreorder

            // Clear target
            SD $0, 8 * 1024 + 0 ({uncached})
            SD $0, 8 * 1024 + 8 ({uncached})
            SD $0, 8 * 1024 + 16 ({uncached})
            SD $0, 8 * 1024 + 32 ({uncached})
            SD $0, 8 * 1024 + 48 ({uncached})
            SD $0, 8 * 1024 + 64 ({uncached})

            // Ensure memory is not in cache
            CACHE {INVALIDATE_CACHE_OP}, 0({cached})
            CACHE {INVALIDATE_CACHE_OP}, 16({cached})
            CACHE {INVALIDATE_CACHE_OP}, 32({cached})

            // Write data into memory
            LUI {scratch}, 0x0123
            ORI {scratch}, {scratch}, 0x4567
            SW {scratch}, 0({uncached})
            SW {scratch}, 16({uncached})
            SW {scratch}, 32({uncached})

            // Load the first two cache lines as dirty, third one as clean
            ORI {scratch}, $0, 0x10
            SB {scratch}, 5({cached})
            SB {scratch}, 5 + 16({cached})
            LB {scratch}, 5 + 32({cached})

            // Move second cache line but mark it is as invalid
            MTC0 {taglo_invalid}, ${TAGLO_REG}
            NOP; NOP;
            CACHE {STORE_CACHE_OP}, 16({cached})

            // Mark all three as invalid/dirty
            MTC0 {taglo_dirty}, ${TAGLO_REG}
            NOP; NOP;
            CACHE {STORE_CACHE_OP}, 0({cached})
            CACHE {STORE_CACHE_OP}, 16({cached})
            CACHE {STORE_CACHE_OP}, 32({cached})

            // Load original cache lines again. This should write back the moved one
            LB {scratch}, 4 ({cached})
            LB {scratch}, 4 + 16 ({cached})
            LB {scratch}, 4 + 32 ({cached})

            // Load results from +8k.
            LW {moved0}, 8 * 1024 + 0 ({uncached})
            LW {moved1}, 8 * 1024 + 16 ({uncached})
            LW {moved2}, 8 * 1024 + 32 ({uncached})
            ",
            cached = in(reg) data,
            uncached = in(reg) MemoryMap::uncached_mut(data),

            scratch = out(reg) _,

            moved0 = out(reg) moved0,
            moved1 = out(reg) moved1,
            moved2 = out(reg) moved2,

            taglo_dirty = in(reg) taglo_dirty.raw_value(),
            taglo_invalid = in(reg) taglo_invalid.raw_value(),

            TAGLO_REG = const RegisterIndex::TagLo.raw_value().value(),
            STORE_CACHE_OP = const STORE_CACHE_OP,
            INVALIDATE_CACHE_OP = const INVALIDATE_CACHE_OP,
            )
        }

        unsafe { dealloc(data, layout); }

        soft_assert_eq(moved0, 0x01234567, "When a dirty cache line is moved via Cache(IndexStoreTag), its content moves")?;
        soft_assert_eq(moved1, 0x01234567, "When a dirty cache line is moved via Cache(IndexStoreTag), its content moves (even if it is temporarily marked as invalid)")?;
        soft_assert_eq(moved2, 0x00000000, "When a clean cache line is moved via Cache(IndexStoreTag), the new cache line will still be clean")?;

        Ok(())
    }
}

/// Tests unused bits in cache line
pub struct UnusedBitsInCacheReadWrite {}

impl Test for UnusedBitsInCacheReadWrite {
    fn name(&self) -> &str { "cache: Cache write unused bits" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        let layout = Layout::from_size_align(16 * 1024, 8 * 1024).unwrap();
        let data = unsafe { alloc(layout) } as *mut u8;

        let result0: u32;
        unsafe {
            const STORE_CACHE_OP: u8 = CacheOp::DataIndexStoreTag.raw_value().value();
            const LOAD_CACHE_OP: u8 = CacheOp::DataIndexLoadTag.raw_value().value();
            const INVALIDATE_CACHE_OP: u8 = CacheOp::DataIndexWriteBackInvalidate.raw_value().value();
            asm!("
            .set noat
            .set noreorder

            // Ensure memory is not in cache
            CACHE {INVALIDATE_CACHE_OP}, 0({cached})
            CACHE {INVALIDATE_CACHE_OP}, 16({cached})
            CACHE {INVALIDATE_CACHE_OP}, 32({cached})

            // Write invalid bits into cache line
            MTC0 {taglo_garbage1}, ${TAGLO_REG}
            NOP; NOP;
            CACHE {STORE_CACHE_OP}, 0({cached})
            CACHE {LOAD_CACHE_OP}, 0({cached})
            NOP; NOP
            MFC0 {result0}, ${TAGLO_REG}
            ",
            cached = in(reg) data,

            result0 = out(reg) result0,

            taglo_garbage1 = in(reg) 0xFFFFFFFFu32,

            TAGLO_REG = const RegisterIndex::TagLo.raw_value().value(),
            STORE_CACHE_OP = const STORE_CACHE_OP,
            LOAD_CACHE_OP = const LOAD_CACHE_OP,
            INVALIDATE_CACHE_OP = const INVALIDATE_CACHE_OP,
            )
        }

        unsafe { dealloc(data, layout); }

        soft_assert_eq(result0, 0xfffffc0, "Result when writing way too many bits")?;

        Ok(())
    }
}

fn test_invalidate_keeps_dirty_flag<const INVALIDATE_CACHE_OP: u8>() -> Result<(), String> {
    let layout = Layout::from_size_align(16 * 1024, 8 * 1024).unwrap();
    let data = unsafe { alloc(layout) } as *mut u8;

    let taglo_dirty = TagLo::new().with_p_state(TagLoPState::Dirty).with_p_tag_lo(u20::extract_u32(MemoryMap::cached_to_physical_mut(data) as u32, 12));
    let taglo_invalid = taglo_dirty.with_p_state(TagLoPState::Invalid);

    let result0: u32;
    let result1: u32;
    let result2: u32;
    let result3: u32;
    unsafe {
        const STORE_CACHE_OP: u8 = CacheOp::DataIndexStoreTag.raw_value().value();
        asm!("
            .set noat
            .set noreorder

            // Ensure memory is not in cache
            SW $0, 8 * 1024 ({cached})
            SW $0, 8 * 1024 + 16 ({cached})
            SW $0, 8 * 1024 + 32 ({cached})
            SW $0, 8 * 1024 + 48 ({cached})

            LUI {scratch}, 0x0123
            ORI {scratch}, {scratch}, 0x4567

            // Prepare all four cache states: valid, valid/dirty, invalid, invalid/dirty,
            MTC0 {taglo_invalid}, ${TAGLO_REG}
            LW $0, 0 ({cached})
            SW {scratch}, 16 ({cached})
            LW $0, 32 ({cached})
            SW {scratch}, 48 ({cached})
            CACHE {STORE_CACHE_OP}, 32({cached})
            CACHE {STORE_CACHE_OP}, 48({cached})

            // Invalidate
            CACHE {INVALIDATE_CACHE_OP}, 0({cached})
            CACHE {INVALIDATE_CACHE_OP}, 16({cached})
            CACHE {INVALIDATE_CACHE_OP}, 32({cached})
            CACHE {INVALIDATE_CACHE_OP}, 48({cached})

            // Clear target
            SW $0, 0 ({uncached})
            SW $0, 16 ({uncached})
            SW $0, 32 ({uncached})
            SW $0, 48 ({uncached})

            // Make them manually valid again
            MTC0 {taglo_dirty}, ${TAGLO_REG}
            NOP; NOP;
            CACHE {STORE_CACHE_OP}, 0({cached})
            CACHE {STORE_CACHE_OP}, 16({cached})
            CACHE {STORE_CACHE_OP}, 32({cached})
            CACHE {STORE_CACHE_OP}, 48({cached})

            // Test write back
            LW $0, 8 * 1024 ({cached})
            LW $0, 8 * 1024 + 16 ({cached})
            LW $0, 8 * 1024 + 32 ({cached})
            LW $0, 8 * 1024 + 48 ({cached})

            // And get result
            LW {result0}, 0 ({cached})
            LW {result1}, 16 ({cached})
            LW {result2}, 32 ({cached})
            LW {result3}, 48 ({cached})
            ",
        cached = in(reg) data,
        uncached = in(reg) MemoryMap::uncached_mut(data),

        scratch = out(reg) _,

        result0 = out(reg) result0,
        result1 = out(reg) result1,
        result2 = out(reg) result2,
        result3 = out(reg) result3,

        taglo_dirty = in(reg) taglo_dirty.raw_value(),
        taglo_invalid = in(reg) taglo_invalid.raw_value(),

        TAGLO_REG = const RegisterIndex::TagLo.raw_value().value(),
        STORE_CACHE_OP = const STORE_CACHE_OP,
        INVALIDATE_CACHE_OP = const INVALIDATE_CACHE_OP,
        )
    }

    unsafe { dealloc(data, layout); }

    soft_assert_eq(result0, 0, "Hit Invalidate sets valid to false, but dirty is unaffected (0)")?;
    soft_assert_eq(result1, 0x01234567, "Hit Invalidate sets valid to false, but dirty is unaffected (1)")?;
    soft_assert_eq(result2, 0, "Hit Invalidate sets valid to false, but dirty is unaffected (2)")?;
    soft_assert_eq(result3, 0x01234567, "Hit Invalidate sets valid to false, but dirty is unaffected (3)")?;

    Ok(())

}

/// The dirty flag is preserved while a cache line is invalid
pub struct HitInvalidateKeepsDirtyFlag {}

impl Test for HitInvalidateKeepsDirtyFlag {
    fn name(&self) -> &str { "cache: Hit Invalidate keeps dirty flag" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const OP: u8 = CacheOp::DataHitInvalidate.raw_value().value();
        test_invalidate_keeps_dirty_flag::<OP>()
    }
}

/// The dirty flag is preserved while a cache line is invalid
pub struct HitWriteBackInvalidateKeepsDirtyFlag {}

impl Test for HitWriteBackInvalidateKeepsDirtyFlag {
    fn name(&self) -> &str { "cache: Hit Write Back Invalidate keeps dirty flag" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const OP: u8 = CacheOp::DataHitWriteBackInvalidate.raw_value().value();
        test_invalidate_keeps_dirty_flag::<OP>()
    }
}

/// The dirty flag is preserved while a cache line is invalid
pub struct IndexWriteBackInvalidateKeepsDirtyFlag {}

impl Test for IndexWriteBackInvalidateKeepsDirtyFlag {
    fn name(&self) -> &str { "cache: Index Write Back Invalidate keeps dirty flag" }

    fn level(&self) -> Level { Level::Weird }

    fn values(&self) -> Vec<Box<dyn Any>> { Vec::new() }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const OP: u8 = CacheOp::DataIndexWriteBackInvalidate.raw_value().value();
        test_invalidate_keeps_dirty_flag::<OP>()
    }
}
