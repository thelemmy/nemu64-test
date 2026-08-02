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

/// (a * b) >> 16 for two s15.16 values without touching i64: the MIPS backend's dropped-
/// truncation bug (docs/i64-value-corruption.md) makes 64-bit intermediates hazardous here, so
/// the product is assembled from 16-bit limbs.
fn fixed_mul(a: i32, b: i32) -> i32 {
    let a_int = a >> 16;
    let a_frac = (a & 0xFFFF) as u32;
    let b_int = b >> 16;
    let b_frac = (b & 0xFFFF) as u32;
    let cross = a_int
        .wrapping_mul(b_frac as i32)
        .wrapping_add(b_int.wrapping_mul(a_frac as i32));
    let low = ((a_frac.wrapping_mul(b_frac)) >> 16) as i32;
    a_int
        .wrapping_mul(b_int)
        .wrapping_shl(16)
        .wrapping_add(cross)
        .wrapping_add(low)
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
    // word once this function is inlined into the command loop. See docs/i64-value-corruption.md (branch bug/mips-i64-miscompile).
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

/// Fill_Triangle in 1-cycle mode with coverage modes CLAMP / WRAP / SAVE: like
/// [`one_cycle_fill_triangle`], but the written alpha byte carries the pixel's coverage.
///
/// Provisional model (this is the frontier - the differential tests are expected to correct it):
/// - WHICH pixels are painted follows the same top-left-subpixel rule as zap (hardware-observed:
///   a pixel with 12/16 coverage on the left edge stays unpainted): px_start/px_end come from the
///   row's top subpixel line, and a fractional yh skips its partial top row entirely.
/// - The coverage is sampled on a CHECKERBOARD: of the 16 subpixels only the 8 with an even
///   (sx ^ sy) contribute, accumulated over all four subpixel lines of the row (on each line the
///   subpixel columns [left, right - 2.0 subpixel units] are covered). Memory stores the sum in
///   coverage-minus-one encoding: alpha = (sum - 1) << 5, full coverage = 0xE0. The sum only
///   affects the written alpha, not which pixels are written. This fits every measured value:
///   full-column right-edge partials (2 samples per column), mixed-row cases distinguishing
///   which subpixels count (e.g. cols 0..2 full + col 3 on rows 1..3 -> 8 -> 0xE0 because
///   (0,3) is odd and never counted; col 2 on rows 1..3 -> 5 -> 0x80 because (0,2) is even).
/// - 32bpp alpha byte = CLAMP_ALPHA[count]; entries not pinned down by the old gated tests are
///   the marker value 0x01 so a hardware mismatch reveals the true value.
/// - 32bpp framebuffers only for now (the alpha byte is CPU-visible there).
pub(crate) fn one_cycle_fill_triangle_coverage(
    state: &State,
    mem: &mut impl Rdram,
    cmd: &[u64],
) -> bool {
    if state.blender_0() != (2, 0, 1, 3) || state.color_image_size() != 3 {
        return false;
    }
    // 0 = clamp, 1 = wrap (provisionally like clamp: without a memory coverage source there is
    // nothing to wrap against), 3 = save (memory keeps its alpha byte)
    let mode = state.coverage_mode();

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
    let first_pixel_x = ((sc_left as i32) + 3) >> 2;
    let first_pixel_y = ((sc_top as i32) + 3) >> 2;
    // The partially covered top pixel row of a fractional yh produces no coverage
    let y_start = (yh + 3) & !3;

    let blend = state.blend_color;
    let (r, g, b) = (blend >> 24, (blend >> 16) & 0xFF, (blend >> 8) & 0xFF);
    let base = state.color_image_addr();
    let stride_pixels = state.color_image_width() + 1;

    // Subpixel coverage counts for the current pixel row, plus the painted range determined by
    // the row's top subpixel line
    let mut coverage = [0u8; 1024];
    let mut row_y = i32::MIN;
    let mut row_px_start = 0i32;
    let mut row_px_end = -1i32;

    // The major edge is sampled one step ahead of the minor edge (hardware-observed: a major
    // edge crossing exactly onto a subpixel boundary mid-triangle loses that sample, while a
    // minor edge or a constant major edge does not)
    let mut major_x = xh.wrapping_add(dh);
    let mut minor_x = xm;
    let mut y = yh;
    let mut section = 0;
    loop {
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

        if y >= y_start && (y >> 2) >= first_pixel_y {
            if (y >> 2) != row_y {
                if row_y != i32::MIN {
                    flush_coverage_row(
                        mem,
                        &mut coverage,
                        row_y,
                        row_px_start,
                        row_px_end,
                        base,
                        stride_pixels,
                        (r, g, b),
                        mode,
                    );
                }
                row_y = y >> 2;
                row_px_start = 0;
                row_px_end = -1;
            }

            let (left, right) = if right_major {
                (major_x, minor_x)
            } else {
                (minor_x, major_x)
            };
            if right >= left {
                // The row's top subpixel line decides which pixels are painted
                if (y & 3) == 0 {
                    row_px_start = (left.wrapping_add((1 << 18) - 1) >> 18).max(first_pixel_x);
                    row_px_end = ((right.wrapping_sub(2 << 2) >> 16).min(sc_right as i32 - 1)) >> 2;
                }
                // Every line contributes its checkerboard samples to the coverage sums. A
                // subpixel column is covered iff it lies at or right of the edge, so the start
                // rounds UP at subpixel precision.
                let sx_start = (left.wrapping_add(0xFFFF) >> 16).max(first_pixel_x << 2);
                let sx_end = (right.wrapping_sub(2 << 2) >> 16).min(sc_right as i32 - 1);
                let mut sx = sx_start;
                while sx <= sx_end {
                    let px = sx >> 2;
                    if ((sx ^ y) & 1) == 0 && (0..1024).contains(&px) {
                        coverage[px as usize] += 1;
                    }
                    sx += 1;
                }
            }
        }

        major_x += dh;
        minor_x += minor_inc;
        y += 1;
    }
    if row_y != i32::MIN {
        flush_coverage_row(
            mem,
            &mut coverage,
            row_y,
            row_px_start,
            row_px_end,
            base,
            stride_pixels,
            (r, g, b),
            mode,
        );
    }

    true
}

#[allow(clippy::too_many_arguments)]
fn flush_coverage_row(
    mem: &mut impl Rdram,
    coverage: &mut [u8; 1024],
    row_y: i32,
    px_start: i32,
    px_end: i32,
    base: u32,
    stride_pixels: u32,
    (r, g, b): (u32, u32, u32),
    mode: u32,
) {
    let row = base + (row_y as u32) * stride_pixels * 4;
    for (x, count) in coverage.iter_mut().enumerate() {
        if *count > 0 {
            if (px_start..=px_end).contains(&(x as i32)) {
                let address = row + (x as u32) * 4;
                // Coverage-minus-one from the checkerboard sample sum (1..=8; a painted pixel
                // always has its top-left subpixel covered, which is a checkerboard sample)
                let sum = (*count).min(8) as u32;
                let alpha = match mode {
                    // save writes the MEMORY coverage back - but with image-read disabled the
                    // memory coverage is never fetched and reads as full (hardware-observed:
                    // 0xE0 regardless of the previous alpha byte). The read-modify path behind
                    // the image-read enable bit is still unexplored.
                    3 => 0xE0,
                    // clamp; wrap is identical without a memory coverage source to wrap against
                    // (hardware-observed)
                    _ => (sum - 1) << 5,
                };
                let value = (r << 24) | (g << 16) | (b << 8) | alpha;
                mem.write_u32(address, value);
            }
            *count = 0;
        }
    }
}

/// The Set_Combine configuration meaning "output = shade" in both cycles:
/// RGB (a - b) * c + d with a = zero(8), c = zero(16), b = zero(8), d = SHADE(4);
/// alpha with a = b = c = zero(7), d = SHADE(4). Encoding is the community bit layout - it is
/// hardware-verified exactly insofar as the shade tests below pass with it.
pub const COMBINE_PASSTHROUGH_SHADE: u64 = (8 << 52)
    | (16 << 47)
    | (7 << 44)
    | (7 << 41)
    | (8 << 37)
    | (16 << 32)
    | (8 << 28)
    | (8 << 24)
    | (7 << 21)
    | (7 << 18)
    | (4 << 15)
    | (7 << 12)
    | (4 << 9)
    | (4 << 6)
    | (7 << 3)
    | 4;

/// Shade_Triangle (opcode 0x0C) in 1-cycle mode, restricted to the combiner passthrough above,
/// the verified blender configuration with P = CombineColor, and coverage mode zap.
///
/// Provisional model (the differential corrects it):
/// - Shade coefficient words after the 4 edge words: RGBA int parts (4 x s16, high to low
///   r,g,b,a), DxDx int parts, RGBA fractions, DcDx fractions, DcDe int, DcDy int, DcDe frac,
///   DcDy frac. Channels are s15.16.
/// - The color of a pixel is base + (rows since yh's pixel row, including a skipped partial top
///   row) * DcDe + (sample position - exact fractional left edge position) * DcDx / 4, where the
///   sample position is subpixel px*4 for even span-relative pixels and px*4 + G for odd ones,
///   with G = sx_first % 4 for a static left edge and ((sx_first - 1) % 4) + 1 for a sloped one
///   (sx_first = the span's first covered subpixel column). Hardware-observed across static
///   integer, static fractional, sloped integer and sloped fractional left edges.
/// - Written bytes are the integer parts' low 8 bits; alpha byte = coverage (0xE0 under zap).
pub(crate) fn one_cycle_shade_triangle(state: &State, mem: &mut impl Rdram, cmd: &[u64]) -> bool {
    if state.blender_0() != (0, 0, 1, 3)
        || state.coverage_mode() != 2
        || state.combine & 0x00FF_FFFF_FFFF_FFFF != COMBINE_PASSTHROUGH_SHADE
        || state.color_image_size() != 3
    {
        return false;
    }

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

    // Shade coefficients: s15.16 per channel, split into int and fraction words
    let channel = |int_word: u64, frac_word: u64, lane: u32| -> i32 {
        let int_part = (int_word >> (48 - lane * 16)) as u16;
        let frac_part = (frac_word >> (48 - lane * 16)) as u16;
        (((int_part as i16 as i32) << 16) as u32 | frac_part as u32) as i32
    };
    let mut color = [0i32; 4];
    let mut dcdx = [0i32; 4];
    let mut dcde = [0i32; 4];
    for lane in 0..4 {
        color[lane as usize] = channel(cmd[4], cmd[6], lane);
        dcdx[lane as usize] = channel(cmd[5], cmd[7], lane);
        dcde[lane as usize] = channel(cmd[8], cmd[10], lane);
    }

    let (sc_left, sc_top, sc_right, sc_bottom) = state.scissor_bounds();
    let first_pixel_x = ((sc_left as i32) + 3) >> 2;
    let first_pixel_y = ((sc_top as i32) + 3) >> 2;

    let base = state.color_image_addr();
    let stride_pixels = state.color_image_width() + 1;

    let yh_row = yh >> 2;
    let mut major_x = xh;
    let mut minor_x = xm;
    let mut y = yh;
    let mut section = 0;
    loop {
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

        if (y & 3) == 0 && (y >> 2) >= first_pixel_y {
            let (left, right) = if right_major {
                (major_x, minor_x)
            } else {
                (minor_x, major_x)
            };
            if right >= left {
                let px_start = (left.wrapping_add((1 << 18) - 1) >> 18).max(first_pixel_x);
                let px_end = ((right.wrapping_sub(2 << 2) >> 16).min(sc_right as i32 - 1)) >> 2;
                if px_start <= px_end {
                    let py = (y >> 2) as u32;
                    let de_steps = (y >> 2) - yh_row;
                    let sx_first = left.wrapping_add(0xFFFF) >> 16;
                    let left_step = if right_major { dh } else { minor_inc };
                    // Odd span-relative pixels sample G subpixels further right
                    let g_odd = if left_step == 0 {
                        sx_first & 3
                    } else {
                        ((sx_first - 1) & 3) + 1
                    };
                    // Exact fractional edge position in 16.16 subpixels (the <<2-domain
                    // accumulator is 16.16 subpixels already)
                    let left_sub16 = left;
                    let row_addr = base + py * stride_pixels * 4;
                    let mut row_base = [0i32; 4];
                    for lane in 0..4 {
                        row_base[lane] =
                            color[lane].wrapping_add(dcde[lane].wrapping_mul(de_steps));
                    }
                    let mut px = px_start;
                    while px <= px_end {
                        let rel_odd = ((px - px_start) & 1) == 1;
                        let sample = (px << 2) + if rel_odd { g_odd } else { 0 };
                        let dist16 = (sample << 16).wrapping_sub(left_sub16);
                        let mut sampled = [0i32; 4];
                        for lane in 0..4 {
                            sampled[lane] =
                                row_base[lane].wrapping_add(fixed_mul(dist16, dcdx[lane] >> 2));
                        }
                        let r = (sampled[0] >> 16) as u32 & 0xFF;
                        let g = (sampled[1] >> 16) as u32 & 0xFF;
                        let b = (sampled[2] >> 16) as u32 & 0xFF;
                        let value = (r << 24) | (g << 16) | (b << 8) | 0xE0;
                        mem.write_u32(row_addr + (px as u32) * 4, value);
                        px += 1;
                    }
                }
            }
        }

        major_x += dh;
        minor_x += minor_inc;
        y += 1;
    }

    true
}

/// The Set_Combine configuration meaning "output = texel 0": the shade passthrough above with
/// every SHADE selector replaced by TEXEL0.
pub const COMBINE_PASSTHROUGH_TEXEL0: u64 = (8 << 52)
    | (16 << 47)
    | (7 << 44)
    | (7 << 41)
    | (8 << 37)
    | (16 << 32)
    | (8 << 28)
    | (8 << 24)
    | (7 << 21)
    | (7 << 18)
    | (1 << 15)
    | (7 << 12)
    | (1 << 9)
    | (1 << 6)
    | (7 << 3)
    | 1;

/// Load_Tile (opcode 0x34): copies a rectangle of the texture image into the tile's TMEM range.
///
/// Provisional model:
/// - The rectangle is [sl, sh] x [tl, th] in TEXELS (the 10.2 fields carry whole texels here).
/// - Source rows come from the texture image at its own width; destination rows are the tile's
///   `line` 64-bit words apart, starting at the tile's `tmem` word.
/// - Bytes are copied verbatim, no swizzling. The odd-line word swapping that texture sampling
///   is known to involve is deliberately NOT modelled yet - the differential decides.
pub(crate) fn load_tile(state: &mut State, mem: &mut impl Rdram, word: u64) -> bool {
    let tile_index = ((word >> 24) & 7) as usize;
    let sl = (((word >> 44) & 0xFFF) >> 2) as u32;
    let tl = (((word >> 32) & 0xFFF) >> 2) as u32;
    let sh = (((word >> 12) & 0xFFF) >> 2) as u32;
    let th = ((word & 0xFFF) >> 2) as u32;

    let tile = state.tile(tile_index);
    let src_size = state.texture_image_size();
    if src_size != 3 || tile.size != 3 {
        // Only 32bpp RGBA is modelled so far
        return false;
    }
    let src_base = state.texture_image_addr();
    let src_stride = (state.texture_image_width() + 1) * 4;

    for (row, t) in (tl..=th).enumerate() {
        let dst = tile.tmem * 8 + (row as u32) * tile.line * 8;
        for (col, s) in (sl..=sh).enumerate() {
            let src = src_base + t * src_stride + s * 4;
            let value = mem.read_u32(src);
            let offset = dst + (col as u32) * 4;
            if offset as usize + 4 <= state.tmem.len() {
                state.tmem[offset as usize..offset as usize + 4]
                    .copy_from_slice(&value.to_be_bytes());
            }
        }
    }
    true
}

/// Texture_Rectangle (opcode 0x24) in 1-cycle mode with the texel passthrough combiner, the
/// verified blender configuration and coverage mode zap.
///
/// Provisional model:
/// - Screen rect follows the 1-cycle rectangle rule (top-left subpixel inside [xh, xl) x [yh, yl)).
/// - Texture coordinates start at (s, t) in 10.5 and step by dsdx / dtdy in 5.10 per pixel;
///   the texel is sampled at the truncated integer coordinate (point sampling).
/// - Texels are read from the tile's TMEM range at its `line` stride.
pub(crate) fn one_cycle_texture_rectangle(
    state: &State,
    mem: &mut impl Rdram,
    cmd: &[u64],
) -> bool {
    if state.blender_0() != (0, 0, 1, 3)
        || state.coverage_mode() != 2
        || state.combine & 0x00FF_FFFF_FFFF_FFFF != COMBINE_PASSTHROUGH_TEXEL0
        || state.color_image_size() != 3
    {
        return false;
    }

    let word = cmd[0];
    let xl = ((word >> 44) & 0xFFF) as i32;
    let yl = ((word >> 32) & 0xFFF) as i32;
    let tile_index = ((word >> 24) & 7) as usize;
    let xh = ((word >> 12) & 0xFFF) as i32;
    let yh = (word & 0xFFF) as i32;

    let coords = cmd[1];
    let s_start = ((coords >> 48) & 0xFFFF) as i32;
    let t_start = ((coords >> 32) & 0xFFFF) as i32;
    let dsdx = (coords >> 16) as u16 as i16 as i32;
    let dtdy = coords as u16 as i16 as i32;

    let tile = state.tile(tile_index);
    if tile.size != 3 {
        return false;
    }

    let (sc_left, sc_top, sc_right, sc_bottom) = state.scissor_bounds();
    let sx0 = xh.max(sc_left as i32);
    let sx1 = xl.min(sc_right as i32);
    let sy0 = yh.max(sc_top as i32);
    let sy1 = yl.min(sc_bottom as i32);
    if sx1 <= sx0 || sy1 <= sy0 {
        return true;
    }
    let x0 = (sx0 + 3) >> 2;
    let x1 = (sx1 - 1) >> 2;
    let y0 = (sy0 + 3) >> 2;
    let y1 = (sy1 - 1) >> 2;
    if x1 < x0 || y1 < y0 {
        return true;
    }

    let base = state.color_image_addr();
    let stride_pixels = state.color_image_width() + 1;

    for y in y0..=y1 {
        // s/t are 10.5; dsdx/dtdy are 5.10. Step in 10.5 by dsdx >> 5 per pixel.
        let t_coord = t_start + ((y - y0) * dtdy >> 5);
        let row_addr = base + (y as u32) * stride_pixels * 4;
        for x in x0..=x1 {
            let s_coord = s_start + ((x - x0) * dsdx >> 5);
            let s_texel = s_coord >> 5;
            let t_texel = t_coord >> 5;
            let offset =
                (tile.tmem * 8 + (t_texel as u32) * tile.line * 8 + (s_texel as u32) * 4) as usize;
            let texel = if offset + 4 <= state.tmem.len() {
                u32::from_be_bytes([
                    state.tmem[offset],
                    state.tmem[offset + 1],
                    state.tmem[offset + 2],
                    state.tmem[offset + 3],
                ])
            } else {
                0
            };
            // Texel RGB with coverage in the alpha byte
            let value = (texel & 0xFFFF_FF00) | 0xE0;
            mem.write_u32(row_addr + (x as u32) * 4, value);
        }
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
