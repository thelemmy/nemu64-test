# i64 values get corrupted under register pressure (MIPS codegen)

Status: **reproducible, characterized, not yet minimized.** Written down so the next person does
not have to rediscover the dead ends.

## Symptom

Code doing plain `i64` arithmetic produces wrong results on console while the identical code is
correct on a host (x86_64/aarch64). No exception is raised - the values are simply wrong. The
result is deterministic for a given build but changes when unrelated code shifts around, which
makes it look like flakiness.

It was found in the software RDP triangle rasterizer (branch `rdp/soft-rdp`, `rdp-core`), where
6 differential tests failed against real hardware. Rewriting the same algorithm with `i32`
accumulators made all of them pass.

## What is actually corrupted

Instrumenting the failing function to compute the span bounds twice - once in `i32`, once in
`i64` - and dumping the first disagreement gave:

| value | observed (i64) | expected |
|-------|----------------|----------|
| `left`     | `0x0004_0000_0004_0000` | `0x0004_0000` |
| `right`    | `0x0014_0000_0014_0000` | `0x0014_0000` |
| `px_start` | `0x0000_0001_0000_0001` | `0x0000_0001` |
| `right` (other case) | `0x0003_0000_0008_0000` | `0x0008_0000` |
| `left` (other case)  | `0x000f_ffff_ff80_000`  | `0xffff_ffff_fff8_0000` |

The pattern: **the low 32 bits are correct and the high 32 bits hold a stray 32-bit value**, very
often a copy of the low word. Negative values come back without their sign extension.

So the arithmetic *expressions* are not what breaks - the *values* are already wrong when they
reach them. This is the signature of a 32-bit result being consumed as 64-bit without the
sign-extension MIPS64 requires, or of a 64-bit spill/reload losing half its content.

Downstream this is very visible: a garbage-large `right` makes `min(right, scissor)` pick the
scissor, so a span that should end at pixel 4 ends at pixel 31.

## Reproduction

```
git checkout rdp/soft-rdp
# in rdp-core/src/raster.rs, one_cycle_fill_triangle: change the six edge variables
# (xl, dl, xh, dh, xm, dm) and the two span-bound expressions from i32 back to i64
cargo run --release          # writes the .z64
# run it on hardware: "SoftRDP: FillTriangle (1-cycle)" fails 6 of 12 cases
```

Reverting just those types makes it pass again. Nothing else changes.

## What has been ruled out

- **Not i64-as-register-pairs.** The target is MIPS III with 64-bit registers; LLVM emits native
  `daddu`/`dsra`/`sd`. There is no pair-splitting to get wrong.
- **Not a bad expression lowering.** A standalone crate containing the same loop compiles to
  correct assembly (checked by reading it), and the same loop as a ROM test passes on hardware -
  see `I64EdgeWalk` and `I64WalkAndPaint` in `src/tests/arithmetic/i64_codegen.rs`. Both are
  faithful structural copies, including the trait-dispatched memory writes, and both are green.
- **Not stack misalignment.** `sd`/`ld` need an 8 byte aligned stack; it is aligned and a
  round-trip works - see `StackPointerIs8ByteAligned`.
- **Not fat LTO.** It reproduces with `lto = "fat"` and with `lto = false`.
- **Not the RDP.** The software rasterizer runs on a heap buffer before the DP is started at all.

## What is still open

The failure only appears when the rasterizer is inlined into `SoftRdp::run` (~6 KB of machine
code, heavy register pressure, 64-bit values spilled to the stack - the disassembly shows
`ld $1, 0xc0($sp)` feeding exactly the corrupted computation). Every attempt to shrink it below
that pressure threshold makes it disappear, which is why there is no small repro yet.

Next steps, in the order that seems most promising:

1. Bisect inside the real function: flip the edge variables to `i64` one at a time and find the
   smallest set that still corrupts.
2. Dump the disassembly of the failing `SoftRdp::run` and trace the corrupted value backwards
   from its use to the instruction that defines it (the shape of the corruption suggests looking
   for a 32-bit-defining instruction whose result is later read as 64-bit).
3. Once the defining instruction is known, reproduce it with `-C llvm-args` tweaks or a smaller
   function that forces the same allocation, then report upstream.

Until then, **prefer `i32` in hot code on this target**, and hardware-verify anything that has to
use `i64`. The suite's existing `u64` use (register values, command words) is not obviously
affected - it tends to be load/store and bit manipulation rather than long-lived accumulators in
high-pressure loops.

## Possible second victim

`src/tests/rdp/filled_triangle.rs`'s `render_on_cpu` uses the same style of `i64` edge walking and
is currently feature-gated (`experimental_rdp`) for being "unstable on hardware". That instability
may well be this bug rather than anything about the RDP. Worth retesting with `i32` accumulators.
