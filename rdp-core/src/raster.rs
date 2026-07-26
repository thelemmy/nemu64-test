//! Rasterization. Span-based: primitives are broken into horizontal pixel spans, per-span state is
//! resolved to plain integers up front, and the span loop is branch-free per pixel so a SIMD
//! backend can later process it in lanes. The scalar code here is the specification.

use crate::rdram::Rdram;
use crate::state::State;

/// FillRectangle in fill cycle type.
///
/// Coordinates are 10.2 fixed point; xl/yl is the lower-right corner, xh/yh the upper-left.
/// Provisional semantics (to be pinned down by the differential tests):
/// - Horizontal: xl is clamped to the scissor right at subpixel precision, then the pixel
///   containing that subpixel is still painted. So an unclipped rectangle includes its xl pixel
///   (the classic fill-mode extra pixel), and a scissor-clipped one paints one pixel AT the
///   scissor right - one past what the scissor nominally allows, wrapping into the next
///   framebuffer row (hardware-observed).
/// - Vertical: the yl pixel is included, but the scissor bottom is subpixel-exclusive - no row at
///   or past it is painted (hardware-observed; the asymmetry to horizontal is real).
/// - 16bpp: the 32-bit fill color holds two pixels; even x takes bits 31..=16, odd x bits 15..=0
///   (hardware-observed).
/// - The hidden bits are left untouched for now; what fill mode writes there is untested.
pub(crate) fn fill_rectangle(state: &State, mem: &mut impl Rdram, word: u64) {
    let xl = ((word >> 44) & 0xFFF) as i32;
    let yl = ((word >> 32) & 0xFFF) as i32;
    let xh = ((word >> 12) & 0xFFF) as i32;
    let yh = (word & 0xFFF) as i32;

    let (sc_left, sc_top, sc_right, sc_bottom) = state.scissor_bounds();

    // Pixel bounds, inclusive.
    let x0 = (xh >> 2).max((sc_left >> 2) as i32);
    let x1 = xl.min(sc_right as i32) >> 2;
    let y0 = (yh >> 2).max((sc_top >> 2) as i32);
    let y1 = yl.min(sc_bottom as i32 - 1) >> 2;

    for y in y0..=y1 {
        fill_span(state, mem, y, x0, x1);
    }
}

/// Fills the pixels x0..=x1 of row y with the fill color.
fn fill_span(state: &State, mem: &mut impl Rdram, y: i32, x0: i32, x1: i32) {
    let base = state.color_image_addr();
    let stride_pixels = state.color_image_width() + 1;
    let fill = state.fill_color;

    match state.color_image_size() {
        2 => {
            // 16bpp: two packed pixels in the fill color, selected by x parity.
            let row = base + (y as u32) * stride_pixels * 2;
            for x in x0..=x1 {
                let value = if x & 1 == 0 { fill >> 16 } else { fill };
                mem.write_u16(row + (x as u32) * 2, value as u16);
            }
        }
        3 => {
            // 32bpp: every pixel gets the full fill color.
            let row = base + (y as u32) * stride_pixels * 4;
            for x in x0..=x1 {
                mem.write_u32(row + (x as u32) * 4, fill);
            }
        }
        // 4bpp/8bpp framebuffers: untested, not implemented yet.
        _ => {}
    }
}
