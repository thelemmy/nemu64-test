# Draft: follow-up comment for llvm/llvm-project#213419

Post as-is. Two purposes: correct the root-cause chain claimed in the issue body (it is a
hypothesis, not something I demonstrated), and add the verification done since filing.

---

Some follow-up work since filing, including a correction to the issue body.

### Correction: the causal chain in the description is not established

The description asserts `isMoveReg` → `MipsSEInstrInfo::isCopyInstrImpl` → the allocator treats
dest as equal to source and eliminates the instruction. The first and last links are solid, but I
have **not** demonstrated the middle one, and several experiments argue against the simple version
of it. None of these delete an `SLL64_64`:

- `-run-pass=machine-cp` forwarding a `SLL64_64` result;
- an identity `$v0_64 = SLL64_64 $v0_64` run through `machine-cp`, `postra-machine-sink` and
  `machine-latecleanup`;
- `-run-pass=register-coalescer` on minimal virtual-register MIR;
- `-run-pass=greedy` on the same minimal MIR;
- a value spilled across a call (tried with 1 and with 12 live sign-extended values).

So generic copy machinery leaves the instruction alone, and the trigger is narrower than "any
`isCopyInstr` consumer". What is verified is only that the deletion happens **during the Greedy
Register Allocator** and that removing the flag prevents it. My current guess is `InlineSpiller`
treating the destination as a sibling value of the source and reusing its spill slot - the final
assembly matches that shape, with the raw 64-bit value spilled once and reloaded straight into the
add - but I have not confirmed which decision actually removes it. Please read the description's
"root cause" as a hypothesis.

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
