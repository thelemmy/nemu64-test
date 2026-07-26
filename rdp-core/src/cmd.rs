//! Command stream decoding. A command is 1..=22 64-bit words; the opcode lives in bits 61..=56 of
//! the first word.

/// Opcodes. Names follow the common community naming; the numbers are what the hardware decodes.
pub mod op {
    pub const NO_OP: u8 = 0x00;
    pub const FILL_TRIANGLE: u8 = 0x08;
    pub const FILL_Z_TRIANGLE: u8 = 0x09;
    pub const TEXTURE_TRIANGLE: u8 = 0x0A;
    pub const TEXTURE_Z_TRIANGLE: u8 = 0x0B;
    pub const SHADE_TRIANGLE: u8 = 0x0C;
    pub const SHADE_Z_TRIANGLE: u8 = 0x0D;
    pub const SHADE_TEXTURE_TRIANGLE: u8 = 0x0E;
    pub const SHADE_TEXTURE_Z_TRIANGLE: u8 = 0x0F;
    pub const TEXTURE_RECTANGLE: u8 = 0x24;
    pub const TEXTURE_RECTANGLE_FLIP: u8 = 0x25;
    pub const SYNC_LOAD: u8 = 0x26;
    pub const SYNC_PIPE: u8 = 0x27;
    pub const SYNC_TILE: u8 = 0x28;
    pub const SYNC_FULL: u8 = 0x29;
    pub const SET_KEY_GB: u8 = 0x2A;
    pub const SET_KEY_R: u8 = 0x2B;
    pub const SET_CONVERT: u8 = 0x2C;
    pub const SET_SCISSOR: u8 = 0x2D;
    pub const SET_PRIM_DEPTH: u8 = 0x2E;
    pub const SET_OTHER_MODES: u8 = 0x2F;
    pub const LOAD_TLUT: u8 = 0x30;
    pub const SET_TILE_SIZE: u8 = 0x32;
    pub const LOAD_BLOCK: u8 = 0x33;
    pub const LOAD_TILE: u8 = 0x34;
    pub const SET_TILE: u8 = 0x35;
    pub const FILL_RECTANGLE: u8 = 0x36;
    pub const SET_FILL_COLOR: u8 = 0x37;
    pub const SET_FOG_COLOR: u8 = 0x38;
    pub const SET_BLEND_COLOR: u8 = 0x39;
    pub const SET_PRIM_COLOR: u8 = 0x3A;
    pub const SET_ENV_COLOR: u8 = 0x3B;
    pub const SET_COMBINE: u8 = 0x3C;
    pub const SET_TEXTURE_IMAGE: u8 = 0x3D;
    pub const SET_MASK_IMAGE: u8 = 0x3E;
    pub const SET_COLOR_IMAGE: u8 = 0x3F;
}

pub const fn opcode(word: u64) -> u8 {
    ((word >> 56) & 0x3F) as u8
}

/// Total length of a command in 64-bit words, including the first one. Triangles carry
/// 4 edge words, plus 8 shade, plus 8 texture, plus 2 z coefficient words.
pub const fn command_words(opcode: u8) -> usize {
    match opcode {
        op::FILL_TRIANGLE => 4,
        op::FILL_Z_TRIANGLE => 4 + 2,
        op::TEXTURE_TRIANGLE => 4 + 8,
        op::TEXTURE_Z_TRIANGLE => 4 + 8 + 2,
        op::SHADE_TRIANGLE => 4 + 8,
        op::SHADE_Z_TRIANGLE => 4 + 8 + 2,
        op::SHADE_TEXTURE_TRIANGLE => 4 + 8 + 8,
        op::SHADE_TEXTURE_Z_TRIANGLE => 4 + 8 + 8 + 2,
        op::TEXTURE_RECTANGLE | op::TEXTURE_RECTANGLE_FLIP => 2,
        _ => 1,
    }
}
