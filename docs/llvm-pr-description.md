# PR description for the SLL64_64 fix

LLVM squash-merges, so this text becomes the commit message. Title:

    [MIPS] Don't mark SLL64_64 as isMoveReg

---

`SLL64_64` is `sll $rd, $rt, 0` with a GPR64 source, which truncates its operand to 32 bits and
sign-extends the result. That is not a register move for an arbitrary 64-bit value: the result
only equals the source when the source is already sign-extended.

Marking it `isMoveReg` makes `MipsSEInstrInfo::isCopyInstrImpl` report it as a copy, and the
register allocator then drops the instruction, letting the untruncated 64-bit value reach its
uses. This is a silent wrong-code bug; it was found by a program whose loop increment, computed
as a sign-extension of the low half of a 64-bit value, accumulated the whole 64-bit value
instead.

### Mechanism (now identified)

`InlineSpiller` spills the `SLL64_64` result and folds the spill into the defining instruction.
`TargetInstrInfo::foldMemoryOperand` has a "Straight COPY may fold as load/store" path
(`llvm/lib/CodeGen/TargetInstrInfo.cpp`, guarded by `if (!isCopyInstr(MI) || Ops.size() != 1)`)
which takes `MI.getOperand(1 - Ops[0])` - the *other* operand, i.e. the copy's source - and emits
`storeRegToStackSlot` for it in place of the instruction. For a real copy that is correct. For
`SLL64_64` it stores the untruncated source, and every later reload yields the wrong value.

`-debug-only=regalloc` on the attached MIR test shows it directly:

```
Inline spilling GPR64:%46
	skipping remat of def %46:gpr64 = SLL64_64 %41:gpr64
	cannot remat for 1632e	%57:gpr64 = DADDu %57:gpr64, %46:gpr64
spillAroundUses %46
	folded:   864r	SD %41:gpr64, %stack.1, 0
	reload:   1624r	%90:gpr64 = LD %stack.1, 0
	rewrite:  1632r	%57:gpr64 = DADDu %57:gpr64, killed %90:gpr64
```

The spill stores `%41`, the *source*, where the value being spilled is `%46`, the sign-extended
result. So the chain is: `isMoveReg` -> `MipsSEInstrInfo::isCopyInstrImpl` returns a
`DestSourcePair` -> `TargetInstrInfo::foldMemoryOperand` folds the "copy" into a store of its
source -> the truncation is gone.

This also explains why the bug needs register pressure: the value has to be spilled *and* the
spill has to be folded into the def. Copy propagation, coalescing and a plain spill across a call
all leave the instruction alone.

`SLL64_32` keeps the flag: its source is a GPR32, and since 32-bit values are kept sign-extended
in their 64-bit registers there the instruction really is value-preserving.

`CodeGen/Mips/madd-msub.ll` needed regenerating. The sign-extensions are still emitted there;
only their destination registers change, because the operands can no longer be coalesced - so the
flag was providing a useful hint as well as an incorrect claim, and this change gives that hint
up. If that trade is not wanted, the alternative is to stop `isCopyInstrImpl` from claiming
`SLL64_64`, which keeps the hint but leaves the flag's meaning wrong for any future consumer.

The new test runs the register allocator over a function that reaches it with four live
`SLL64_64` instructions; before this change one of them is dropped.

Fixes #213419
