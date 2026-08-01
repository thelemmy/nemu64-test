# Draft: LLVM issue (up to date, all claims verified)

Target: <https://github.com/llvm/llvm-project/issues/new>, labels `backend:MIPS`,
`miscompilation`. Attach `docs/repro/mips-sll64_64.ll`.

Everything below was verified experimentally on upstream `main`
(`17efc66a340e35ae03a18e34e7f267832fff7940`, LLVM 24.0.0git) with a local
`-DLLVM_TARGETS_TO_BUILD=Mips` release build; nothing is inferred-only.

---

## Title

`[MIPS] Miscompile: SLL64_64 (sext of GPR64) is treated as a register copy and deleted by the register allocator`

## Body

### Summary

On mips64, the sign-extend-from-32 instruction `SLL64_64` (`sll $rd, $rt, 0` with a **GPR64**
source) can be deleted during register allocation, so the untruncated 64-bit value flows into its
uses. Any IR of the shape

```llvm
%lo = shl i64 %x, 32
%s  = ashr exact i64 %lo, 32     ; canonical sign-extend-from-32
```

can silently lose the truncation when it survives to a `SLL64_64` and register pressure makes the
allocator coalesce its operands.

The chain appears to be:

- `llvm/lib/Target/Mips/Mips64InstrInfo.td` marks `SLL64_64` with `isMoveReg = 1` (shared `let`
  block with `SLL64_32`);
- `MipsSEInstrInfo::isCopyInstrImpl` (MipsSEInstrInfo.cpp:296) returns a `DestSourcePair` for any
  `isMoveReg()` instruction, so generic `isCopyInstr()` users believe destination and source hold
  the same value;
- during the Greedy Register Allocator the instruction is then eliminated as a redundant copy.

That reasoning is sound for `SLL64_32` (GPR32 source, which the backend keeps sign-extended in its
64-bit register, so the instruction really is value-preserving) but not for `SLL64_64`, whose
entire purpose is to discard the upper 32 bits of an arbitrary GPR64.

### Reproducer

```
llc -mtriple=mips64-unknown-unknown -mcpu=mips3 -O3 mips-sll64_64.ll -o out.s
```

The attached `.ll` is ~650 lines. Apologies for the size: the bug needs (a) the *same* 64-bit
value to be used both whole and truncated - otherwise ISel folds the truncation into an `lw` and
no `SLL64_64` exists - and (b) enough register pressure that the allocator coalesces the
`SLL64_64` operands. Every attempt to shrink below this (hand-written IR with 4-20 accumulators,
extracted loops, MIR line reduction) makes the deletion stop happening, so the file is close to
minimal *for this failure mode* even though it is large.

Watching the instruction disappear (counts of `SLL64_64` per dump):

```
llc -mtriple=mips64-unknown-unknown -mcpu=mips3 -O3 -print-after-all \
    -filter-print-funcs=<the function> mips-sll64_64.ll -o /dev/null

after mips-isel:                4
after greedy:                   2     <- two deleted
```

It can also be isolated to the one pass:

```
llc -mtriple=mips64 -mcpu=mips3 -O3 -stop-before=greedy mips-sll64_64.ll -o before.mir
llc -mtriple=mips64 -mcpu=mips3 -run-pass=greedy before.mir -o after.mir
# grep -c SLL64_64: before=4, after=2
```

The deleted instructions are loop-step values derived from 64-bit loads:

```
%33:gpr64  = LD %17:gpr64, 8               ; raw 64-bit word
%55:gpr64  = SLL64_64 %33:gpr64            ; sext-from-32 of it        <- deleted
%34:gpr64  = DADDu %34:gpr64, %55:gpr64    ; loop-carried accumulate
```

and the final assembly spills and re-adds the **raw** word with no `sll` anywhere:

```
ld    $9, 0x18($3)       ; raw 64-bit word
sd    $9, 0xa8($sp)      ; spilled raw (slot written exactly once)
...loop:
ld    $1, 0xa8($sp)
daddu $17, $1, $17       ; accumulator += raw word - truncation gone
```

### How this was found

A Rust program (an N64 test ROM; bare-metal mips64, n64 ABI, cpu mips3) computed loop increments
as `word as u32 as i32 as i64` and accumulated `initial + N * raw_64bit_word` instead - observed
as wrong pixels from a software rasterizer on real hardware, with values like
`0x0004_0000_0004_0000` where `0x0004_0000` was expected. The emitted LLVM IR is the correct
canonical sext-from-32; `llc` alone reproduces the wrong code from it. First seen on the LLVM
22.1.8 shipped with rustc nightly; confirmed unchanged on `main` as above.

### Verified fix (one candidate)

Moving `SLL64_64` out of the `isMoveReg` block fixes it:

```diff
-  let isMoveReg = 1 in {
-    def SLL64_32 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR32:$rt),
-                      "sll\t$rd, $rt, 0", []>, GPR_64;
-    def SLL64_64 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR64:$rt),
-                      "sll\t$rd, $rt, 0", []>, GPR_64;
-  }
+  let isMoveReg = 1 in
+  def SLL64_32 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR32:$rt),
+                    "sll\t$rd, $rt, 0", []>, GPR_64;
+  // Not isMoveReg: the GPR64 source means this truncates to 32 bits and
+  // sign-extends, which is not value-preserving.
+  def SLL64_64 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR64:$rt),
+                    "sll\t$rd, $rt, 0", []>, GPR_64;
```

With this patch, on the same build:

- all 4 `SLL64_64` survive register allocation (both in the full pipeline and under
  `-run-pass=greedy`);
- the resulting assembly has no remaining "reload straight into `daddu` with no sign-extension"
  site (baseline had 2);
- `check-llvm-codegen-mips`: 1024 passed, 4 expectedly failed, **1 failed**:
  `CodeGen/Mips/madd-msub.ll`, and only because the expected register assignment changes -
  `sll $4, $4, 0` becomes `sll $1, $4, 0`. The sign-extension is still emitted; what is lost is
  the operand coalescing that `isCopyInstr` hinting used to provide. So the test just needs its
  CHECK lines regenerated, at the cost of one extra live register in such functions.

### Open question for maintainers

`isMoveReg` on `SLL64_64` currently produces both a valid effect (coalescing hints that put
source and destination in the same register) and an invalid one (the instruction is deleted and
its uses see the untruncated source). The `.td` change above removes both. If it is preferable to
keep the hinting, the alternative is to stop `MipsSEInstrInfo::isCopyInstrImpl` (or the specific
`isCopyInstr` consumer that performs the elimination) from claiming `SLL64_64`'s destination
equals its source - the instruction is only a copy when the source is already known sign-extended,
which `isCopyInstrImpl` cannot see. Happy to turn whichever direction is preferred into a PR,
including the madd-msub.ll regeneration and a regression test.

### Environment

- Reproduced on `main` @ `17efc66a340e35ae03a18e34e7f267832fff7940` (LLVM 24.0.0git),
  Release/no-assertions host build (AArch64 host), and on LLVM 22.1.8 (rustc-bundled).
- `-mtriple=mips64-unknown-unknown -mcpu=mips3 -O3`; also observed with `-mcpu=mips64`.
