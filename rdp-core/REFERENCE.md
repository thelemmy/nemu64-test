# N64 RDP Reference

A reference manual for the Reality Display Processor, built from our own reverse engineering on
real hardware. Every statement is backed by a test in n64-systemtest that runs on a real N64;
existing software implementations are deliberately not used as a source.

Markers used throughout:

- **[verified]** - proven by a named test on real hardware.
- **[provisional]** - implemented in rdp-core and consistent with all hardware runs so far, but not
  yet pinned down by a dedicated test.
- **[open]** - unknown; a test still needs to be written.

## 1. CPU interface (DP registers, physical 0x0410_0000)

| Offset | Name       | Notes |
|-------:|------------|-------|
| 0x00   | DP_START   | Start address of the command stream |
| 0x04   | DP_END     | End address (exclusive); writing it kicks off execution |
| 0x08   | DP_CURRENT | Read-only fetch pointer |
| 0x0C   | DP_STATUS  | Read: status bits. Write: set/clear command bits |
| 0x10   | DP_CLOCK   | Read-only free-running 24-bit counter |

### Addresses and masking

- START, END and CURRENT are masked to `0x00FF_FFF8` on write: 24-bit address space, 64-bit
  aligned. **[verified: "RDP START & END REG (masking)"]**
- Writing START sets the `start-valid` status bit. While `start-valid` is set, further writes to
  START are ignored. Writing END clears `start-valid` and latches CURRENT to START.
  **[verified: "RSP STATUS: start-valid"]**
- With START == END nothing executes. Moving END forward executes commands up to END; this can be
  done incrementally to single-step a command list. START keeps its value while CURRENT advances.
  **[verified: "RDP STATUS: Flags during a run"]**
- END may exceed the DMEM size in xbus mode; the RDP performs an internal START < END style check
  rather than masking END to DMEM. Fetch wraps within DMEM.
  **[verified: "RDP STATUS: Run from DMEM (xbus) (overflowing dmem)"]**

### DP_STATUS bits (read)

| Bit    | Name                  | Observed behavior |
|-------:|-----------------------|-------------------|
| 0x001  | XBUS                  | Command fetch from RSP DMEM instead of RDRAM |
| 0x002  | FREEZE                | Execution frozen; registers stay writable/readable |
| 0x008  | START_GCLK            | Set while a list is being processed |
| 0x020  | PIPE_BUSY             | Set while a list is being processed |
| 0x080  | COMMAND_BUFFER_READY  | Set when the RDP is idle enough to accept commands |
| 0x200  | END_VALID             | See start/end-valid semantics above |
| 0x400  | START_VALID           | See start/end-valid semantics above |

- While a list is running (END advanced past unexecuted commands, before SyncFull):
  status == `COMMAND_BUFFER_READY | PIPE_BUSY | START_GCLK`. After executing a SyncFull it drops to
  just `COMMAND_BUFFER_READY`. **[verified: "RDP STATUS: Flags during a run"]**
- DMA_BUSY is set briefly while an instruction is copied in; not asserted on because the window is
  tiny. **[open: exact timing]**

### DP_STATUS write (command) bits

`0x001` clear xbus / `0x002` set xbus / `0x004` clear freeze / `0x008` set freeze.
Setting freeze allows register writes without execution; clearing it resumes cleanly.
**[verified: used as the register-poking mechanism in "RDP START & END REG (masking)" and
"RSP STATUS: start-valid"]** Semantics of setting a set+clear pair at once: **[open]**.

### xbus mode

With XBUS set, START/END/CURRENT address RSP DMEM (0x000..0xFFF) instead of RDRAM. Command lists
run correctly from anywhere in DMEM, including ending exactly at 0x1000 and wrapping past it.
**[verified: "RDP STATUS: Run from DMEM (xbus)" and its end-of-dmem/overflow variants]**

### DP_CLOCK

- Free-running 24-bit counter; upper 8 bits read as zero. **[verified: "RSP Timing: Clock is just 24 bit"]**
- Read-only. **[verified: "RSP Timing: Clock must be readonly"]**
- Ticks at 4/3 the rate of the CPU COUNT register (COUNT = CPU clock / 2 = 46.875 MHz), i.e. the
  62.5 MHz RCP clock. FREEZE does not stop it. **[verified: "RSP Timing: Clock CPU vs RDP"]**

## 2. Command stream

Commands are sequences of 64-bit big-endian words. The opcode is bits 61..=56 of the first word.
Whether bits 63..=62 participate in decoding: **[open]**.

Command lengths in 64-bit words (including the first) **[provisional - lengths of the commands
exercised by tests are implicitly verified by the streams the differential tests run]**:

| Opcode | Name                     | Words |
|-------:|--------------------------|------:|
| 0x00   | No_Op                    | 1 |
| 0x08   | Fill_Triangle            | 4 |
| 0x09   | Fill_Z_Triangle          | 6 |
| 0x0A   | Texture_Triangle         | 12 |
| 0x0B   | Texture_Z_Triangle       | 14 |
| 0x0C   | Shade_Triangle           | 12 |
| 0x0D   | Shade_Z_Triangle         | 14 |
| 0x0E   | Shade_Texture_Triangle   | 20 |
| 0x0F   | Shade_Texture_Z_Triangle | 22 |
| 0x24   | Texture_Rectangle        | 2 |
| 0x25   | Texture_Rectangle_Flip   | 2 |
| 0x26   | Sync_Load                | 1 |
| 0x27   | Sync_Pipe                | 1 |
| 0x28   | Sync_Tile                | 1 |
| 0x29   | Sync_Full                | 1 |
| 0x2A   | Set_Key_GB               | 1 |
| 0x2B   | Set_Key_R                | 1 |
| 0x2C   | Set_Convert              | 1 |
| 0x2D   | Set_Scissor              | 1 |
| 0x2E   | Set_Prim_Depth           | 1 |
| 0x2F   | Set_Other_Modes          | 1 |
| 0x30   | Load_Tlut                | 1 |
| 0x32   | Set_Tile_Size            | 1 |
| 0x33   | Load_Block               | 1 |
| 0x34   | Load_Tile                | 1 |
| 0x35   | Set_Tile                 | 1 |
| 0x36   | Fill_Rectangle           | 1 |
| 0x37   | Set_Fill_Color           | 1 |
| 0x38   | Set_Fog_Color            | 1 |
| 0x39   | Set_Blend_Color          | 1 |
| 0x3A   | Set_Prim_Color           | 1 |
| 0x3B   | Set_Env_Color            | 1 |
| 0x3C   | Set_Combine              | 1 |
| 0x3D   | Set_Texture_Image        | 1 |
| 0x3E   | Set_Mask_Image           | 1 |
| 0x3F   | Set_Color_Image          | 1 |

Behavior of the unlisted opcodes (0x01..0x07, 0x10..0x23, 0x31): **[open]** - no-op, alias, or
hang?

## 3. State commands

### Set_Color_Image (0x3F)

| Bits    | Field |
|--------:|-------|
| 55..=53 | format (0 = RGBA) |
| 52..=51 | size: 0 = 4bpp, 1 = 8bpp, 2 = 16bpp, 3 = 32bpp |
| 41..=32 | width - 1 (whether bits 43..=42 are sampled: **[open]**) |
| 25..=0  | RDRAM physical address |

**[provisional - exercised with 16bpp/32bpp by every differential and triangle test]**

### Set_Scissor (0x2D)

| Bits    | Field |
|--------:|-------|
| 55..=44 | left, 10.2 unsigned |
| 43..=32 | top, 10.2 unsigned |
| 23..=12 | right, 10.2 unsigned |
| 11..=0  | bottom, 10.2 unsigned |

Field/odd-line bits (interlace support): **[open]**. Edge semantics are per-primitive; see the
FillRectangle and triangle sections.

### Set_Other_Modes (0x2F)

| Bits    | Field |
|--------:|-------|
| 53..=52 | cycle type: 0 = 1-cycle, 1 = 2-cycle, 2 = copy, 3 = fill |
| 31..=30 / 29..=28 | blender cycle 0 / 1 mux P |
| 27..=26 / 25..=24 | blender cycle 0 / 1 mux A |
| 23..=22 / 21..=20 | blender cycle 0 / 1 mux M |
| 19..=18 / 17..=16 | blender cycle 0 / 1 mux B |
| 9..=8   | coverage mode: 0 = clamp, 1 = wrap, 2 = zap, 3 = save |

All other bits: **[open]** (documented layouts exist but nothing here is verified yet).

### Set_Fill_Color (0x37)

Bits 31..=0: raw fill pattern. For 32bpp framebuffers it is one pixel; for 16bpp it holds two
pixels and the pixel written is selected by x parity: even x takes bits 31..=16, odd x takes bits
15..=0. **[verified: "SoftRDP: FillRectangle (fill mode, 16bpp)" - the differential test fills
with two different halves]**

### Set_Blend_Color (0x39)

Bits 31..=0: r, g, b, a bytes, high to low. **[provisional - used by the triangle tests through
the blender]**

## 4. Fill_Rectangle (0x36)

| Bits    | Field |
|--------:|-------|
| 55..=44 | xl (right), 10.2 unsigned |
| 43..=32 | yl (bottom), 10.2 unsigned |
| 23..=12 | xh (left), 10.2 unsigned |
| 11..=0  | yh (top), 10.2 unsigned |

Behavior in fill cycle type, 16bpp **[verified: "SoftRDP: FillRectangle (fill mode, 16bpp)"]**:

- The xl/yl pixel is included: a rectangle 0..=(width-1) with integer coordinates paints exactly
  `width` pixels (the classic fill/copy extra pixel relative to 1-/2-cycle mode).
- **Scissor right, horizontal**: xl is clamped to the scissor right at *subpixel* precision, and
  the pixel containing the clamped subpixel is still painted. A rectangle reaching to or past the
  scissor right therefore paints one pixel *at* `scissor_right >> 2` - one past what the scissor
  nominally allows. The write is address-based, so on a framebuffer whose width equals the scissor
  width it lands on the first pixel of the next row (and past the end of the framebuffer on its
  last row - real memory corruption).
- **Scissor bottom, vertical**: subpixel-exclusive - no row at or past the scissor bottom is
  painted. The asymmetry with the horizontal edge is real hardware behavior.
- Rows are painted top to bottom; within a row, writes are per-pixel 16-bit (no observable
  64-bit chunk rounding at either span edge).
- **Fractional coordinates**: on all four edges the pixel containing the subpixel coordinate is
  painted - left/top truncate down (`xh >> 2`, `yh >> 2`), right/bottom include their containing
  pixel (`xl >> 2`, `yl >> 2`). A right edge fractionally inside the scissor (e.g. 31.75 against a
  scissor right of 32.0) does not produce the scissor spill pixel.
- **Fractional scissor right/bottom** operate on subpixels: `x1 = min(xl, sc_right) >> 2`,
  `y1 = min(yl, sc_bottom - 1) >> 2`. A fractional scissor bottom (15.25/15.75) still paints its
  containing row, and one past the last full row (16.5) paints the row past the framebuffer -
  ruling out pixel-index clamping. **[verified: "SoftRDP: FillRectangle vs fractional scissor"]**
- **32bpp**: every pixel is written with the full 32-bit fill color; all edge/scissor rules above
  apply identically, and the scissor-right spill is a full 32-bit pixel write.
  **[verified: "SoftRDP: FillRectangle (fill mode, 32bpp)"]**
- **Fractional scissor left/top** truncate: `x0 = max(xh, sc_left applied as >> 2)`, i.e. a
  scissor left of 2.25 still paints pixel 2. This is NOT the triangle-style `(edge + 3) >> 2`
  partial-pixel exclusion. **[verified: "SoftRDP: FillRectangle vs fractional scissor"]**
- **Inverted rectangles** (xh > xl or yh > yl) paint nothing and do not hang the DP.
  **[verified: "SoftRDP: FillRectangle (fill mode, 16bpp)"]**

### Fill_Rectangle in 1-cycle mode

All **[verified: "SoftRDP: FillRectangle (1-cycle, 16bpp/32bpp)"]**, with the pipeline configured
as blender cycle 0 `P=BlendColor, B=Zero` (output = blend color) and coverage mode zap:

- **Geometry**: a pixel is painted iff its *top-left subpixel* lies inside `[xh, xl) x [yh, yl)`:
  left/top round up (`(edge + 3) >> 2` - a fractional xh/yh SKIPS its partially covered
  pixel/row), right/bottom paint their containing pixel (`(edge - 1) >> 2`). There is no
  fill-mode extra pixel, and no scissor spill pixel either - the scissor clips the subpixel
  ranges and the same rounding applies.
- **16bpp write**: r/g/b truncated to 5 bits, alpha bit = 1 (full coverage under zap).
- **32bpp write**: memory byte order is `r, g, b, coverage << 5` (0xE0 under zap) - this pins
  down both the 32bpp framebuffer layout and that the alpha byte carries coverage.
- Set_Blend_Color's word is `r, g, b, a` bytes high to low (r/g/b confirmed by the writes above).
- Inverted rectangles paint nothing here as well.

Still **[open]** for Fill_Rectangle:
- 4bpp/8bpp framebuffers.
- What fill mode writes into the hidden bits (coverage).
- Fill_Rectangle in 2-cycle/copy cycle types; 1-cycle beyond the blender/coverage configuration
  above (other blender muxes, clamp/wrap/save coverage, alpha compare, dither).

## 5. Triangles

Fill_Triangle (0x08) geometry in 1-cycle mode with coverage mode zap is now differentially
verified: **[verified: "SoftRDP: FillTriangle (1-cycle)" - 6 shapes x 16/32bpp, including
right/left major, negative x, past-scissor, fractional x and y edges]**.

Edge walker (word layout: yl/ym/yh as 12.2 signed; xl/xm/xh and their per-scanline steps dl/dm/dh
as 16.16 signed):

- The major edge starts at xh and walks from yh stepping dh once per *subpixel* line; the minor
  side steps xm/dm until ym, then restarts at xl stepping dl until yl. `right_major` (bit 55 of
  word 0) selects which side is left/right.
- **Which pixels are painted (under zap)**: the same top-left-subpixel rule as the 1-cycle
  rectangle. Only each pixel row's top subpixel line (y % 4 == 0) determines painted pixels, and
  on it a pixel is painted iff its first subpixel column is inside [left, right - 2.0 subpixel
  units]: the left edge rounds up to the next whole pixel, the right edge paints its containing
  pixel, a fractional yh skips its partially covered top row, yl paints its containing row.
- Scissor top and left clip at pixel granularity (`(edge + 3) >> 2`); scissor right and bottom
  clip at subpixel granularity.
- **Coverage (clamp mode)** is now differentially verified
  **[verified: "SoftRDP: FillTriangle (1-cycle, clamp coverage, 32bpp)"]**:
  - WHICH pixels are painted is the same top-left-subpixel rule as zap - a pixel with high
    partial coverage on the left edge still stays unpainted. Coverage only affects the written
    alpha.
  - The coverage samples form a CHECKERBOARD: of the 16 subpixels per pixel, only the 8 with an
    even (sx ^ sy) count. The written alpha is the sample sum in coverage-minus-one encoding:
    alpha = (sum - 1) << 5, so full coverage = 0xE0 and the finest partial = 0x00.
  - Along a span line, a subpixel column is covered iff it lies at or right of the left edge
    (subpixel-precision round-up) and at or left of right - 2.0 subpixel units.
  - The MAJOR edge samples one dh step ahead of the minor edge (observed via a major edge
    crossing exactly onto a subpixel boundary mid-triangle; a constant or minor edge at the same
    position keeps the sample).
  - A fractional yh skips its partial top pixel row in clamp mode too.
  - The old partially-known clamp table from the pre-differential tests (8..10 -> 0x60 etc.) was
    partly wrong - measured in the miscompiled-i64 era; the checkerboard model replaces it.
  - **Wrap mode is identical to clamp** in this configuration - with no memory coverage source
    (image read disabled) there is nothing to wrap against.
    **[verified: same test, coverage mode 1]**
  - **Save mode writes full coverage (0xE0)** regardless of the framebuffer's previous alpha:
    save stores the MEMORY coverage back, and with image read disabled that reads as full.
    **[verified: same test, coverage mode 3]** The read-modify path behind the image-read enable
    bit is **[open]**.
  - Still **[open]**: fractional yl bottom rows (which sample rows count), right edges exactly on
    subpixel boundaries, the image-read enable bit (real coverage blending), 16bpp coverage
    (alpha bit + hidden bits).

### Shade triangles (0x0C)

Verified in 1-cycle mode through a shade-passthrough combiner on static-edge geometry
**[verified: "SoftRDP: ShadeTriangle (1-cycle, 32bpp)"]**:

- **Set_Combine encoding**: the community bit layout is correct as far as exercised - the
  passthrough configuration (RGB: (zero - zero) * zero + SHADE, alpha likewise) behaves exactly
  as encoded. See `COMBINE_PASSTHROUGH_SHADE` in rdp-core.
- **Shade coefficient words** (after the 4 edge words): RGBA integer parts (4 x s16, r g b a
  high to low), DcDx int, RGBA fractions, DcDx frac, DcDe int, DcDy int, DcDe frac, DcDy frac.
  Channels are s15.16.
- **DcDe** steps once per pixel row, counted from yh's pixel row (a skipped partial top row
  still counts).
- **DcDx** anchors at the exact fractional left edge position: pixel color =
  base + de_steps * DcDe + (px * 4 - left) * DcDx/4 evaluated in subpixels at 16.16 precision.
- Written bytes are the integer parts (truncated); the 32bpp alpha byte still carries coverage.

**[open] Sloped/fractional left edges add a sub-subpixel sampling phase** that is not yet
understood. Probe data (offset added to the sampling position of pixels with (px ^ py) even, in
subpixels; other pixels stay exact; fitted per row):

| left edge at row | dh/row = 0.5 | 0.25 | 0.75 | const |
|---|---|---|---|---|
| 12.0 | 0 | 0 | 0 | |
| 12.25 | | unfit | | |
| 12.5 | 0.5 | 0.5 | | |
| 12.75 | | unfit | 0.75 | |
| 13.0 | 1 | 1 | | 1 (all rows) |
| 13.25 | | 1.25 | | |
| 13.5 | **0** | **1.5** | **1.5** | |
| 14.25 | | | 0.25 uniform (both parities) | |
| 15.75 | | | unfit | |

The phase mostly equals (left mod 2), but identical left values yield different phases under
different slopes (13.5 above), some rows fit no per-parity quarter-subpixel offset at all, and
one row shifted BOTH parities.

A second probe series (5 slopes: 0.25/0.5/0.75/1.0/2.0 subpixels per row, 10 rows, phase fitted
at EIGHTH-subpixel resolution independently per parity) fit almost nothing - nearly every row
came back unfit. Only the two steepest slopes fit, and they fit the PLAIN LINEAR model (phase 0,
both parities): dh = 2.0/row on rows 7..11, dh = 1.0/row on row 11. So the phase is not a
per-parity sub-subpixel sampling offset at all; refining the resolution made the fits worse, not
better. Whatever the mechanism is, it vanishes once the edge moves fast enough, which hints at
something latched or carried between scanlines rather than computed from the edge position.

Until this is understood the soft-RDP only claims shade on static-edge spans.

Everything else about triangles (texture, z), and the combiner/blender in general: **[open]**.

## 6. TMEM and texturing

TMEM is not CPU-visible, so everything here is measured through a readback: load into TMEM, then
draw a Texture_Rectangle in COPY cycle type into a 16bpp framebuffer. Copy mode writes texels
verbatim with no combiner or blender involved, which keeps a TMEM measurement from depending on
either. **[verified: "SoftRDP: LoadTile + TextureRectangle (RGBA 16bpp, copy mode)"]**

### Load_Tile (0x34), RGBA 16bpp, single row

- Copies the rectangle [sl, sh] x [tl, th] (the 10.2 fields carry whole texels) from the texture
  image into the tile's TMEM range: source rows at the texture image's own width, destination
  rows `line` 64-bit words apart starting at the tile's `tmem` word. Bytes are copied verbatim -
  no swizzling for a single row.
- Set_Texture_Image and Set_Tile take format/size in the same bit positions as Set_Color_Image.

### Texture_Rectangle (0x24) in copy mode

Copy mode processes **four pixels per cycle**, which shows up in two distinct ways - both were
found by hardware disagreeing with the naive model:

- **The painted span is rounded up to a whole group of four**, measured from its start. A
  rectangle covering pixels 2..=10 paints 2..=13. Found with a uniform texture, where hardware
  painting past the rectangle could not be mistaken for a sampling difference.
- **dsdx advances S once per GROUP, not per pixel**; inside a group each pixel takes the next
  texel. With dsdx = 1.0 the sampled texels run 0,1,2,3, 1,2,3,4, 2,3,4,5 ... - so a 1:1 copy
  needs **dsdx = 4.0**. Found by decoding the texel sequence from a coordinate-encoded texture.
- The screen rectangle follows the fill-mode rule (the xl/yl pixel is included).

### Still open

- Multi-row loads (the odd-line swizzling texture sampling is believed to involve), Load_Block,
  Load_Tlut, tile masks/shifts/clamp/mirror.
- What TMEM holds beyond a load - the group rounding samples past the loaded texels, and hardware
  returned data there where an empty model returns zero.
- **RGBA 32bpp**: an earlier attempt showed a uniform 32bpp texture producing a varying result,
  meaning hardware samples TMEM the load never wrote. The likely explanation is that 32bpp texels
  are split across TMEM (red/green low, blue/alpha high) with a matching load path. 32bpp is the
  wrong format to start from; it should be revisited now that the 16bpp path is pinned down.
- Formats other than RGBA, 4bpp/CI, filtering, and the 1-cycle texture path (which needs the
  texel-passthrough combiner encoding verified separately).
