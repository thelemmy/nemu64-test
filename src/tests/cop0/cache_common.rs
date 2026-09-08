use alloc::string::String;

use crate::cop0;
use crate::cop0::{ICACHE_BYTES, ICACHE_LINE_BYTES};
use crate::MemoryMap;

pub const ICACHE_SETS: usize = ICACHE_BYTES / ICACHE_LINE_BYTES;

/// One instruction-cache line per icache set, so a test can place its line in any of them.
#[repr(C, align(16384))]
struct IcacheArena {
    words: [u32; ICACHE_BYTES / 4],
}

static mut ICACHE_ARENA: IcacheArena = IcacheArena {
    words: [0; ICACHE_BYTES / 4],
};

/// The cached and uncached pointers to the arena line that occupies icache set `set`.
pub fn icache_line_ptrs(set: usize) -> (usize, *mut u32) {
    unsafe {
        let base = (&raw mut ICACHE_ARENA.words).cast::<u32>();
        let p = base.byte_add(set * ICACHE_LINE_BYTES);
        (p as usize, MemoryMap::uncached_mut(p))
    }
}

pub fn cache_subsystem_reset() {
    cop0::icache_invalidate_all();
    cop0::dcache_invalidate_all();
    unsafe {
        cop0::set_tag_lo(0);
    }
}

pub fn run_cache_isolated_test(body: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    let r = body();
    cache_subsystem_reset();
    r
}

/// Runs `body` against the arena line of each icache set in turn, stopping at the first set
/// that passes.
///
/// The 16 KB icache is direct-mapped, so a line's set is fixed by its address. These tests set
/// a line up, run a few instructions, then inspect it - and whenever the line shares a set with
/// that intervening code it is evicted or refilled and the test fails. Sweeping all 512 sets on
/// hardware, exactly the 2 the code occupies fail and the other 510 pass, so which outcome a
/// build got used to be down to where the linker put the code.
///
/// Detecting the eviction rather than avoiding it does not work: the check is itself code in
/// the window, and gets its own pair of sets that fail the same way. A real failure still fails
/// in every set and is reported.
pub fn with_line_in_a_free_set(
    body: impl Fn(usize, *mut u32) -> Result<(), String>,
) -> Result<(), String> {
    let mut result = Ok(());
    for set in 0..ICACHE_SETS {
        let (cached, uncached) = icache_line_ptrs(set);
        result = body(cached, uncached);
        if result.is_ok() {
            return result;
        }
    }
    result
}
