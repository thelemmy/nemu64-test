# Draft: LLVM issue to file

Not filed yet. Target: <https://github.com/llvm/llvm-project/issues/new> with labels
`backend:MIPS`, `miscompilation`. Reproducer: `docs/repro/mips-sll64_64.ll`.

---

## Title

`[MIPS] Wrong code: SLL64_64 is marked isMoveReg, so register allocation drops the 32-bit truncation`

## Body

### Summary

On `mips64`, a sign-extend-from-32 (`sext_inreg ... to i32`, i.e. `shl i64 %x, 32` +
`ashr exact i64 ..., 32`) can be silently dropped, leaving the untruncated 64-bit value in place.

The cause looks like this definition in `llvm/lib/Target/Mips/Mips64InstrInfo.td` (present on
`main`):

```tablegen
let isCodeGenOnly = 1, rs = 0, shamt = 0 in {
  def DSLL64_32 : FR<0x00, 0x3c, (outs GPR64:$rd), (ins GPR32:$rt), "dsll\t$rd, $rt, 32", []>, GPR_64;
  let isMoveReg = 1 in {
    def SLL64_32 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR32:$rt), "sll\t$rd, $rt, 0", []>, GPR_64;
    def SLL64_64 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR64:$rt), "sll\t$rd, $rt, 0", []>, GPR_64;
  }
}
```

`SLL64_64` takes a **GPR64** source and emits `sll rd, rt, 0`, which on MIPS64 truncates to 32 bits
and sign-extends. For an arbitrary 64-bit source that is not a register move: if the high 32 bits
of `$rt` are not the sign extension of bit 31, the result differs from the input. Marking it
`isMoveReg` lets the register allocator treat it as a redundant copy and delete it once source and
destination share a physical register.

`SLL64_32` looks fine to keep the flag - its source is a `GPR32`, and the backend maintains the
invariant that 32-bit values are sign-extended in their 64-bit registers, so there the instruction
really is a copy. Only the `GPR64` variant seems wrong, but maintainers should confirm which of the
two (or which consumer of `isMoveReg`) is the right thing to change.

### Reproducer

```
llc -mtriple=mips64-unknown-unknown -mcpu=mips3 -O3 mips-sll64_64.ll -o out.s
```

The attached file is a reduced-but-still-large (~650 line) function; the bug needs enough register
pressure that the allocator coalesces the `SLL64_64` operands, so it has resisted further
reduction. Small hand-written cases do not reproduce, because when the low half of a 64-bit value
is only needed as 32 bits, ISel folds the truncation into an `lw` and no `SLL64_64` is created at
all. The bug needs the *same* 64-bit value to be used whole **and** truncated.

To observe it directly:

```
llc -mtriple=mips64-unknown-unknown -mcpu=mips3 -O3 -print-after-all \
    -filter-print-funcs=<function> mips-sll64_64.ll -o /dev/null 2>&1 \
  | grep -c SLL64_64   # per section
```

Four `SLL64_64` are selected; after **Greedy Register Allocator** only one remains:

```
after machine-scheduler:              after greedy:
%36:gpr64  = SLL64_64 %35:gpr64       %968:gpr64 = SLL64_64 %35:gpr64
%48:gpr64  = SLL64_64 %607:gpr64      (deleted)
%55:gpr64  = SLL64_64 %33:gpr64       (deleted)
%103:gpr64 = SLL64_64 %33:gpr64       (deleted)
```

The deleted ones feed loop-carried adds:

```
%33:gpr64  = LD %17:gpr64, 8              ; a raw 64-bit load
%55:gpr64  = SLL64_64 %33:gpr64           ; sext-from-32 of it
%34:gpr64  = DADDu %34:gpr64, %55:gpr64   ; accumulator += that
```

and the resulting assembly spills and re-adds the **raw** loaded word, with no `sll` anywhere:

```
ld    $9, 0x18($3)      ; raw 64-bit word
sd    $9, 0xa8($sp)     ; spilled raw (this slot is written exactly once)
...
ld    $1, 0xa8($sp)
daddu $17, $1, $17      ; accumulator += raw word    <-- truncation gone
```

### Impact

This is silent wrong code, not a crash. It was found on real hardware: a Rust program computing
`some_u64 as u32 as i32 as i64` and using the result as a loop increment accumulated the whole
64-bit word instead of its sign-extended low half, so a rasterizer produced wrong pixels. Values
came back as e.g. `0x0004_0000_0004_0000` where `0x0004_0000` was expected - exactly
`correct_value + 4 * raw_word` after four loop iterations.

The frontend is not involved: the LLVM IR is the canonical form,

```llvm
%a = shl i64 %x, 32
%b = ashr exact i64 %a, 32
```

and `llc` alone reproduces it from the attached `.ll`.

### Environment

- LLVM 22.1.8 (as shipped with a recent Rust nightly); the `.td` definition is unchanged on `main`.
- Triple `mips64-unknown-unknown`, `-mcpu=mips3`, `-O3`. Also reproduces at the equivalent
  optimization level through `rustc` for a bare-metal `mips64` target with `n64` ABI.
