use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::any::Any;

use crate::cop0;
use crate::rsp::rsp::RSP;
use crate::rsp::spmem::SPMEM;
use crate::tests::soft_asserts::{soft_assert_eq2, soft_assert_neq};
use crate::tests::{Level, Test};
use crate::MemoryMap;

const SPDMA_MULTIROW_BUF_WORDS: usize = 0x1000;
const SPDMA_MULTIROW_BUF_BYTES: usize = SPDMA_MULTIROW_BUF_WORDS * core::mem::size_of::<u32>();
const SPMEM_HALF_WORDS: usize = 0x1000 >> 2;

// DMA:
// - RDRAM address and SPMEM address are aligned on 8 byte boundaries (the lower 3 bits are ignoed)
// - Length: To the written value 1 is added and then it is rounded up to the next multiple of 8 (e.g. 0..7 ==> 8 bytes, 8..15 => 16 bytes)
// - A DMA goes either into IMEM or DMEM. If it overflows, it will overflow within that memory but never overlap into the other one

fn dma_test_with_source<const N: usize>(
    source_ptr: *const u8,
    spmem_index: u32,
    length: u32,
    expected_start_offset: usize,
    expected_sp_address_after_dma: u32,
    expected: [[u16; 8]; N],
) -> Result<(), String> {
    // Clear SPMEM
    for i in 0..(N * 4) {
        SPMEM::write(expected_start_offset + i * 4, 0xBADDECAF);
    }

    // DMA simple
    RSP::start_dma_cpu_to_sp(source_ptr, spmem_index, length);
    RSP::wait_until_dma_completed();

    // Ensure the data arrived as expected
    for i in 0..N {
        soft_assert_eq2(
            SPMEM::read_vector16_from_dmem_or_imem(expected_start_offset + i * 0x10),
            expected[i],
            || format!("SPMEM[0x{:x}] after DMA", expected_start_offset + i * 0x10),
        )?;
    }

    soft_assert_eq2(RSP::sp_address(), expected_sp_address_after_dma, || {
        "SP-Address after DMA".to_string()
    })?;

    Ok(())
}

fn dma_test<const N: usize>(
    source_index: usize,
    spmem_index: u32,
    length: u32,
    expected_start_offset: usize,
    expected_sp_address_after_dma: u32,
    expected: [[u16; 8]; N],
) -> Result<(), String> {
    // Create some test data. Use uncached memory to ensure the DMA engine can see it
    // without us having to flush any caches first. The buffer must be 8-byte aligned: the SP DMA
    // ignores the low 3 bits of the source address, so a misaligned buffer reads shifted data.
    #[repr(align(8))]
    struct Aligned([[u16; 8]; 4]);
    let mut source_data = Aligned([[0u16; 8]; 4]);
    let source_data_uncached = MemoryMap::uncached_mut(source_data.0.as_mut_ptr());
    unsafe {
        source_data_uncached.add(0).write_volatile([
            0x0123, 0x4567, 0x89AB, 0xCDEF, 0xFEDC, 0x89BA, 0x7654, 0x3210,
        ]);
        source_data_uncached.add(1).write_volatile([
            0x1212, 0x3434, 0x4545, 0x5656, 0x6767, 0x7878, 0x8989, 0x9A9A,
        ]);
        source_data_uncached.add(2).write_volatile([
            0xA11A, 0xB11B, 0xC11C, 0xD11D, 0xE11E, 0xF11F, 0xF00F, 0xE00E,
        ]);
        source_data_uncached.add(3).write_volatile([
            0xD00D, 0xC00C, 0xB00B, 0xA00A, 0x9009, 0x8008, 0x7007, 0x6006,
        ]);
    }

    let source_ptr = unsafe { (source_data_uncached as *mut u8).add(source_index) };
    dma_test_with_source::<N>(
        source_ptr,
        spmem_index,
        length,
        expected_start_offset,
        expected_sp_address_after_dma,
        expected,
    )
}

pub struct SPDMA0_8_7 {}

impl Test for SPDMA0_8_7 {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM -> DMEM (all aligned)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        dma_test(
            0,
            8,
            7,
            0,
            0x10,
            [
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0x0123, 0x4567, 0x89AB, 0xCDEF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
            ],
        )?;
        Ok(())
    }
}

pub struct SPDMA0_12_7D {}

impl Test for SPDMA0_12_7D {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM -> DMEM (SP offset unaligned)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        dma_test(
            0,
            12,
            7,
            0,
            0x10,
            [
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0x0123, 0x4567, 0x89AB, 0xCDEF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
            ],
        )?;
        Ok(())
    }
}

pub struct SPDMA0_12_7I {}

impl Test for SPDMA0_12_7I {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM -> IMEM (SP offset unaligned)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        dma_test(
            0,
            0x100B,
            7,
            0x1000,
            0x1010,
            [
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0x0123, 0x4567, 0x89AB, 0xCDEF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
            ],
        )?;
        Ok(())
    }
}

pub struct SPDMA4_8_7 {}

impl Test for SPDMA4_8_7 {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM -> DMEM (RAM offset unaligned)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        dma_test(
            4,
            8,
            7,
            0,
            0x10,
            [
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0x0123, 0x4567, 0x89AB, 0xCDEF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
            ],
        )?;
        Ok(())
    }
}

pub struct SPDMA0_8_11D {}

impl Test for SPDMA0_8_11D {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM -> DMEM (length = 11)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        dma_test(
            0,
            8,
            11,
            0,
            0x18,
            [
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0x0123, 0x4567, 0x89AB, 0xCDEF,
                ],
                [
                    0xFEDC, 0x89BA, 0x7654, 0x3210, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
            ],
        )?;
        Ok(())
    }
}

pub struct SPDMA0_8_11I {}

impl Test for SPDMA0_8_11I {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM -> IMEM (length = 11)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        dma_test(
            0,
            0x1008,
            11,
            0x1000,
            0x1018,
            [
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0x0123, 0x4567, 0x89AB, 0xCDEF,
                ],
                [
                    0xFEDC, 0x89BA, 0x7654, 0x3210, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
            ],
        )?;
        Ok(())
    }
}

pub struct SPDMAIntoDMEMUntilEnd {}

impl Test for SPDMAIntoDMEMUntilEnd {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM -> DMEM (until the end)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // expected will read from both DMEM and IMEM. Ensure that nothing spilled into IMEM
        dma_test(
            0,
            0xFF0,
            15,
            0xFE0,
            0x0,
            [
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0x0123, 0x4567, 0x89AB, 0xCDEF, 0xFEDC, 0x89BA, 0x7654, 0x3210,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
            ],
        )?;

        Ok(())
    }
}

pub struct SPDMAIntoDMEMWithOverflow {}

impl Test for SPDMAIntoDMEMWithOverflow {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM -> DMEM (overflow)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // expected will read from both DMEM and IMEM. Ensure that nothing spilled into IMEM
        dma_test(
            0,
            0xFF0,
            31,
            0xFE0,
            0x10,
            [
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0x0123, 0x4567, 0x89AB, 0xCDEF, 0xFEDC, 0x89BA, 0x7654, 0x3210,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
            ],
        )?;

        // But also ensure that DMEM properly overflowed
        soft_assert_eq2(
            SPMEM::read_vector16_from_dmem(0),
            [
                0x1212, 0x3434, 0x4545, 0x5656, 0x6767, 0x7878, 0x8989, 0x9A9A,
            ],
            || "SPMEM[0x0] after DMA".to_string(),
        )?;
        Ok(())
    }
}

pub struct SPDMAIntoIMEMUntilEnd {}

impl Test for SPDMAIntoIMEMUntilEnd {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM -> IMEM (until the end)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // expected will read from both DMEM and IMEM. Ensure that nothing spilled into IMEM
        dma_test(
            0,
            0x1FF0,
            15,
            0x1FE0,
            0x1000,
            [
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0x0123, 0x4567, 0x89AB, 0xCDEF, 0xFEDC, 0x89BA, 0x7654, 0x3210,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
            ],
        )?;

        Ok(())
    }
}

pub struct SPDMAIntoIMEMWithOverflow {}

impl Test for SPDMAIntoIMEMWithOverflow {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM -> IMEM (overflow)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        // expected will read from both IMEM and DMEM. Ensure that nothing spilled into DMEM
        dma_test(
            0,
            0x1FF0,
            31,
            0x1FE0,
            0x1010,
            [
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0x0123, 0x4567, 0x89AB, 0xCDEF, 0xFEDC, 0x89BA, 0x7654, 0x3210,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
                [
                    0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF, 0xBADD, 0xECAF,
                ],
            ],
        )?;

        // But also ensure that IMEM properly overflowed
        soft_assert_eq2(
            SPMEM::read_vector16_from_dmem_or_imem(0x1000),
            [
                0x1212, 0x3434, 0x4545, 0x5656, 0x6767, 0x7878, 0x8989, 0x9A9A,
            ],
            || "SPMEM[0x1000] after DMA".to_string(),
        )?;
        Ok(())
    }
}

pub struct SPDMAFromDMEMWithOverflow {}

impl Test for SPDMAFromDMEMWithOverflow {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM <- DMEM (overflow)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        SPMEM::write(0x0000, 0x01234567);
        SPMEM::write(0x0004, 0x89ABCDEF);
        SPMEM::write(0x0FF8, 0xFEDCBA98);
        SPMEM::write(0x0FFC, 0x76543210);

        SPMEM::write(0x1000, 0x11223344);
        SPMEM::write(0x1004, 0x55667788);
        SPMEM::write(0x1FF8, 0x99AABBCC);
        SPMEM::write(0x1FFC, 0xDDEEFF00);

        // 8-byte aligned: the SP DMA ignores the low 3 bits of the target address.
        #[repr(align(8))]
        struct Aligned([u32; 4]);
        let mut source_data = Aligned([0u32; 4]);
        let source_data_uncached = MemoryMap::uncached_mut(&mut source_data.0);
        let source_ptr = source_data_uncached as *mut u8;
        let source_cached = 0xFFFF_FFFF_8000_0000usize | (source_ptr as usize & 0x1FFF_FFFF);
        let rdram_sync = core::mem::size_of_val(&source_data.0);
        unsafe {
            cop0::dcache_hit_writeback_invalidate_range(source_cached, rdram_sync);
            RSP::start_dma_sp_to_cpu(0x0FF8, source_ptr, 16);
            RSP::wait_until_dma_completed();
            cop0::dcache_hit_writeback_invalidate_range(source_cached, rdram_sync);
            soft_assert_eq2(
                *source_data_uncached,
                [0xFEDCBA98, 0x76543210, 0x01234567, 0x89ABCDEF],
                || "RDRAM data after DMA overflow from DMEM".to_string(),
            )?;
        }

        unsafe {
            cop0::dcache_hit_writeback_invalidate_range(source_cached, rdram_sync);
            RSP::start_dma_sp_to_cpu(0x1FF8, source_ptr, 16);
            RSP::wait_until_dma_completed();
            cop0::dcache_hit_writeback_invalidate_range(source_cached, rdram_sync);
            soft_assert_eq2(
                *source_data_uncached,
                [0x99AABBCC, 0xDDEEFF00, 0x11223344, 0x55667788],
                || "RDRAM data after DMA overflow from IMEM".to_string(),
            )?;
        }

        Ok(())
    }
}

/// A DMA sourced from beyond RDRAM: the overflow must stay inside IMEM.
///
/// The data dma'd in is usually 0, but occasionally we see values like 0xB4190010. Unclear where
/// that value comes from, so we only verify that the target was overwritten but not the specific
/// value.
pub struct SPDMAFromNowhereStaysInIMEM {}

impl Test for SPDMAFromNowhereStaysInIMEM {
    fn name(&self) -> &str {
        "spmem: DMA (nowhere) -> IMEM (overflow stays in IMEM)"
    }

    fn level(&self) -> Level {
        Level::BasicFunctionality
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        const FILL: u32 = 0xBADDECAF;

        for i in 0..20 {
            SPMEM::write(0x1FE0 + i * 4, FILL);
        }
        for i in 0..4 {
            SPMEM::write(0x1000 + i * 4, FILL);
        }

        RSP::start_dma_cpu_to_sp(
            MemoryMap::addr32_to_usize(0x80000000 + 12 * 1024 * 1024) as *const u8,
            0x1FF0,
            31,
        );
        RSP::wait_until_dma_completed();

        // Untouched: before the destination, and the wrap past the end of SPMEM into DMEM
        for i in 0..4 {
            soft_assert_eq2(SPMEM::read(0x1FE0 + i * 4), FILL, || {
                format!("SPMEM[0x{:x}] before the destination", 0x1FE0 + i * 4)
            })?;
        }
        for i in 0..12 {
            soft_assert_eq2(SPMEM::read(0x2000 + i * 4), FILL, || {
                format!("SPMEM[0x{:x}] wrapped into DMEM", 0x2000 + i * 4)
            })?;
        }

        // Written, with what is undefined: the destination and the IMEM overflow
        for i in 0..4 {
            soft_assert_neq(
                SPMEM::read(0x1FF0 + i * 4),
                FILL,
                &format!("SPMEM[0x{:x}] written", 0x1FF0 + i * 4),
            )?;
            soft_assert_neq(
                SPMEM::read(0x1000 + i * 4),
                FILL,
                &format!("SPMEM[0x{:x}] IMEM overflow", 0x1000 + i * 4),
            )?;
        }

        soft_assert_eq2(RSP::sp_address(), 0x1010, || {
            "SP-Address after DMA".to_string()
        })?;
        Ok(())
    }
}

const SPDMA_LEN_4_ROWS_0X1000: u32 = (0x1000 - 1) | ((4 - 1) << 12);

fn sp_dma_multirow_alloc() -> Vec<u64> {
    let mut buf = Vec::new();
    buf.resize(SPDMA_MULTIROW_BUF_WORDS / 2, 0u64);
    buf
}

pub struct SPDMAFromDMEMWithOverflowByCount {}

impl Test for SPDMAFromDMEMWithOverflowByCount {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM <- DMEM (overflow with count != 1)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        SPMEM::write(0x0000, 0x01234567);
        SPMEM::write(0x0004, 0x89ABCDEF);
        SPMEM::write(0x0008, 0xFEDCBA98);
        SPMEM::write(0x000C, 0x76543210);
        SPMEM::write(0x1000, 0x11223344);
        SPMEM::write(0x1004, 0x55667788);
        SPMEM::write(0x1008, 0x99AABBCC);
        SPMEM::write(0x100C, 0xDDEEFF00);
        let mut source_data = sp_dma_multirow_alloc();
        let source_data_uncached = MemoryMap::uncached_mut(source_data.as_mut_ptr() as *mut u32);
        let source_ptr = source_data_uncached as *mut u8;
        let source_cached = 0xFFFF_FFFF_8000_0000usize | (source_ptr as usize & 0x1FFF_FFFF);
        let row = [0x01234567u32, 0x89ABCDEF, 0xFEDCBA98, 0x76543210];
        unsafe {
            cop0::dcache_hit_writeback_invalidate_range(source_cached, SPDMA_MULTIROW_BUF_BYTES);
            RSP::start_dma_sp_to_cpu(0x0000, source_ptr, SPDMA_LEN_4_ROWS_0X1000);
            RSP::wait_until_dma_completed();
            cop0::dcache_hit_writeback_invalidate_range(source_cached, SPDMA_MULTIROW_BUF_BYTES);
            let sl = core::slice::from_raw_parts(source_data_uncached, SPDMA_MULTIROW_BUF_WORDS);
            for k in 0..4 {
                let o = k * (0x1000 >> 2);
                soft_assert_eq2(&sl[o..o + 4], &row[..], || {
                    format!("RDRAM row {} after DMA overflow from DMEM (count != 1)", k)
                })?;
            }
        }
        Ok(())
    }
}

pub struct SPDMAFromIMEMWithOverflowByCount {}

impl Test for SPDMAFromIMEMWithOverflowByCount {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM <- IMEM (overflow with count != 1)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        SPMEM::write(0x0000, 0x01234567);
        SPMEM::write(0x0004, 0x89ABCDEF);
        SPMEM::write(0x0008, 0xFEDCBA98);
        SPMEM::write(0x000C, 0x76543210);
        SPMEM::write(0x1000, 0x11223344);
        SPMEM::write(0x1004, 0x55667788);
        SPMEM::write(0x1008, 0x99AABBCC);
        SPMEM::write(0x100C, 0xDDEEFF00);
        let mut source_data = sp_dma_multirow_alloc();
        let source_data_uncached = MemoryMap::uncached_mut(source_data.as_mut_ptr() as *mut u32);
        let source_ptr = source_data_uncached as *mut u8;
        let source_cached = 0xFFFF_FFFF_8000_0000usize | (source_ptr as usize & 0x1FFF_FFFF);
        let row = [0x11223344u32, 0x55667788, 0x99AABBCC, 0xDDEEFF00];
        unsafe {
            cop0::dcache_hit_writeback_invalidate_range(source_cached, SPDMA_MULTIROW_BUF_BYTES);
            RSP::start_dma_sp_to_cpu(0x1000, source_ptr, SPDMA_LEN_4_ROWS_0X1000);
            RSP::wait_until_dma_completed();
            cop0::dcache_hit_writeback_invalidate_range(source_cached, SPDMA_MULTIROW_BUF_BYTES);
            let sl = core::slice::from_raw_parts(source_data_uncached, SPDMA_MULTIROW_BUF_WORDS);
            for k in 0..4 {
                let o = k * (0x1000 >> 2);
                soft_assert_eq2(&sl[o..o + 4], &row[..], || {
                    format!("RDRAM row {} after DMA overflow from IMEM (count != 1)", k)
                })?;
            }
        }
        Ok(())
    }
}

pub struct SPDMAMultiRowDMEMRoundtrip {}

impl Test for SPDMAMultiRowDMEMRoundtrip {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM <-> DMEM (multi-row overflow roundtrip)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        SPMEM::write(0x0000, 0x01234567);
        SPMEM::write(0x0004, 0x89ABCDEF);
        SPMEM::write(0x0008, 0xFEDCBA98);
        SPMEM::write(0x000C, 0x76543210);
        SPMEM::write(0x1000, 0xEEEEFFFF);
        SPMEM::write(0x1004, 0x00001111);
        let mut source_data = sp_dma_multirow_alloc();
        let source_data = unsafe {
            core::slice::from_raw_parts_mut(
                source_data.as_mut_ptr() as *mut u32,
                SPDMA_MULTIROW_BUF_WORDS,
            )
        };
        source_data.fill(0xBAD0BAD0);
        let source_data_uncached = MemoryMap::uncached_mut(source_data.as_mut_ptr());
        let source_ptr = source_data_uncached as *mut u8;
        let source_cached = 0xFFFF_FFFF_8000_0000usize | (source_ptr as usize & 0x1FFF_FFFF);
        unsafe {
            cop0::dcache_hit_writeback_invalidate_range(source_cached, SPDMA_MULTIROW_BUF_BYTES);
            RSP::start_dma_sp_to_cpu(0x0000, source_ptr, SPDMA_LEN_4_ROWS_0X1000);
            RSP::wait_until_dma_completed();
            cop0::dcache_hit_writeback_invalidate_range(source_cached, SPDMA_MULTIROW_BUF_BYTES);
        }
        for i in 0..SPMEM_HALF_WORDS {
            SPMEM::write(i << 2, 0x0BADF00D);
        }
        unsafe {
            cop0::dcache_hit_writeback_invalidate_range(source_cached, SPDMA_MULTIROW_BUF_BYTES)
        };
        RSP::start_dma_cpu_to_sp(source_ptr, 0x0000, SPDMA_LEN_4_ROWS_0X1000);
        RSP::wait_until_dma_completed();
        soft_assert_eq2(SPMEM::read(0x0000), 0x01234567, || {
            "DMEM[0x0000] after roundtrip".to_string()
        })?;
        soft_assert_eq2(SPMEM::read(0x0004), 0x89ABCDEF, || {
            "DMEM[0x0004] after roundtrip".to_string()
        })?;
        soft_assert_eq2(SPMEM::read(0x0008), 0xFEDCBA98, || {
            "DMEM[0x0008] after roundtrip".to_string()
        })?;
        soft_assert_eq2(SPMEM::read(0x000C), 0x76543210, || {
            "DMEM[0x000C] after roundtrip".to_string()
        })?;
        soft_assert_eq2(SPMEM::read(0x1000), 0xEEEEFFFF, || {
            "IMEM[0x1000] after roundtrip (unchanged)".to_string()
        })?;
        soft_assert_eq2(SPMEM::read(0x1004), 0x00001111, || {
            "IMEM[0x1004] after roundtrip (unchanged)".to_string()
        })?;
        Ok(())
    }
}

pub struct SPDMAMultiRowIMEMRoundtrip {}

impl Test for SPDMAMultiRowIMEMRoundtrip {
    fn name(&self) -> &str {
        "spmem: DMA RDRAM <-> IMEM (multi-row overflow roundtrip)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Vec::new()
    }

    fn run(&self, _value: &Box<dyn Any>) -> Result<(), String> {
        SPMEM::write(0x0000, 0xDEADBEEF);
        SPMEM::write(0x0004, 0xCAFEBABE);
        SPMEM::write(0x1000, 0x11223344);
        SPMEM::write(0x1004, 0x55667788);
        SPMEM::write(0x1008, 0x99AABBCC);
        SPMEM::write(0x100C, 0xDDEEFF00);
        let mut source_data = sp_dma_multirow_alloc();
        let source_data = unsafe {
            core::slice::from_raw_parts_mut(
                source_data.as_mut_ptr() as *mut u32,
                SPDMA_MULTIROW_BUF_WORDS,
            )
        };
        source_data.fill(0xBAD0BAD0);
        let source_data_uncached = MemoryMap::uncached_mut(source_data.as_mut_ptr());
        let source_ptr = source_data_uncached as *mut u8;
        let source_cached = 0xFFFF_FFFF_8000_0000usize | (source_ptr as usize & 0x1FFF_FFFF);
        unsafe {
            cop0::dcache_hit_writeback_invalidate_range(source_cached, SPDMA_MULTIROW_BUF_BYTES);
            RSP::start_dma_sp_to_cpu(0x1000, source_ptr, SPDMA_LEN_4_ROWS_0X1000);
            RSP::wait_until_dma_completed();
            cop0::dcache_hit_writeback_invalidate_range(source_cached, SPDMA_MULTIROW_BUF_BYTES);
        }
        for i in 0..SPMEM_HALF_WORDS {
            SPMEM::write((i << 2) | 0x1000, 0x0BADF00D);
        }
        unsafe {
            cop0::dcache_hit_writeback_invalidate_range(source_cached, SPDMA_MULTIROW_BUF_BYTES)
        };
        RSP::start_dma_cpu_to_sp(source_ptr, 0x1000, SPDMA_LEN_4_ROWS_0X1000);
        RSP::wait_until_dma_completed();
        soft_assert_eq2(SPMEM::read(0x0000), 0xDEADBEEF, || {
            "DMEM[0x0000] after IMEM roundtrip (unchanged)".to_string()
        })?;
        soft_assert_eq2(SPMEM::read(0x0004), 0xCAFEBABE, || {
            "DMEM[0x0004] after IMEM roundtrip (unchanged)".to_string()
        })?;
        soft_assert_eq2(SPMEM::read(0x1000), 0x11223344, || {
            "IMEM[0x1000] after roundtrip".to_string()
        })?;
        soft_assert_eq2(SPMEM::read(0x1004), 0x55667788, || {
            "IMEM[0x1004] after roundtrip".to_string()
        })?;
        soft_assert_eq2(SPMEM::read(0x1008), 0x99AABBCC, || {
            "IMEM[0x1008] after roundtrip".to_string()
        })?;
        soft_assert_eq2(SPMEM::read(0x100C), 0xDDEEFF00, || {
            "IMEM[0x100C] after roundtrip".to_string()
        })?;
        Ok(())
    }
}
