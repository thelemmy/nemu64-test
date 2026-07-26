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
///   or past it is painted (hardware-observed; the asymmetry to horizontal is real). The clamp
///   operates on subpixels, not pixel indices (hardware-observed with fractional scissor values).
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

/// FillRectangle in 1-cycle mode.
///
/// Only the pipeline configuration verified so far is implemented: blender cycle 0 with
/// P=BlendColor, B=Zero (output = blend color) and coverage mode zap (coverage forced to full).
/// Returns false for anything else so the caller can count it as unhandled.
///
/// Provisional semantics:
/// - A pixel (row) is painted iff its top-left subpixel lies inside [xh, xl) x [yh, yl):
///   left/top round UP (`(edge + 3) >> 2`, so a fractional xh/yh skips its partially covered
///   pixel), right/bottom still paint their containing pixel (`(edge - 1) >> 2`). Both
///   hardware-observed; note the asymmetry, and that there is no fill-mode extra pixel.
/// - The scissor clips the subpixel ranges on all four edges.
/// - Blend color word is r, g, b, a bytes high to low. 16bpp writes r/g/b truncated to 5 bit and
///   the alpha bit set (full coverage); 32bpp writes r, g, b and 0xE0 (coverage 7 << 5) as the
///   alpha byte.
/// - Hidden bits untouched for now.
pub(crate) fn one_cycle_rectangle(state: &State, mem: &mut impl Rdram, word: u64) -> bool {
    if state.blender_0() != (2, 0, 1, 3) || state.coverage_mode() != 2 {
        return false;
    }

    let xl = ((word >> 44) & 0xFFF) as i32;
    let yl = ((word >> 32) & 0xFFF) as i32;
    let xh = ((word >> 12) & 0xFFF) as i32;
    let yh = (word & 0xFFF) as i32;

    let (sc_left, sc_top, sc_right, sc_bottom) = state.scissor_bounds();

    // Covered subpixel ranges, exclusive on the right/bottom, clipped by the scissor
    let sx0 = xh.max(sc_left as i32);
    let sx1 = xl.min(sc_right as i32);
    let sy0 = yh.max(sc_top as i32);
    let sy1 = yl.min(sc_bottom as i32);
    if sx1 <= sx0 || sy1 <= sy0 {
        return true;
    }

    // Pixels whose top-left subpixel is covered
    let x0 = (sx0 + 3) >> 2;
    let x1 = (sx1 - 1) >> 2;
    let y0 = (sy0 + 3) >> 2;
    let y1 = (sy1 - 1) >> 2;
    if x1 < x0 || y1 < y0 {
        return true;
    }

    let blend = state.blend_color;
    let (r, g, b) = (blend >> 24, (blend >> 16) & 0xFF, (blend >> 8) & 0xFF);

    let base = state.color_image_addr();
    let stride_pixels = state.color_image_width() + 1;
    match state.color_image_size() {
        2 => {
            let value = (((r >> 3) << 11) | ((g >> 3) << 6) | ((b >> 3) << 1) | 1) as u16;
            for y in y0..=y1 {
                let row = base + (y as u32) * stride_pixels * 2;
                for x in x0..=x1 {
                    mem.write_u16(row + (x as u32) * 2, value);
                }
            }
        }
        3 => {
            let value = (r << 24) | (g << 16) | (b << 8) | 0xE0;
            for y in y0..=y1 {
                let row = base + (y as u32) * stride_pixels * 4;
                for x in x0..=x1 {
                    mem.write_u32(row + (x as u32) * 4, value);
                }
            }
        }
        _ => return false,
    }
    true
}

const fn sign_extend_14(value: u32) -> i32 {
    ((value << 18) as i32) >> 18
}

/// Fill_Triangle (opcode 0x08) in 1-cycle mode, restricted like [`one_cycle_rectangle`] to the
/// verified pipeline configuration (blender output = blend color, coverage mode zap - so this is
/// geometry only; the actual coverage counts don't matter yet, only covered vs. not).
///
/// Semantics (hardware-verified by "SoftRDP: FillTriangle (1-cycle)"):
/// - yl/ym/yh are 12.2 signed; xl/xm/xh and the per-scanline steps dl/dm/dh are 16.16 signed.
/// - The major edge starts at xh and steps dh once per subpixel line from yh; the minor side
///   steps xm/dm until ym, then restarts at xl stepping dl until yl. `right_major` (bit 55)
///   selects which side is left/right.
/// - Which pixels are painted follows the same rule as the 1-cycle rectangle
///   (hardware-observed, three separate observations): a pixel is painted iff its TOP-LEFT
///   subpixel is inside the primitive. Equivalently, only each pixel row's top subpixel line
///   (y % 4 == 0) is sampled, and on it only whole-pixel subpixel columns: left rounds up to the
///   next pixel, right paints its containing pixel, a fractional yh skips its partial top row,
///   yl paints its containing row. The edges still step across every subpixel line.
/// - Scissor: top and left clip at pixel granularity (`(edge + 3) >> 2` pixels), right and bottom
///   at subpixel granularity.
///
/// Whether the unsampled subpixels matter for the coverage VALUE (clamp/save modes, AA blending)
/// is untested - zap hides them. This will need revisiting when coverage lands.
pub(crate) fn one_cycle_fill_triangle(state: &State, mem: &mut impl Rdram, cmd: &[u64]) -> bool {
    if state.blender_0() != (2, 0, 1, 3) || state.coverage_mode() != 2 {
        return false;
    }
    let size = state.color_image_size();
    if size != 2 && size != 3 {
        return false;
    }

    // All edge math is i32: x accumulators live in a <<2-shifted 16.16 domain (2 fraction bits
    // more so the per-subpixel-line step keeps full precision), which still fits i32 for the
    // whole 10.2 coordinate space - the hardware's own edge registers aren't wider either.
    // Spelling these as i64 also breaks on console: the values come back with a corrupted high
    // word once this function is inlined into the command loop. See docs/mips-i64-codegen-bug.md.
    let right_major = (cmd[0] >> 55) & 1 != 0;
    let yl = sign_extend_14(((cmd[0] >> 32) & 0x3FFF) as u32);
    let ym = sign_extend_14(((cmd[0] >> 16) & 0x3FFF) as u32);
    let yh = sign_extend_14((cmd[0] & 0x3FFF) as u32);
    let xl = ((cmd[1] >> 32) as u32 as i32).wrapping_shl(2);
    let dl = cmd[1] as u32 as i32;
    let xh = ((cmd[2] >> 32) as u32 as i32).wrapping_shl(2);
    let dh = cmd[2] as u32 as i32;
    let xm = ((cmd[3] >> 32) as u32 as i32).wrapping_shl(2);
    let dm = cmd[3] as u32 as i32;

    let (sc_left, sc_top, sc_right, sc_bottom) = state.scissor_bounds();
    // Left/top clip at pixel granularity, right/bottom at subpixel granularity
    let first_pixel_x = ((sc_left as i32) + 3) >> 2;
    let first_pixel_y = ((sc_top as i32) + 3) >> 2;

    // Resolve the pixel write up front
    let blend = state.blend_color;
    let (r, g, b) = (blend >> 24, (blend >> 16) & 0xFF, (blend >> 8) & 0xFF);
    let base = state.color_image_addr();
    let stride_pixels = state.color_image_width() + 1;
    let value16 = (((r >> 3) << 11) | ((g >> 3) << 6) | ((b >> 3) << 1) | 1) as u16;
    let value32 = (r << 24) | (g << 16) | (b << 8) | 0xE0;

    let mut major_x = xh;
    let mut minor_x = xm;
    let mut y = yh;
    let mut section = 0;
    loop {
        // Two sections: the minor side walks xm/dm until ym, then restarts at xl/dl until yl
        let (y_target, minor_inc) = if section == 0 { (ym, dm) } else { (yl, dl) };
        if y >= y_target {
            if section == 1 {
                break;
            }
            section = 1;
            minor_x = xl;
            continue;
        }
        if y >= sc_bottom as i32 {
            break;
        }

        // Only each pixel row's top subpixel line determines the painted pixels
        if (y & 3) == 0 && (y >> 2) >= first_pixel_y {
            let (left, right) = if right_major {
                (major_x, minor_x)
            } else {
                (minor_x, major_x)
            };
            if right >= left {
                // A pixel is painted iff its top-left subpixel is inside the span: the left edge
                // rounds up to the next pixel whose first subpixel column is covered, the right
                // edge paints its containing pixel (same rule as the 1-cycle rectangle,
                // hardware-observed)
                let px_start = (left.wrapping_add((1 << 18) - 1) >> 18).max(first_pixel_x);
                let px_end = ((right.wrapping_sub(2 << 2) >> 16).min(sc_right as i32 - 1)) >> 2;
                let py = (y >> 2) as u32;
                let mut px = px_start;
                while px <= px_end {
                    if size == 2 {
                        mem.write_u16(base + py * stride_pixels * 2 + (px as u32) * 2, value16);
                    } else {
                        mem.write_u32(base + py * stride_pixels * 4 + (px as u32) * 4, value32);
                    }
                    px += 1;
                }
            }
        }

        major_x += dh;
        minor_x += minor_inc;
        y += 1;
    }

    true
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
