//! RDP pipeline state. State is kept as the raw command words that set it; accessors decode the
//! individual fields. This keeps the bit layout - the actual object of the reverse engineering -
//! in one visible place per register.

/// Cycle type (SetOtherModes bits 53..=52).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CycleType {
    OneCycle,
    TwoCycle,
    Copy,
    Fill,
}

#[derive(Default)]
pub struct State {
    /// SetOtherModes command word.
    pub other_modes: u64,
    /// SetScissor command word.
    pub scissor: u64,
    /// SetFillColor: 32-bit color, or two packed 16-bit pixels for 16bpp framebuffers.
    pub fill_color: u32,
    /// SetBlendColor: r bits 31..=24, g 23..=16, b 15..=8, a 7..=0.
    pub blend_color: u32,
    /// SetColorImage command word.
    pub color_image: u64,
}

impl State {
    pub fn cycle_type(&self) -> CycleType {
        match (self.other_modes >> 52) & 3 {
            0 => CycleType::OneCycle,
            1 => CycleType::TwoCycle,
            2 => CycleType::Copy,
            _ => CycleType::Fill,
        }
    }

    /// Scissor bounds as raw 10.2 fixed point: (left, top, right, bottom).
    pub fn scissor_bounds(&self) -> (u32, u32, u32, u32) {
        let left = ((self.scissor >> 44) & 0xFFF) as u32;
        let top = ((self.scissor >> 32) & 0xFFF) as u32;
        let right = ((self.scissor >> 12) & 0xFFF) as u32;
        let bottom = (self.scissor & 0xFFF) as u32;
        (left, top, right, bottom)
    }

    /// Physical RDRAM address of the color image.
    pub fn color_image_addr(&self) -> u32 {
        (self.color_image & 0x03FF_FFFF) as u32
    }

    /// log2(bits per pixel) - 2: 0 = 4bpp, 1 = 8bpp, 2 = 16bpp, 3 = 32bpp.
    pub fn color_image_size(&self) -> u32 {
        ((self.color_image >> 51) & 3) as u32
    }

    /// Width field of SetColorImage (stored width - 1 by convention of the callers). Provisional:
    /// documented as 10 bits; whether the hardware samples more of bits 43..=32 is untested.
    pub fn color_image_width(&self) -> u32 {
        ((self.color_image >> 32) & 0x3FF) as u32
    }
}
