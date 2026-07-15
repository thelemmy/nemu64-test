// Memory map:
// 0x8000_0000 to (as much as needed): bss, text, data, rodata
// until 3.0mb: heap (including the framebuffer)
// growing down from the end: stack

static mut MEMORY_SIZE: usize = 0;
static mut ELF_HEADER_OFFSET: usize = 0;

pub struct MemoryMap {}

impl MemoryMap {
    pub const fn addr32_to_usize(from: u32) -> usize {
        from as i32 as usize
    }

    pub const HEAP_END: usize = 7 * 1024 * 1024;
    pub const HEAP_END_VIRTUAL_UNCACHED: usize =
        Self::addr32_to_usize(0xA000_0000) | MemoryMap::HEAP_END;

    pub const PHYSICAL_SPMEM_BASE: usize = 0x0400_0000;
    pub const PHYSICAL_PIFRAM_BASE: usize = 0x1FC0_07C0;

    /// Call very early (before setting up exception handlers) during boot to set memory size
    pub(super) fn init(memory_size: usize, elf_header_offset: usize) {
        assert_eq!(Self::memory_size(), 0);
        unsafe {
            MEMORY_SIZE = memory_size;
            ELF_HEADER_OFFSET = elf_header_offset;
        };
    }

    /// Returns the total memory size of this device (either 4MB or 8MB)
    pub fn memory_size() -> usize {
        // MEMORY_SIZE is only set during early boot and then never again, so this should be safe
        unsafe { MEMORY_SIZE }
    }

    /// Returns the number of bytes that the elf header is offset RAM vs ROM
    pub fn elf_header_offset() -> usize {
        // MEMORY_SIZE is only set during early boot and then never again, so this should be safe
        unsafe { ELF_HEADER_OFFSET }
    }

    /// Returns an uncached pointer of the given pointer (e.g. 0xA000_1234 is returned for 0x8000_1234
    pub fn uncached<T>(p: *const T) -> *const T {
        let memory_address = p as usize;
        assert_eq!(memory_address & 0xE000_0000, 0x8000_0000);
        ((((memory_address as i32) & 0x1FFF_FFFF) | (0xA000_0000u32 as i32)) as usize) as *const T
    }

    pub fn uncached_mut<T>(p: *mut T) -> *mut T {
        Self::uncached(p) as *mut T
    }

    /// Returns the cartridge (rom) address of a given constant
    pub fn physical_cart_address<T>(p: *const T) -> u32 {
        extern "C" {
            static loaded_rom_start: u8;
            static loaded_rom_end: u8;
        }

        let memory_address = p as u32;
        let start = unsafe { &loaded_rom_start as *const u8 as u32 };
        let end = unsafe { &loaded_rom_end as *const u8 as u32 };
        assert!(memory_address >= start);
        assert!(memory_address < end);

        memory_address - 0x8000_0000 + 0x10000000 + MemoryMap::elf_header_offset() as u32
    }

    pub fn uncached_cart_address<T>(p: *const T) -> *const T {
        Self::addr32_to_usize(Self::physical_cart_address(p) | 0xA000_0000) as *const T
    }

    pub fn physical_to_uncached_mut<T>(address: usize) -> *mut T {
        (address | 0xFFFF_FFFFA000_0000) as *mut T
    }

    pub fn physical_to_cached_mut<T>(address: usize) -> *mut T {
        Self::physical_to_cached::<T>(address) as *mut T
    }

    pub fn physical_to_cached<T>(address: usize) -> *const T {
        (address | 0xFFFF_FFFF_8000_0000) as *const T
    }

    pub fn uncached_to_physical_mut<T>(p: *mut T) -> usize {
        (p as usize) & 0x1FFF_FFFF
    }

    pub fn cached_to_physical_mut<T>(p: *mut T) -> usize {
        (p as usize) & 0x1FFF_FFFF
    }

    pub fn uncached_spmem_address<T>(offset: usize) -> *mut T {
        Self::physical_to_uncached_mut::<T>(Self::PHYSICAL_SPMEM_BASE + offset)
    }

    pub fn uncached_pifram_address<T>(offset: usize) -> *mut T {
        Self::physical_to_uncached_mut::<T>(Self::PHYSICAL_PIFRAM_BASE + offset)
    }
}
