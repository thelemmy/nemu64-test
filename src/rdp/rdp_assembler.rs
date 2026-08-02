use core::fmt::{Debug, Formatter};

use arbitrary_int::prelude::*;

use crate::graphics::color::{ARGB8888, RGBA5551};
use crate::math::bits::{Bitmasks32, Bitmasks64};
use crate::rdp::fixedpoint::{I12_2, I16_16, U10_2};
use crate::rdp::modes::{Format, Othermode, PixelSize};
use crate::uncached_memory::UncachedHeapMemory;

#[allow(dead_code)]
#[repr(u8)]
#[derive(Debug)]
enum RDPCommand {
    FilledTriangle = 8,
    DepthFilledTriangle = 9,
    TexturedTriangle = 10,
    TexturedDepthTriangle = 11,
    ShadedTriangle = 12,
    ShadedDepthTriangle = 13,
    ShadedTexturedTriangle = 14,
    ShadedTexturedDepthTriangle = 15,
    TexturedRectangle = 36,
    FlippedTexturedRectangle = 37,
    SyncLoad = 38,
    SyncPipe = 39,
    TileSync = 40,
    SyncFull = 41,
    SetKeyColorGreenBlue = 42,
    SetKeyColorRed = 43,
    SetConvert = 44,
    SetScissor = 45,
    SetPrimitiveDepth = 46,
    SetOtherMode = 47,
    LoadPalette = 48,
    SetTileSize = 50,
    LoadBlock = 51,
    LoadTile = 52,
    SetTile = 53,
    FilledRectangle = 54,
    SetFillColor = 55,
    SetFogColor = 56,
    SetBlendColor = 57,
    SetPrimitiveColor = 58,
    SetEnvColor = 59,
    SetCombine = 60,
    SetTextureImage = 61,
    SetDepthImage = 62,
    SetFramebufferImage = 63,
}

const INSTRUCTION_STREAM_SIZE: usize = 128;

#[derive(Debug)]
pub struct RDPRectangle {
    left: U10_2,
    top: U10_2,
    right: U10_2,
    bottom: U10_2,
}

#[allow(dead_code)] // accessors round out the type; not all are exercised
impl RDPRectangle {
    pub const fn new(left: U10_2, top: U10_2, right: U10_2, bottom: U10_2) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn left(&self) -> U10_2 {
        self.left
    }
    pub const fn top(&self) -> U10_2 {
        self.top
    }
    pub const fn right(&self) -> U10_2 {
        self.right
    }
    pub const fn bottom(&self) -> U10_2 {
        self.bottom
    }
}

pub struct TriangleBase {
    data: [u64; 4],
}

impl TriangleBase {
    const fn first(
        is_right_major: bool,
        mip: u32,
        tile: u32,
        yh: I12_2,
        ym: I12_2,
        yl: I12_2,
    ) -> u64 {
        assert!(mip <= Bitmasks32::M4);
        assert!(tile <= Bitmasks32::M3);
        let value = ((is_right_major as u64) << 55)
            | ((mip as u64) << 51)
            | ((tile as u64) << 48)
            | ((yl.masked_value() as u64) << 32)
            | ((ym.masked_value() as u64) << 16)
            | ((yh.masked_value() as u64) << 0);

        value
    }

    pub const fn new(
        is_right_major: bool,
        mip: u32,
        tile: u32,
        yl: I12_2,
        ym: I12_2,
        yh: I12_2,
        xl: I16_16,
        xm: I16_16,
        xh: I16_16,
        dl: I16_16,
        dm: I16_16,
        dh: I16_16,
    ) -> Self {
        Self {
            data: [
                Self::first(is_right_major, mip, tile, yh, ym, yl),
                ((xl.masked_value() as u64) << 32) | (dl.masked_value() as u64),
                ((xh.masked_value() as u64) << 32) | (dh.masked_value() as u64),
                ((xm.masked_value() as u64) << 32) | (dm.masked_value() as u64),
            ],
        }
    }

    pub const fn is_right_major(&self) -> bool {
        ((self.data[0] >> 55) & 1) != 0
    }

    pub const fn mip(&self) -> u32 {
        ((self.data[0] >> 51) as u32) & Bitmasks32::M4
    }

    pub const fn tile(&self) -> u32 {
        ((self.data[0] >> 48) as u32) & Bitmasks32::M3
    }

    pub const fn yl(&self) -> I12_2 {
        I12_2::new_with_masked_value(((self.data[0] >> 32) as u32) & Bitmasks32::M14)
    }

    pub const fn ym(&self) -> I12_2 {
        I12_2::new_with_masked_value(((self.data[0] >> 16) as u32) & Bitmasks32::M14)
    }

    pub const fn yh(&self) -> I12_2 {
        I12_2::new_with_masked_value(((self.data[0] >> 0) as u32) & Bitmasks32::M14)
    }

    pub const fn xl(&self) -> I16_16 {
        I16_16::new_with_masked_value((self.data[1] >> 32) as u32)
    }

    pub const fn xm(&self) -> I16_16 {
        I16_16::new_with_masked_value((self.data[3] >> 32) as u32)
    }

    pub const fn xh(&self) -> I16_16 {
        I16_16::new_with_masked_value((self.data[2] >> 32) as u32)
    }

    pub const fn dl(&self) -> I16_16 {
        I16_16::new_with_masked_value((self.data[1] >> 0) as u32)
    }

    pub const fn dm(&self) -> I16_16 {
        I16_16::new_with_masked_value((self.data[3] >> 0) as u32)
    }

    pub const fn dh(&self) -> I16_16 {
        I16_16::new_with_masked_value((self.data[2] >> 0) as u32)
    }
}

impl Debug for TriangleBase {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TriangleBase")
            .field("is_right_major", &self.is_right_major())
            .field("mip", &self.mip())
            .field("tile", &self.tile())
            .field("yl", &self.yl())
            .field("ym", &self.ym())
            .field("yh", &self.yh())
            .field("xl", &self.xl())
            .field("xm", &self.xm())
            .field("xh", &self.xh())
            .field("dl", &self.dl())
            .field("dm", &self.dm())
            .field("dh", &self.dh())
            .finish()
    }
}

pub struct RDPAssembler {
    data: UncachedHeapMemory<u64>,
    index: usize,
}

#[allow(dead_code)] // texture/TMEM commands land ahead of the tests that exercise them
impl<'a> RDPAssembler {
    pub fn new() -> Self {
        Self {
            data: UncachedHeapMemory::new(INSTRUCTION_STREAM_SIZE),
            index: 0,
        }
    }

    pub fn start(&mut self) -> usize {
        self.data.start_physical()
    }

    pub fn end(&mut self) -> usize {
        self.data.start_physical() + (self.index << 3)
    }

    /// Number of command words written so far
    pub fn len(&self) -> usize {
        self.index
    }

    /// Reads back a previously written command word (e.g. to feed the identical stream into the
    /// soft-RDP)
    pub fn word(&mut self, index: usize) -> u64 {
        assert!(index < self.index);
        self.data.read(index)
    }

    fn write(&mut self, value: u64) {
        self.data.write(self.index, value);
        self.index += 1;
    }

    fn write_command(&mut self, command: RDPCommand, value: u64) {
        assert!((value >> 56) == 0);

        self.write(((command as u64) << 56) | value);
    }

    pub fn set_scissor(&mut self, value: &RDPRectangle) {
        self.write_command(
            RDPCommand::SetScissor,
            ((value.left.masked_value() as u64) << 44)
                | ((value.top.masked_value() as u64) << 32)
                | ((value.right.masked_value() as u64) << 12)
                | ((value.bottom.masked_value() as u64) << 0),
        );
    }

    pub fn sync_full(&mut self) {
        self.write_command(RDPCommand::SyncFull, 0);
    }

    pub fn sync_pipe(&mut self) {
        self.write_command(RDPCommand::SyncPipe, 0);
    }

    pub fn set_blendcolor(&mut self, color: ARGB8888) {
        self.write_command(RDPCommand::SetBlendColor, color.raw_value() as u64);
    }

    pub fn set_fillcolor32(&mut self, color: ARGB8888) {
        self.write_command(RDPCommand::SetFillColor, color.raw_value() as u64);
    }

    pub fn set_fillcolor16(&mut self, color1: RGBA5551, color2: RGBA5551) {
        self.write_command(
            RDPCommand::SetFillColor,
            ((color1.raw_value() as u32) | ((color2.raw_value() as u32) << 16)) as u64,
        );
    }

    pub fn set_othermode(&mut self, othermode: Othermode) {
        self.write_command(RDPCommand::SetOtherMode, othermode.raw_value());
    }

    pub fn set_framebuffer_image<T: Copy + Clone>(
        &mut self,
        format: Format,
        pixel_size: PixelSize,
        width: u12,
        memory: &'a mut UncachedHeapMemory<T>,
    ) {
        let value = ((memory.start_physical() as u64) & Bitmasks64::M26)
            | ((width.value() as u64) << 32)
            | ((pixel_size as u64) << 51)
            | ((format as u64) << 53);

        self.write_command(RDPCommand::SetFramebufferImage, value);
    }

    pub fn filled_triangle(&mut self, base: &TriangleBase) {
        self.write_command(RDPCommand::FilledTriangle, base.data[0]);

        for i in 1..base.data.len() {
            self.write(base.data[i]);
        }
    }

    /// Shaded triangle: the edge words followed by the 8 shade coefficient words
    pub fn shaded_triangle(&mut self, base: &TriangleBase, shade: &[u64; 8]) {
        self.write_command(RDPCommand::ShadedTriangle, base.data[0]);

        for i in 1..base.data.len() {
            self.write(base.data[i]);
        }
        for word in shade {
            self.write(*word);
        }
    }

    /// SetCombine from the raw 56-bit mux configuration
    pub fn set_combine_raw(&mut self, value: u64) {
        self.write_command(RDPCommand::SetCombine, value);
    }

    pub fn set_texture_image<T: Copy + Clone>(
        &mut self,
        format: Format,
        pixel_size: PixelSize,
        width_minus_one: u12,
        memory: &'a mut UncachedHeapMemory<T>,
    ) {
        let value = ((memory.start_physical() as u64) & Bitmasks64::M26)
            | ((width_minus_one.value() as u64) << 32)
            | ((pixel_size as u64) << 51)
            | ((format as u64) << 53);
        self.write_command(RDPCommand::SetTextureImage, value);
    }

    /// SetTile. `line` is the tile's row stride in 64-bit words, `tmem` its start in 64-bit words.
    #[allow(clippy::too_many_arguments)]
    pub fn set_tile(
        &mut self,
        format: Format,
        pixel_size: PixelSize,
        line: u16,
        tmem: u16,
        tile: u8,
        palette: u8,
        clamp_mirror_t: (bool, bool),
        mask_shift_t: (u8, u8),
        clamp_mirror_s: (bool, bool),
        mask_shift_s: (u8, u8),
    ) {
        let value = ((format as u64) << 53)
            | ((pixel_size as u64) << 51)
            | ((line as u64 & 0x1FF) << 41)
            | ((tmem as u64 & 0x1FF) << 32)
            | ((tile as u64 & 7) << 24)
            | ((palette as u64 & 0xF) << 20)
            | ((clamp_mirror_t.0 as u64) << 19)
            | ((clamp_mirror_t.1 as u64) << 18)
            | ((mask_shift_t.0 as u64 & 0xF) << 14)
            | ((mask_shift_t.1 as u64 & 0xF) << 10)
            | ((clamp_mirror_s.0 as u64) << 9)
            | ((clamp_mirror_s.1 as u64) << 8)
            | ((mask_shift_s.0 as u64 & 0xF) << 4)
            | (mask_shift_s.1 as u64 & 0xF);
        self.write_command(RDPCommand::SetTile, value);
    }

    /// Shared layout of SetTileSize / LoadTile / LoadTlut: two 10.2 corners plus a tile index
    fn tile_rect_command(
        &mut self,
        command: RDPCommand,
        tile: u8,
        sl: u16,
        tl: u16,
        sh: u16,
        th: u16,
    ) {
        let value = ((sl as u64 & 0xFFF) << 44)
            | ((tl as u64 & 0xFFF) << 32)
            | ((tile as u64 & 7) << 24)
            | ((sh as u64 & 0xFFF) << 12)
            | (th as u64 & 0xFFF);
        self.write_command(command, value);
    }

    pub fn set_tile_size(&mut self, tile: u8, sl: u16, tl: u16, sh: u16, th: u16) {
        self.tile_rect_command(RDPCommand::SetTileSize, tile, sl, tl, sh, th);
    }

    pub fn load_tile(&mut self, tile: u8, sl: u16, tl: u16, sh: u16, th: u16) {
        self.tile_rect_command(RDPCommand::LoadTile, tile, sl, tl, sh, th);
    }

    pub fn load_tlut(&mut self, tile: u8, sl: u16, tl: u16, sh: u16, th: u16) {
        self.tile_rect_command(RDPCommand::LoadPalette, tile, sl, tl, sh, th);
    }

    /// LoadBlock: `texels` is the count minus one, `dxt` the per-line T step
    pub fn load_block(&mut self, tile: u8, sl: u16, tl: u16, texels: u16, dxt: u16) {
        self.tile_rect_command(RDPCommand::LoadBlock, tile, sl, tl, texels, dxt);
    }

    pub fn sync_load(&mut self) {
        self.write_command(RDPCommand::SyncLoad, 0);
    }

    pub fn sync_tile(&mut self) {
        self.write_command(RDPCommand::TileSync, 0);
    }

    /// TextureRectangle: screen rect in 10.2, texture start in 10.5 and steps in 5.10
    #[allow(clippy::too_many_arguments)]
    pub fn texture_rectangle(
        &mut self,
        tile: u8,
        rect: &RDPRectangle,
        s: u16,
        t: u16,
        dsdx: u16,
        dtdy: u16,
    ) {
        self.write_command(
            RDPCommand::TexturedRectangle,
            ((rect.right.masked_value() as u64) << 44)
                | ((rect.bottom.masked_value() as u64) << 32)
                | ((tile as u64 & 7) << 24)
                | ((rect.left.masked_value() as u64) << 12)
                | (rect.top.masked_value() as u64),
        );
        self.write(((s as u64) << 48) | ((t as u64) << 32) | ((dsdx as u64) << 16) | (dtdy as u64));
    }

    pub fn filled_rectangle(&mut self, value: &RDPRectangle) {
        self.write_command(
            RDPCommand::FilledRectangle,
            ((value.right.masked_value() as u64) << 44)
                | ((value.bottom.masked_value() as u64) << 32)
                | ((value.left.masked_value() as u64) << 12)
                | ((value.top.masked_value() as u64) << 0),
        );
    }
}
