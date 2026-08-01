# Draft: follow-up comment proposing the two fixes

To post on the issue after filing. Both options were implemented and tested locally on `main`
(`17efc66a3`, Mips-only release build): each one makes all 4 `SLL64_64` survive register
allocation in the reproducer (full pipeline and `-run-pass=greedy`), eliminates every
"reload straight into `daddu` with no sign-extension" site (baseline has 2), and has the same,
single, cosmetic test fallout.

---

I looked into possible fixes and tested two. For context, the current definition in
`llvm/lib/Target/Mips/Mips64InstrInfo.td`:

```tablegen
let isCodeGenOnly = 1, rs = 0, shamt = 0 in {
  def DSLL64_32 : FR<0x00, 0x3c, (outs GPR64:$rd), (ins GPR32:$rt),
                     "dsll\t$rd, $rt, 32", []>, GPR_64;
  let isMoveReg = 1 in {
    def SLL64_32 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR32:$rt),
                      "sll\t$rd, $rt, 0", []>, GPR_64;
    def SLL64_64 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR64:$rt),
                      "sll\t$rd, $rt, 0", []>, GPR_64;
  }
}
```

and the consumer that turns the flag into "destination equals source", in
`llvm/lib/Target/Mips/MipsSEInstrInfo.cpp`:

```cpp
std::optional<DestSourcePair>
MipsSEInstrInfo::isCopyInstrImpl(const MachineInstr &MI) const {
  if (MI.isMoveReg() || isORCopyInst(MI))
    return DestSourcePair{MI.getOperand(0), MI.getOperand(1)};

  return std::nullopt;
}
```

### Option A: stop marking `SLL64_64` as a move

```diff
--- a/llvm/lib/Target/Mips/Mips64InstrInfo.td
+++ b/llvm/lib/Target/Mips/Mips64InstrInfo.td
   let isCodeGenOnly = 1, rs = 0, shamt = 0 in {
     def DSLL64_32 : FR<0x00, 0x3c, (outs GPR64:$rd), (ins GPR32:$rt),
                        "dsll\t$rd, $rt, 32", []>, GPR_64;
-    let isMoveReg = 1 in {
-      def SLL64_32 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR32:$rt),
-                        "sll\t$rd, $rt, 0", []>, GPR_64;
-      def SLL64_64 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR64:$rt),
-                        "sll\t$rd, $rt, 0", []>, GPR_64;
-    }
+    let isMoveReg = 1 in
+    def SLL64_32 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR32:$rt),
+                      "sll\t$rd, $rt, 0", []>, GPR_64;
+    // Not isMoveReg: with a 64-bit source this truncates to 32 bits and
+    // sign-extends, which is not value-preserving.
+    def SLL64_64 : FR<0x0, 0x00, (outs GPR64:$rd), (ins GPR64:$rt),
+                      "sll\t$rd, $rt, 0", []>, GPR_64;
   }
```

`SLL64_32` keeps the flag: its source is a GPR32, and since the backend maintains the invariant
that 32-bit values live sign-extended in 64-bit registers, that one really is value-preserving.

### Option B: keep the flag, make `isCopyInstrImpl` not claim it

```diff
--- a/llvm/lib/Target/Mips/MipsSEInstrInfo.cpp
+++ b/llvm/lib/Target/Mips/MipsSEInstrInfo.cpp
 std::optional<DestSourcePair>
 MipsSEInstrInfo::isCopyInstrImpl(const MachineInstr &MI) const {
+  // SLL64_64 is marked isMoveReg, but with a 64-bit source it truncates to
+  // 32 bits and sign-extends, so its result only equals its source if the
+  // source is already sign-extended - which cannot be determined here.
+  // Claiming it as a copy lets the register allocator delete it, dropping
+  // the truncation.
+  if (MI.getOpcode() == Mips::SLL64_64)
+    return std::nullopt;
+
   if (MI.isMoveReg() || isORCopyInst(MI))
     return DestSourcePair{MI.getOperand(0), MI.getOperand(1)};

   return std::nullopt;
 }
```

### Measured results (identical for both)

- Reproducer: all 4 `SLL64_64` survive `greedy`; generated code re-extends after reloads
  (baseline has two sites that `daddu` a reloaded raw 64-bit word).
- `llvm/test/CodeGen/Mips`: one test affected in both cases, `madd-msub.ll` - the expected
  `sll $4, $4, 0` becomes `sll $1, $4, 0`. The sign-extension is still emitted; only the
  operand coalescing is lost, because that hint also flowed through `isCopyInstr()`. So the
  test needs its CHECK lines regenerated either way, and there appears to be no codegen-quality
  argument between A and B.

Given the identical outcomes, Option A seems preferable: it is smaller, and it removes the
mismatch at its source instead of compensating for it downstream - any future `isMoveReg`
consumer would otherwise re-introduce the same bug through B's remaining flag. But if `isMoveReg`
is intended to mean something weaker than "destination equals source" (register-allocation
hinting only), then B is the correct scoping and `SLL64_32`/`Mips16` deserve the same audit.

Happy to open a PR for whichever direction is preferred, including the `madd-msub.ll`
regeneration and a `-run-pass=greedy` MIR regression test.

---

# Audit: other `isMoveReg` instructions in the MIPS backend

Checked every `isMoveReg = 1` site in `llvm/lib/Target/Mips/*.td` against the consumer contract
(`isCopyInstrImpl` returns `DestSourcePair{getOperand(0), getOperand(1)}`, i.e. "operand 0's
register now holds operand 1's value"). Findings, by confidence:

## Same bug family, structurally wrong: `MOVEP_MM` / `MOVEP_MMR6` (microMIPS `movep`)

`MicroMipsInstrInfo.td:231` (`MovePMM16`, also used by `MOVEP_MMR6_DESC`):

```tablegen
class MovePMM16<...> :
MicroMipsInst16<(outs RO1:$rd1, RO2:$rd2), (ins RO3:$rs, RO3:$rt), ...> {
  let isReMaterializable = 1;
  let isMoveReg = 1;
  ...
}
```

`movep` performs TWO moves: rd1 <- rs, rd2 <- rt. `isCopyInstrImpl` returns
`{getOperand(0), getOperand(1)}` = `{$rd1, $rd2}` - which claims **rd1 is a copy of the second
destination**. Both are GPRs, so unlike cross-bank moves nothing structurally prevents a consumer
from acting on the bogus pair. Untested (no microMIPS hardware here), but the operand indexing is
wrong by inspection; `isCopyInstr` has a multi-move-aware overload that `movep` would need, or the
flag should go.

## Questionable: `CFC1` / `CTC1` (FP control register moves)

`MipsInstrFPU.td:597/599` - both inherit `isMoveReg = 1` from `MFC1_FT`/`MTC1_FT`. These
move to/from the FP control registers (including FCSR: rounding mode, exception enables/flags),
which FP instructions read and modify implicitly. Describing them as plain value-preserving
copies, with no side-effect modelling, invites copy-forwarding around instructions that change
FCSR. The cross-bank shape (GPR vs CCR) makes the identity-deletion path of this bug impossible,
so this is a weaker concern - flagged for completeness.

## Checked and fine

- `SLL64_32`: claims GPR64 dest = GPR32 source. In practice safe with current consumers: the
  cross-class shape means dest and source can never be the same register, so the deletion path
  cannot trigger, and its input is produced by 32-bit ops (already sign-extended).
- `FMOV_S`/`FMOV_D`(+microMIPS variants), MSA `MOVE_V`: genuine same-class copies.
- `MFHI`/`MFLO`/`MTHI`/`MTLO` (base, microMIPS, Mips16, DSP): genuine cross-bank moves; the
  HI/LO source appears as the implicit-use operand, which `getOperand(1)` picks up correctly.
- `MFC1`/`MTC1`/`DMFC1`/`DMTC1`: bit-preserving cross-bank moves.
- `MTC1_D64`(`MTC1_64_FT`): writes only the low half of a 64-bit FPR (tied operand) - would be
  exactly this bug if flagged, but it is (correctly) NOT marked `isMoveReg`.
- MSA `CFCMSA`/`CTCMSA`: control-register moves, but at least marked `hasSideEffects = 1`.

Curiosity: Mips16 `Mfhi16` has `isMoveReg = 1` while the adjacent, structurally identical
`Mflo16` has an explicit `isMoveReg = 0` - the asymmetry suggests someone already ran into
trouble with one of these.
