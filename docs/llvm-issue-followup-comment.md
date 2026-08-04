# Draft: follow-up comment for llvm/llvm-project#213419

Post as-is. Two purposes: correct the root-cause chain claimed in the issue body (it is a
hypothesis, not something I demonstrated), and add the verification done since filing.

---

Some follow-up work since filing, including a correction to the issue body.

### Update: the causal chain is now identified

The description's root cause was right in outline but I had not verified the middle of it. I have
now, and it is more specific than "the allocator treats it as a copy".

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

### The observation itself, re-verified on an assertions build

Built `main` (`17efc66a340e35ae03a18e34e7f267832fff7940`) with `LLVM_ENABLE_ASSERTIONS=ON`, MIPS
only. The attached reproducer compiles with no assertion failures and still loses the
truncations: `SLL64_64` goes 4 → 2 across the Greedy Register Allocator.

### The proposed fix, measured

Moving `SLL64_64` out of the `isMoveReg` block:

- all four `SLL64_64` survive register allocation, and the generated code no longer has any site
  that reloads a raw 64-bit value straight into a `daddu` (the unfixed build has two);
- `check-llvm-codegen-mips`: **1025 passed, 0 failed** after regenerating one test;
- the only affected test is `CodeGen/Mips/madd-msub.ll`, and the change is register numbering
  only - `sll $4, $4, 0` becomes `sll $1, $4, 0`. The sign-extension is still emitted; what is
  lost is the operand coalescing that the copy hint used to enable. So the flag is doing
  something useful as well as something wrong, and this fix gives up the useful part. If that
  trade is not acceptable, the alternative is to stop whichever consumer deletes the instruction
  from treating it as a copy, which needs the mechanism pinned down first.

### Next

I am preparing a PR with the `.td` change, the regenerated test, and a `-run-pass=greedy` MIR
regression test reduced against a two-binary oracle (unpatched must delete, patched must keep, and
an assertions build must accept the MIR - a line-based reducer will otherwise happily produce
invalid MIR that a no-assertions `llc` accepts silently).
