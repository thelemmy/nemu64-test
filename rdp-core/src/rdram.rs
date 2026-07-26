//! RDRAM as the RDP sees it: 9 bit wide bytes, big-endian, addressed physically.
//!
//! The 9th ("hidden") bits are modeled per 16-bit word as a 2-bit value, since that is the
//! granularity the RDP uses them at (coverage/dz storage for 16-bit framebuffers). They are not
//! CPU-visible on real hardware; an emulator keeps them in a separate plane.

/// Physical memory interface for the RDP. All addresses are physical RDRAM addresses.
pub trait Rdram {
    fn read_u8(&self, addr: u32) -> u8;
    fn write_u8(&mut self, addr: u32, value: u8);

    /// The two hidden bits of the 16-bit word containing `addr` (bit 1 = even byte's, bit 0 = odd
    /// byte's 9th bit).
    fn read_hidden(&self, addr: u32) -> u8;
    fn write_hidden(&mut self, addr: u32, value: u8);

    fn read_u16(&self, addr: u32) -> u16 {
        ((self.read_u8(addr) as u16) << 8) | (self.read_u8(addr + 1) as u16)
    }

    fn write_u16(&mut self, addr: u32, value: u16) {
        self.write_u8(addr, (value >> 8) as u8);
        self.write_u8(addr + 1, value as u8);
    }

    fn read_u32(&self, addr: u32) -> u32 {
        ((self.read_u16(addr) as u32) << 16) | (self.read_u16(addr + 2) as u32)
    }

    fn write_u32(&mut self, addr: u32, value: u32) {
        self.write_u16(addr, (value >> 16) as u16);
        self.write_u16(addr + 2, value as u16);
    }
}

/// An [`Rdram`] over plain slices, windowed at `base`: index 0 of the slices corresponds to
/// physical address `base`. `hidden` holds one entry per 16-bit word (so `bytes.len() / 2`
/// entries), each 0..=3. Accesses outside the window panic - in the differential tests that means
/// the soft RDP touched memory the real RDP was not given.
pub struct SliceRdram<'a> {
    base: u32,
    bytes: &'a mut [u8],
    hidden: &'a mut [u8],
}

impl<'a> SliceRdram<'a> {
    pub fn new(base: u32, bytes: &'a mut [u8], hidden: &'a mut [u8]) -> Self {
        assert!(hidden.len() >= bytes.len() / 2);
        Self {
            base,
            bytes,
            hidden,
        }
    }

    fn index(&self, addr: u32) -> usize {
        (addr - self.base) as usize
    }
}

impl Rdram for SliceRdram<'_> {
    fn read_u8(&self, addr: u32) -> u8 {
        self.bytes[self.index(addr)]
    }

    fn write_u8(&mut self, addr: u32, value: u8) {
        let index = self.index(addr);
        self.bytes[index] = value;
    }

    fn read_hidden(&self, addr: u32) -> u8 {
        self.hidden[self.index(addr) >> 1]
    }

    fn write_hidden(&mut self, addr: u32, value: u8) {
        let index = self.index(addr) >> 1;
        self.hidden[index] = value & 3;
    }
}
