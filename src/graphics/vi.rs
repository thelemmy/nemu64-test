use arbitrary_int::{u10, u12, u24, u4};
use bitbybit::{bitenum, bitfield};
use crate::graphics::framebuffer_images::FramebufferImages;

// Supported: RGBA1555
pub type PixelType = crate::graphics::color::RGBA5551;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;

const FRAMEBUFFER_ALIGNMENT: usize = 1 * 1024 * 1024;

#[bitenum(u2)]
enum StatusFramebufferType {
    Off = 0,
    Bits16 = 2,
    Bits32 = 3,
}

#[bitfield(u32, default: 0)]
struct Status {
    #[bits(0..=1, rw)]
    framebuffer_type: Option<StatusFramebufferType>,

    #[bit(2, rw)]
    gamma_dither: bool,

    #[bit(3, rw)]
    gamma_boost: bool,

    #[bit(6, rw)]
    serrate: bool,

    #[bit(8, rw)]
    anti_alias: bool,

    #[bit(9, rw)]
    resample: bool,

    #[bits(12..=15, rw)]
    pixel_advance: u4,
}

#[bitfield(u32, default: 0)]
struct VIIntr {
    /// When this equals the VI's current row, an interrupt is caused
    #[bits(0..=9, rw)]
    row: u10,
}

#[bitfield(u32, default: 0)]
struct Scale {
    #[bits(0..=11, rw)]
    scale_2_10: u12,

    #[bits(16..=27, rw)]
    subpixel_offset_2_10: u12,
}

#[bitfield(u32, default: 0)]
struct DRAMAddress {
    #[bits(0..=23, rw)]
    address: u24,
}


const VI_BASE_REG: *mut u32 = 0xA440_0000 as *mut u32;

pub struct Video {
    framebuffers: FramebufferImages<PixelType>,
}

#[allow(dead_code)]
enum RegisterOffset {
    Status = 0x00,
    DRAMAddress = 0x04,
    HWidth = 0x08,
    VIntr = 0x0C,
    Current = 0x10,
    Timing = 0x14,
    VSync = 0x18,
    HSync = 0x1C,
    HSyncLeap = 0x20,
    HVideo = 0x24,
    VVideo = 0x28,
    VBurst = 0x2C,
    XScale = 0x30,
    YScale = 0x34,
}

impl Video {
    pub const fn new() -> Self {
        Self {
            framebuffers: FramebufferImages::new(),
        }
    }

    pub fn init(&self) {
        // Initialize VI. See https://github.com/PeterLemon/N64/blob/master/RDP/TextureCoordinates/LIB/N64_GFX.INC#L38 for an assembly version of this

        unsafe {
            VI_BASE_REG.add(RegisterOffset::Status as usize >> 2).write_volatile(
                Status::new()
                    .with_framebuffer_type(StatusFramebufferType::Bits16)
                    .with_gamma_dither(true)
                    .with_gamma_boost(true)
                    .with_serrate(true)
                    .with_anti_alias(false)
                    .with_resample(true)
                    .with_pixel_advance(u4::new(3))
                    .raw_value(), );
            VI_BASE_REG.add(RegisterOffset::VIntr as usize >> 2).write_volatile(
                VIIntr::new().with_row(u10::new(2)).raw_value());
            VI_BASE_REG.add(RegisterOffset::Timing as usize >> 2).write_volatile(0x03E5_2239);
            VI_BASE_REG.add(RegisterOffset::VSync as usize >> 2).write_volatile(0x0000_020D);
            VI_BASE_REG.add(RegisterOffset::HSync as usize >> 2).write_volatile(0x0000_0C15);
            VI_BASE_REG.add(RegisterOffset::HSyncLeap as usize >> 2).write_volatile(0x0C15_0C15);
            VI_BASE_REG.add(RegisterOffset::HVideo as usize >> 2).write_volatile(0x006C_02EC);
            VI_BASE_REG.add(RegisterOffset::VVideo as usize >> 2).write_volatile(0x0025_01FF);
            VI_BASE_REG.add(RegisterOffset::VBurst as usize >> 2).write_volatile(0x000E_0204);
            VI_BASE_REG.add(RegisterOffset::XScale as usize >> 2).write_volatile((0x100 * WIDTH) / 160);
            VI_BASE_REG.add(RegisterOffset::YScale as usize >> 2).write_volatile((0x100 * HEIGHT) / 60);
        }
    }

    fn set_framebuffer_type(&self, value: StatusFramebufferType) -> StatusFramebufferType {
        unsafe {
            let mmr = VI_BASE_REG.add(RegisterOffset::Status as usize >> 2);
            let status = Status::new_with_raw_value(mmr.read_volatile());
            let new_status = status.with_framebuffer_type(value);
            mmr.write_volatile(new_status.raw_value());
            status.framebuffer_type().unwrap_or(StatusFramebufferType::Off)
        }
    }

    pub fn framebuffers(&self) -> &FramebufferImages<PixelType> { &self.framebuffers }

    pub fn alloc_framebuffer(&self) {
        self.framebuffers.alloc_buffers(FRAMEBUFFER_ALIGNMENT, WIDTH, HEIGHT);
        self.activate_frontbuffer();
        unsafe { VI_BASE_REG.add(RegisterOffset::HWidth as usize >> 2).write_volatile(WIDTH); }
    }

    pub fn swap_buffers(&self) {
        self.framebuffers.swap_buffers();
        self.activate_frontbuffer();
    }

    pub fn frontbuffer_physical_address(&self) -> u32 {
        let mut frontbuffer_lock = self.framebuffers.frontbuffer().lock();
        if let Some(frontbuffer) = frontbuffer_lock.as_mut() {
            let pixels = frontbuffer.pixels_mut();
            let ptr = pixels.as_ptr();
            (ptr as u32) & 0x1FFF_FFFF
        } else {
            0
        }
    }

    fn activate_frontbuffer(&self) {
        let mut frontbuffer_lock = self.framebuffers.frontbuffer().lock();
        if let Some(frontbuffer) = frontbuffer_lock.as_mut() {
            let pixels = frontbuffer.pixels_mut();
            let ptr = pixels.as_ptr();
            let dram_address = ((ptr as u32) & 0x1FFF_FFFF) | 0xA000_0000;

            // The framebuffer is accessed cached by the CPU, so invalidate it now
            for i in (0..pixels.len()).step_by(8) {
                unsafe {
                    crate::cop0::cache::<0b001, 0>(ptr.add(i) as usize);
                }
            }

            unsafe {
                VI_BASE_REG.add(RegisterOffset::DRAMAddress as usize >> 2).write_volatile(dram_address);
            }
        }
    }

    pub fn current_scanline(&self) -> u32 {
        unsafe { VI_BASE_REG.add(RegisterOffset::Current as usize >> 2).read_volatile() }
    }

    pub fn spinwait_for_vsync(&self) {
        // 520, 522, 524, 0, 2, 4, 6, 8, 10...
        loop {
            if self.current_scanline() < 10 {
                break;
            }
        }
    }

    pub fn disable_video(&self) -> VideoDisabler {
        VideoDisabler::new(&self)
    }
}

pub struct VideoDisabler<'a> {
    vi: &'a Video,
    previous: StatusFramebufferType,
}

impl<'a> VideoDisabler<'a> {
    pub fn new(vi: &'a Video) -> Self {
        let previous = vi.set_framebuffer_type(StatusFramebufferType::Off);
        Self { vi, previous }
    }
}

impl<'a> Drop for VideoDisabler<'a> {
    fn drop(&mut self) {
        self.vi.set_framebuffer_type(self.previous);
    }
}