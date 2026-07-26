# LLVM MIPS backend drops 32-bit truncations (`SLL64_64` is marked `isMoveReg`)

Status: **root cause found in upstream LLVM.** Not yet reported.

## One-line summary

`llvm/lib/Target/Mips/Mips64InstrInfo.td` marks `SLL64_64` as `isMoveReg = 1`, but that
instruction is `sll rd, rt, 0` with a **64-bit** source, which truncates to 32 bits and
sign-extends. The register allocator believes it is a plain copy and deletes it, so the
untruncated 64-bit value is used instead.

Any Rust expression of the form `some_u64 as u32 as i32 as i64` (or the equivalent
`((x as i64) << 32) >> 32`) can therefore silently keep its high bits on this target.

## Symptom

Plain `i64` arithmetic produced wrong results on console while the identical code was correct on a
host. No exception, deterministic per build, but moving unrelated code around changed it - so it
looked like flakiness. Found in the software RDP triangle rasterizer (`rdp-core`, branch
`rdp/soft-rdp`): six differential tests failed against real hardware, and rewriting the same
algorithm with `i32` accumulators made all of them pass.

Values came back like this:

| value | observed | expected |
|-------|----------|----------|
| `left`  | `0x0004_0000_0004_0000` | `0x0004_0000` |
| `right` | `0x0014_0000_0014_0000` | `0x0014_0000` |

which is exactly `correct_initial_value + 4 * raw_command_word` - the loop had stepped its
accumulators by the whole 64-bit command word instead of by the sign-extended low half, four
times, before the first mismatch was recorded.

## Evidence chain

The source computes the step as a truncation of a 64-bit command word:

```rust
let dm = cmd[3] as u32 as i32 as i64;   // truncate to 32 bits, sign-extend
...
minor_x += dm;
```

**1. The LLVM IR is correct** (`--emit=llvm-ir`):

```llvm
%_44.i.i = shl i64 %_41.i.i, 32
%dh.i.i  = ashr exact i64 %_44.i.i, 32     ; canonical sign-extend-from-32
```

So this is not a rustc frontend problem.

**2. The instruction survives selection and coalescing, then dies in register allocation.**
Dumping MIR after every backend pass (`-Cllvm-args=-print-after-all` with
`-Cllvm-args=-filter-print-funcs=<mangled fn>`) and counting `SLL64_64`:

```
after machine-scheduler:              after greedy (Greedy Register Allocator):
%36:gpr64  = SLL64_64 %35:gpr64       %968:gpr64 = SLL64_64 %35:gpr64
%48:gpr64  = SLL64_64 %607:gpr64      (deleted)
%55:gpr64  = SLL64_64 %33:gpr64       (deleted)
%103:gpr64 = SLL64_64 %33:gpr64       (deleted)
```

The deleted ones are exactly the loop steps:

```
%33:gpr64  = LD %17:gpr64, 8       ; raw cmd[1]
%607:gpr64 = LD %17:gpr64, 24      ; raw cmd[3]
%55:gpr64  = SLL64_64 %33:gpr64    ; dl  = sext32(cmd[1])
%48:gpr64  = SLL64_64 %607:gpr64   ; dm  = sext32(cmd[3])
%34:gpr64  = DADDu %34:gpr64, %55:gpr64    ; minor_x += dl
%81:gpr64  = DADDu %81:gpr64, %48:gpr64    ; minor_x += dm
```

**3. The final machine code adds the raw word**, with the raw `LD` result spilled and reloaded
straight into the accumulate:

```
800cb59c:  ld    $9, 0x18($3)      ; $9 = cmd[3], raw
800cb5cc:  sd    $9, 0xa8($sp)     ; spill the RAW word (slot written exactly once)
...loop:
800cb6cc:  ld    $1, 0xa8($sp)
800cb6e0:  daddu $17, $1, $17      ; left += raw cmd[3]   <-- truncation gone
```

**4. Upstream cause.** In `llvm/lib/Target/Mips/Mips64InstrInfo.td` (still present on `main`):

```tablegen
let isCodeGenOnly = 1, rs = 0, shamt = 0 in {
  def DSLL64_32 : FR<0x00, 0x3c, (outs GPR64:$rd), (ins GPR32:$rt), "dsll\t$rd, $rt, 32", []>, GPR_64;
  let isMoveReg = 1 in {
    def SLL64_32 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR32:$rt), "sll\t$rd, $rt, 0", []>, GPR_64;
    def SLL64_64 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR64:$rt), "sll\t$rd, $rt, 0", []>, GPR_64;
  }
}
```

`isMoveReg = 1` is fine for `SLL64_32`: its source is a `GPR32`, and the backend keeps 32-bit
values sign-extended in 64-bit registers, so `sll rd, rt, 0` really is a copy there. It is **not**
fine for `SLL64_64`, whose source is an arbitrary `GPR64` - the truncation is the entire point of
the instruction. Marking it a move lets the allocator drop it once source and destination share a
physical register.

## Why the reductions all passed

Small repros kept coming out correct because the truncation was folded into the load: when the low
half is only needed as a 32-bit value, ISel emits `lw $3, 12($4)`, which sign-extends for free and
never creates an `SLL64_64` at all. The bug needs the *same* 64-bit word to be loaded whole (for
the high half) **and** truncated for the low half, plus enough register pressure for the allocator
to coalesce the `SLL64_64` source and destination. `src/tests/arithmetic/i64_codegen.rs` keeps
those attempts as regression tests; they pass, and that is expected.

## Consequences for this repo

- `rdp-core`'s rasterizer uses `i32` edge math. That is the right shape anyway (it matches the
  hardware's edge register widths and vectorizes better), and it sidesteps this entirely.
- Anywhere a `u64` is truncated to 32 bits and used as a long-lived value in a hot loop is
  suspect. Loading the half directly (an `lw`-shaped access, e.g. reading a `[u32; 2]` or using
  `u32::from_be_bytes`) avoids the pattern, because no `SLL64_64` is created.
- `src/tests/rdp/filled_triangle.rs`'s `render_on_cpu` walks edges in `i64` and is feature-gated
  (`experimental_rdp`) for being "unstable on hardware". That is very likely this bug, not the
  RDP. Worth retrying with `i32`.

## Reproducing

```
git checkout rdp/soft-rdp
# rdp-core/src/raster.rs, one_cycle_fill_triangle: change xl/dl/xh/dh/xm/dm and the two span-bound
# expressions from i32 to i64
cargo run --release
# on hardware: "SoftRDP: FillTriangle (1-cycle)" fails 6 of 12 cases
```

To watch the instruction disappear:

```
RUSTFLAGS="-Cllvm-args=-print-after-all -Cllvm-args=-filter-print-funcs=<mangled SoftRdp::run>" \
  cargo rustc --release -- --emit=asm 2> passes.txt
# count SLL64_64 in the section after machine-scheduler (4) and after greedy (1)
```

## Upstream report

Drafted in `docs/llvm-issue-draft.md`, with a self-contained reproducer in
`docs/repro/mips-sll64_64.ll` (~650 lines, verified to reproduce under plain `llc`). Not filed
yet.
