# i64 values get corrupted under register pressure

Status: **reproducible and characterized; cause NOT established.** Written down so the next person
does not have to rediscover the dead ends.

Do not read this as a compiler bug report - that is only one of the two live hypotheses, and the
experiment that separates them has not been run yet (see "Which is it?" below).

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
- **Not the exception handler clobbering registers.** It saves and restores with `sd`/`ld`, and
  `Status::DEFAULT` leaves interrupts disabled, so nothing runs between the corrupted value's
  definition and its use.
- **Not a semantic difference between the two spellings.** The observed values are arithmetically
  unreachable by the expressions that produced them, and a direct i64-vs-i32 comparison of the
  same inputs agrees everywhere outside the high-pressure context.
- **Not stack misalignment.** `sd`/`ld` need an 8 byte aligned stack; it is aligned and a
  round-trip works - see `StackPointerIs8ByteAligned`.
- **Not fat LTO.** It reproduces with `lto = "fat"` and with `lto = false`.
- **Not the RDP.** The software rasterizer runs on a heap buffer before the DP is started at all.

## Which is it? Two hypotheses, one experiment

`rdp-core` is 595 lines of safe Rust with zero `unsafe`, and the observed value is arithmetically
unreachable by the expression that produced it (`((x as i32) as i64) << 2` cannot exceed ~2^33,
yet `0x0004_0000_0004_0000` came back). Safe Rust producing an impossible value leaves two
candidates:

1. **The toolchain emits wrong machine code.**
2. **The VR4300 executes correct machine code incorrectly** - a hardware erratum affecting 64-bit
   operations under some pipeline or scheduling condition.

Both explain everything observed: host correctness, layout sensitivity, the pressure threshold,
and the fact that reductions go green. They differ in one cheap, decisive way - the machine code
is byte-identical either way, so:

| | in an emulator | on hardware |
|---|---|---|
| toolchain bug | fails identically | fails |
| VR4300 erratum | **passes** | fails |

**Run the failing ROM in an emulator.** That single result eliminates one hypothesis. Until it has
been run, neither wording ("miscompile", "errata") is justified.

One weak signal already leans towards hardware: the disassembly of the span-bound computation was
read and is correct MIPS III. That is not conclusive - the corrupted value's *definition* has not
been traced, only its use.

Next steps after the emulator run:

- If the emulator also fails: trace the corrupted value backwards in the disassembly of
  `SoftRdp::run` to the instruction defining it, then reduce towards an upstream report.
- If the emulator passes: this is a hardware finding, and it belongs in the test suite as a real
  VR4300 quirk test. Narrow down which instruction sequence triggers it and under what pipeline
  conditions, the same way the other quirk tests in this suite were built.

Until then, **prefer `i32` in hot code on this target**, and hardware-verify anything that has to
use `i64`. The suite's existing `u64` use (register values, command words) is not obviously
affected - it tends to be load/store and bit manipulation rather than long-lived accumulators in
high-pressure loops.

## Possible second victim

`src/tests/rdp/filled_triangle.rs`'s `render_on_cpu` uses the same style of `i64` edge walking and
is currently feature-gated (`experimental_rdp`) for being "unstable on hardware". That instability
may well be this bug rather than anything about the RDP. Worth retesting with `i32` accumulators.
