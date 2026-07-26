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

Still **[open]** for Fill_Rectangle:

- Fractional scissor *left/top* values (does the triangle-style `(edge + 3) >> 2` exclusion apply,
  or plain truncation?).
- 32bpp framebuffers (including whether the scissor-right spill exists there), 4bpp/8bpp.
- What fill mode writes into the hidden bits (coverage).
- Fill_Rectangle in 1-cycle/2-cycle/copy cycle types.
- xh > xl / yh > yl (inverted rectangles).

## 5. Triangles

Knowledge from the Fill_Triangle (0x08) tests in 1-cycle mode. These ran green on hardware but the
harness is currently feature-gated (`experimental_rdp`) due to instability; treat as
**[provisional]** until re-validated by differential tests.

Edge walker (word layout: yl/ym/yh as 12.2 signed; xl/xm/xh and their per-scanline steps dl/dm/dh
as 16.16 signed):

- The major edge walks from yh with xh/dh; the minor side walks xm/dm until ym, then xl/dl until
  yl. `right_major` (bit 55 of word 0) selects which side is left/right.
- Edges step once per *subpixel* line (quarter-pixel), accumulating coverage out of 16 subpixel
  samples per pixel.
- Scissor top and left clip at pixel granularity (`(edge + 3) >> 2`); scissor right and bottom
  clip at subpixel granularity.
- Coverage-to-alpha mapping (coverage mode clamp) is only partially mapped: of the 16 possible
  subpixel counts, 4->0x20, 7->0x40, 8..10->0x60, 12->0xA0, 16->0xE0 are known. **[open: the
  remaining counts]**
- Coverage modes zap (alpha = 0xE0 unconditionally) and the blender configuration
  `(A=CombineAlpha, P=BlendColor, B=Zero, M=MemoryColor)` behaved as the tests model.

Everything else about triangles (shade, texture, z), and all of TMEM, combiner, blender in
general: **[open]**.

## 6. RDRAM hidden bits

RDRAM is 9 bits per byte; the RDP (and VI) see the 9th bits, the CPU cannot read them. For 16-bit
framebuffers they extend each pixel's 1-bit alpha to a 3-bit coverage value; for 16-bit z-buffers
they extend dz. rdp-core models them as 2 bits per 16-bit word via its `Rdram` trait.

- Whether/how CPU writes clobber the hidden bits, and what value they take: **[open]** (testable
  via an RDP readback pass that routes memory coverage into visible bits).
- What fill mode and 1-cycle writes put there: **[open]**.

## 7. Sync commands

Sync_Full is required at the end of a list for the status bits to settle back to idle
(`COMMAND_BUFFER_READY`), and is what the tests use to detect completion.
**[verified: "RDP STATUS: Flags during a run"]** Whether Sync_Load/Pipe/Tile have any effect
observable in memory results (as opposed to timing/hazards): **[open]** - rdp-core treats them as
no-ops.
