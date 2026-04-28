//! Testsuite for a wide variety of N64 features and behaviors.
//!
//! All tests included in this suite are found in the [`tests`] module.

#![no_std]
#![feature(alloc_error_handler)]
#![feature(asm_const)]
#![feature(asm_experimental_arch)]
#![feature(naked_functions)]
#![feature(step_trait)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(rustdoc::private_intra_doc_links)]
#![no_main]
#![deny(unused_must_use)]

extern crate alloc;

use core::arch::asm;

use spinning_top::Spinlock;

use crate::cop1::set_fcsr;
use crate::cop1::FCSR;
use crate::graphics::framebuffer_console::FramebufferConsole;
use crate::graphics::vi::Video;
use crate::memory_map::MemoryMap;
use crate::rsp::spmem::SPMEM;

mod allocator;
mod assembler;
mod cop0;
mod cop1;
mod exception_handler;
mod graphics;
mod isviewer;
mod math;
mod memory_map;
mod mi;
mod panic;
mod pi;
mod print;
mod rdp;
mod rsp;
mod tests;
mod uncached_memory;

static VIDEO: Spinlock<Video> = Spinlock::new(Video::new());
static mut IPL3_TV_TYPE: u8 = 0;

#[no_mangle]
unsafe extern "C" fn entrypoint() -> ! {
    // Tests require these to be 0. Can't mark as clobbered as they are reserved as far as the compiler is concerned
    unsafe {
        asm!(
            "move $26, $zero",
            "move $27, $zero",
            options(nomem, nostack)
        );
    }

    // IPL3 (the bootloader) write the memory size to DMEM. We can read it from there
    let memory_size = SPMEM::read(0) as usize;
    let elf_header_offset = ((SPMEM::read(12) >> 16) << 8) as usize;
    unsafe {
        IPL3_TV_TYPE = SPMEM::read_u8(9);
    }
    MemoryMap::init(memory_size, elf_header_offset);

    // fcsr isn't reset on boot. Use a good default for the main loop - some tests will change and
    // restore this
    set_fcsr(
        FCSR::DEFAULT
            .with_flush_denorm_to_zero(true)
            .with_enable_invalid_operation(true),
    );

    mi::clear_interrupt_mask();
    allocator::init_allocator();
    main();

    loop {}
}

fn main() {
    exception_handler::install_exception_handlers();
    let video_init = VIDEO.lock();
    video_init.init(unsafe { IPL3_TV_TYPE });
    video_init.alloc_framebuffer();
    drop(video_init);
    tests::run();

    let v = VIDEO.lock();
    FramebufferConsole::instance()
        .lock()
        .render(v.framebuffers().backbuffer().lock().as_mut().unwrap());
    v.swap_buffers();
}

/// Renders the framebuffer console to the screen. Best-effort and non-blocking (uses
/// `try_lock`), so it is safe to call from the panic handler and from the hot test loop
/// without risking a deadlock. This is what keeps a hang or panic from showing a black
/// screen: the last state (including the currently running test) stays visible.
pub fn render_console() {
    let Some(video) = VIDEO.try_lock() else {
        return;
    };
    let Some(console) = FramebufferConsole::instance().try_lock() else {
        return;
    };
    {
        let Some(mut backbuffer) = video.framebuffers().backbuffer().try_lock() else {
            return;
        };
        let Some(image) = backbuffer.as_mut() else {
            return;
        };
        console.render(image);
    }
    video.swap_buffers();
}
