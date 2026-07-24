use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::arch::asm;

use arbitrary_int::prelude::*;

use crate::cop0;
use crate::cop1::{set_fcsr, FCSR};
use crate::exception_handler::{drain_seen_exception, ExceptionContext};
use crate::tests::soft_asserts::soft_assert_eq;
use crate::tests::{Level, Test};
use crate::uncached_memory::UncachedHeapMemory;

/// COP0 with the CO bit set (opcode 0b010000, rs=0b1_0000). The low 6 bits select the
/// TLB/exception operation.
const COP0_CO: u32 = (0b010000 << 26) | (0b10000 << 21);

/// Exception code 10 (Reserved Instruction), as it sits in Cause bits 2..=6.
const RI_CODE: u32 = 10;

/// Exception code 11 (Coprocessor Unusable) and 15 (Floating-Point Exception).
const COP_UNUSABLE_CODE: u32 = 11;
const FPE_CODE: u32 = 15;

/// COP1 (opcode 0b010001) shifted into place.
const COP1: u32 = 0b010001 << 26;

/// The function codes in the CO=1 space that the VR4300 actually implements. Everything
/// else in 0x00..=0x3F is reserved - and (as the sweep proves) those reserved codes are
/// silently ignored (no-ops) rather than raising a Reserved Instruction exception.
const DEFINED_FUNCTS: [u32; 5] = [
    1,  // TLBR
    2,  // TLBWI
    6,  // TLBWR
    8,  // TLBP
    24, // ERET
];

/// The one exception to the "reserved CO=1 code is a no-op" rule: function code 0x10 (the
/// R3000's old RFE slot, dropped in the R4000 in favour of ERET) does raise RI on the VR4300.
/// Every other reserved code is a silent no-op; this single one traps. It's asserted by
/// [ReservedEncodingsRaiseRI] instead and excluded from the no-op sweep.
const RFE_FUNCT: u32 = 0x10;

/// Executes a single instruction `word` and returns the exception it raised, if any.
///
/// The word is written into an uncached buffer, followed by a landing pad of `nop` and
/// `jr $ra` slots, and called. Fetching uncached keeps the instruction cache out of it.
/// Whatever the instruction does, control comes back to us:
///  - a genuine no-op falls through into the landing pad
///  - a Reserved Instruction (or any) exception is caught by the default handler, which
///    resumes at EPC+4 - which is inside the landing pad
///  - if a reserved encoding ever decodes as a (short-offset) branch, its target and delay
///    slot both fall inside the pad, which is all `nop`/`jr $ra`
///  - even an unexpected ERET-decode returns safely: EPC is pre-armed into the pad
///
/// The buffer is freed on return, so the context's `exceptpc` points into freed memory - it's an
/// address to compare, not to dereference (a later allocation, e.g. a format! message, reuses it).
fn run_isolated(word: u32) -> Option<(ExceptionContext, u32)> {
    // [0] = the instruction under test. [1..] = a landing pad: a leading nop absorbs a stray
    // branch delay slot / the handler's EPC+4, then alternating jr $ra / nop return to us.
    let mut memory = UncachedHeapMemory::<u32>::new(8);
    memory.write(0, word);
    memory.write(1, 0x0000_0000); // nop
    memory.write(2, 0x03E0_0008); // jr $ra
    memory.write(3, 0x0000_0000); // nop (delay slot)
    memory.write(4, 0x03E0_0008); // jr $ra
    memory.write(5, 0x0000_0000); // nop (delay slot)
    memory.write(6, 0x03E0_0008); // jr $ra
    memory.write(7, 0x0000_0000); // nop (delay slot)

    let body = memory.as_ptr() as usize;
    let landing = body + 8; // address of the first jr $ra

    // Make sure the drain below only observes this one instruction.
    drain_seen_exception();

    unsafe {
        asm!("
            .set noreorder
            .set noat

            move {saved_ra}, $ra

            // Pre-arm EPC to the landing pad (sign-extended via sll ,,0) so an unexpected
            // ERET-decode returns cleanly instead of jumping into hyperspace.
            sll {tmp}, {landing}, 0
            dmtc0 {tmp}, $14
            jalr {body}
            nop
            move $ra, {saved_ra}

            .set at
            .set reorder
            ",
            body = in(reg) body,
            landing = in(reg) landing,
            tmp = out(reg) _,
            saved_ra = out(reg) _,
            // The call may clobber any caller-saved register; list them explicitly since
            // this target doesn't support clobber_abi. ($at/$ra are handled specially by
            // the assembler and can't be listed here.)
            out("$2") _, out("$3") _, out("$4") _, out("$5") _, out("$6") _, out("$7") _,
            out("$8") _, out("$9") _, out("$10") _, out("$11") _, out("$12") _, out("$13") _,
            out("$14") _, out("$15") _, out("$24") _, out("$25") _,
        );
    }

    drain_seen_exception()
}

/// As [run_isolated], but runs the instruction with COP1 forced usable or unusable via
/// Status.CU1. Restores the caller's Status afterwards. Toggling CU1 is safe even for the
/// unusable case: the exception handler momentarily re-enables COP1 to save FCSR.
fn run_isolated_cop1(word: u32, cop1_usable: bool) -> Option<(ExceptionContext, u32)> {
    let saved = cop0::status();
    unsafe { cop0::set_status(saved.with_cop1usable(cop1_usable)) };
    let result = run_isolated(word);
    unsafe { cop0::set_status(saved) };
    result
}

/// The claim (as e.g. emux assumes): reserved COP0 CO=1 function codes are no-ops on the
/// VR4300 - they neither trap nor touch any TLB register. This walks every reserved function
/// code in that space and proves exactly that.
pub struct ReservedCOP0FunctionsAreNops {}

impl Test for ReservedCOP0FunctionsAreNops {
    fn name(&self) -> &str {
        "Reserved COP0 (CO=1) function codes are no-ops"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        (0u32..64)
            .filter(|funct| !DEFINED_FUNCTS.contains(funct) && *funct != RFE_FUNCT)
            .map(|funct| Box::new(COP0_CO | funct) as Box<dyn Any>)
            .collect()
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let word = *value.downcast_ref::<u32>().unwrap();

        // Snapshot the TLB registers a stray TLB operation would disturb. If the reserved
        // code secretly behaved like TLBP/TLBR/TLBWI, one of these would move.
        let index = cop0::index();
        let entry_hi = cop0::entry_hi();
        let entry_lo0 = cop0::entry_lo0_64();
        let entry_lo1 = cop0::entry_lo1_64();
        let pagemask = cop0::pagemask();

        if let Some((context, count)) = run_isolated(word) {
            return Err(format!(
                "Expected no-op but {:#010x} raised {} exception(s); CauseRaw={:#010x} EPC={:#018x}",
                word,
                count,
                context.cause.raw_value(),
                context.exceptpc,
            ));
        }

        soft_assert_eq(cop0::index(), index, "Index changed - not a no-op")?;
        soft_assert_eq(cop0::entry_hi(), entry_hi, "EntryHi changed - not a no-op")?;
        soft_assert_eq(
            cop0::entry_lo0_64(),
            entry_lo0,
            "EntryLo0 changed - not a no-op",
        )?;
        soft_assert_eq(
            cop0::entry_lo1_64(),
            entry_lo1,
            "EntryLo1 changed - not a no-op",
        )?;
        soft_assert_eq(cop0::pagemask(), pagemask, "PageMask changed - not a no-op")?;

        Ok(())
    }
}

/// Reserved main opcodes (bits 31..=26) that the VR4300 does not implement. The COP2 family
/// (opcode 0x12 and the LWC2/LDC2/SWC2/SDC2 loads/stores) is deliberately excluded here: with
/// COP2 disabled those raise Coprocessor Unusable, not RI, and are covered by the cop_unusable
/// tests.
const RESERVED_MAIN_OPCODES: [u32; 7] = [
    0x13, // COP3 - removed in MIPS III
    0x1C, 0x1D, 0x1E, 0x1F, // gap between LDR (0x1B) and the load group
    0x33, // gap between LWC2 (0x32) and LLD (0x34)
    0x3B, // gap between SWC2 (0x3A) and SCD (0x3C)
];

/// Reserved SPECIAL function codes (opcode 0x00, bits 5..=0). The MOVCI/MOVZ/MOVN slots
/// (0x01/0x0A/0x0B) belong to MIPS IV and are reserved on the MIPS III VR4300.
const RESERVED_SPECIAL_FUNCTS: [u32; 12] = [
    0x01, 0x05, 0x0A, 0x0B, 0x0E, 0x15, 0x28, 0x29, 0x35, 0x37, 0x39, 0x3D,
];

/// Reserved REGIMM rt codes (opcode 0x01, bits 20..=16). Everything outside the branch and
/// trap-immediate rows.
const RESERVED_REGIMM_RT: [u32; 18] = [
    0x04, 0x05, 0x06, 0x07, 0x0D, 0x0F, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D,
    0x1E, 0x1F,
];

/// Reserved COP0 sub-opcodes (opcode 0x10, rs field bits 25..=21) that raise RI. These are
/// the rs values with no coprocessor meaning at all: not MFC0/DMFC0/MTC0/DMTC0 (0/1/4/5),
/// not the recognised-but-idle CFC0/CTC0/BC0 (see [COP0_RECOGNISED_RS]), and without the CO
/// bit set (rs >= 0x10, handled via the function field).
const RESERVED_COP0_RS: [u32; 9] = [0x03, 0x07, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F];

/// COP0 sub-opcodes that the VR4300 decodes but treats as inert rather than trapping: the
/// generic coprocessor CFC0 (rs=2) and CTC0 (rs=6) - which have no COP0 control registers to
/// move and simply do nothing - and BC0 (rs=8), a coprocessor-condition branch that is never
/// taken. All three raise no exception; the reserved rs values around them do.
const COP0_RECOGNISED_RS: [u32; 3] = [0x02, 0x06, 0x08];

/// The counterpart to [ReservedCOP0FunctionsAreNops]: the reserved encodings across the CPU's
/// integer and COP0 spaces that do raise a Reserved Instruction exception. Between the two
/// tests, every reserved slot of the main-opcode, SPECIAL, REGIMM and COP0 fields is
/// classified as either a no-op (COP0 CO=1 only) or an RI trap (everything here).
pub struct ReservedEncodingsRaiseRI {}

impl ReservedEncodingsRaiseRI {
    fn encodings() -> Vec<u32> {
        let mut words = Vec::new();
        // Reserved main opcodes: opcode in bits 31..=26, all operand fields zero.
        words.extend(RESERVED_MAIN_OPCODES.iter().map(|op| op << 26));
        // Reserved SPECIAL functs: opcode 0x00, funct in bits 5..=0.
        words.extend(RESERVED_SPECIAL_FUNCTS.iter().copied());
        // Reserved REGIMM rt: opcode 0x01 in bits 31..=26, rt in bits 20..=16.
        words.extend(
            RESERVED_REGIMM_RT
                .iter()
                .map(|rt| (0x01 << 26) | (rt << 16)),
        );
        // Reserved COP0 rs sub-opcodes: opcode 0x10 in bits 31..=26, rs in bits 25..=21.
        words.extend(RESERVED_COP0_RS.iter().map(|rs| (0x10 << 26) | (rs << 21)));
        // The lone reserved COP0 CO=1 function code that traps.
        words.push(COP0_CO | RFE_FUNCT);
        words
    }
}

impl Test for ReservedEncodingsRaiseRI {
    fn name(&self) -> &str {
        "Reserved integer/COP0 encodings raise RI"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        Self::encodings()
            .into_iter()
            .map(|w| Box::new(w) as Box<dyn Any>)
            .collect()
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let word = *value.downcast_ref::<u32>().unwrap();

        match run_isolated(word) {
            None => Err(format!(
                "Expected RI but {:#010x} was ignored (no exception)",
                word
            )),
            Some((context, count)) => {
                soft_assert_eq(count, 1, "Expected exactly one exception")?;
                let code = (context.cause.raw_value() >> 2) & 0x1F;
                soft_assert_eq(code, RI_CODE, "Exception code (want RI=10)")?;
                Ok(())
            }
        }
    }
}

/// The COP0 sub-opcodes the VR4300 recognises but leaves inert: CFC0, CTC0 and BC0. Unlike the
/// reserved rs values around them (see [ReservedEncodingsRaiseRI]) they raise no exception -
/// which is what makes the "reserved COP0 rs traps" boundary precise. This pins that boundary
/// so an emulator can tell the two apart.
pub struct COP0RecognisedSubOpcodesDoNotTrap {}

impl Test for COP0RecognisedSubOpcodesDoNotTrap {
    fn name(&self) -> &str {
        "COP0 CFC0/CTC0/BC0 are recognised (no RI)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        COP0_RECOGNISED_RS
            .iter()
            .map(|rs| Box::new((0x10 << 26) | (rs << 21)) as Box<dyn Any>)
            .collect()
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let word = *value.downcast_ref::<u32>().unwrap();

        if let Some((context, count)) = run_isolated(word) {
            return Err(format!(
                "Expected no trap but {:#010x} raised {} exception(s); CauseRaw={:#010x}",
                word,
                count,
                context.cause.raw_value(),
            ));
        }
        Ok(())
    }
}

/// Reserved COP1 sub-opcodes (opcode 0x11, rs field bits 25..=21): everything that isn't
/// MFC1/DMFC1/CFC1/MTC1/DMTC1/CTC1 (0/1/2/4/5/6), BC1 (8), or a valid format S/D/W/L
/// (0x10/0x11/0x14/0x15).
const COP1_RESERVED_RS: [u32; 21] = [
    0x03, 0x07, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x12, 0x13, 0x16, 0x17, 0x18, 0x19, 0x1A,
    0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
];

/// The four valid COP1 formats and their format-field values: S, D (arithmetic) and W, L
/// (integer sources, only used by conversions).
const COP1_FORMATS: [u32; 4] = [0x10, 0x11, 0x14, 0x15];

/// Whether function code `f` is a defined operation in COP1 format `fmt`.
///
/// S/D support arithmetic + round/trunc/ceil/floor (0x00..=0x0F), the compares (0x30..=0x3F),
/// and the four conversions CVT.S/D/W/L (0x20/0x21/0x24/0x25) - except converting a format to
/// itself (CVT.S in S, CVT.D in D), which is unimplemented. W/L are integer source formats and
/// only support CVT.S and CVT.D (word/long -> float); everything else is unimplemented.
const fn cop1_funct_is_defined(fmt: u32, f: u32) -> bool {
    match fmt {
        0x10 | 0x11 => {
            f <= 0x0F
                || (0x30 <= f && f <= 0x3F)
                || f == 0x24
                || f == 0x25
                || (f == 0x20 && fmt != 0x10)
                || (f == 0x21 && fmt != 0x11)
        }
        _ => f == 0x20 || f == 0x21, // W, L
    }
}

/// Every reserved COP1 encoding both dimensions exercise: the reserved sub-opcodes, the BC1
/// reserved branch conditions, and every reserved function code across all four formats.
fn cop1_reserved_encodings() -> Vec<u32> {
    let mut words = Vec::new();
    // Reserved sub-opcodes (function field irrelevant).
    words.extend(COP1_RESERVED_RS.iter().map(|rs| COP1 | (rs << 21)));
    // BC1 (rs = 8) reserved conditions: only rt 0..=3 (BC1F/T/FL/TL) are defined.
    words.extend((4u32..32).map(|rt| COP1 | (0x08 << 21) | (rt << 16)));
    // Every undefined function code in each valid format.
    for &fmt in COP1_FORMATS.iter() {
        words.extend(
            (0u32..64)
                .filter(|&f| !cop1_funct_is_defined(fmt, f))
                .map(move |f| COP1 | (fmt << 21) | f),
        );
    }
    words
}

/// With COP1 usable, reserved COP1 encodings don't raise Reserved Instruction like their
/// integer counterparts - the FPU decodes them and raises a Floating-Point Exception flagged
/// as an Unimplemented Operation. This proves that split.
pub struct COP1ReservedEncodingsAreUnimplemented {}

impl Test for COP1ReservedEncodingsAreUnimplemented {
    fn name(&self) -> &str {
        "Reserved COP1 encodings raise FPE (unimplemented)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        cop1_reserved_encodings()
            .into_iter()
            .map(|w| Box::new(w) as Box<dyn Any>)
            .collect()
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let word = *value.downcast_ref::<u32>().unwrap();

        // Clean FCSR so the only cause that can surface is the (unmaskable) unimplemented one.
        set_fcsr(FCSR::ZERO);

        match run_isolated(word) {
            None => Err(format!(
                "Expected FPE but {:#010x} was ignored (no exception)",
                word
            )),
            Some((context, count)) => {
                soft_assert_eq(count, 1, "Expected exactly one exception")?;
                let code = (context.cause.raw_value() >> 2) & 0x1F;
                soft_assert_eq(code, FPE_CODE, "Exception code (want FPE=15)")?;
                soft_assert_eq(
                    context.fcsr.cause_unimplemented_operation(),
                    true,
                    "FCSR unimplemented-operation cause bit",
                )?;
                Ok(())
            }
        }
    }
}

/// The other half of the emulator's job: with COP1 marked unusable (Status.CU1 = 0), the
/// usability check wins over the decode - every COP1 instruction, reserved or not, raises
/// Coprocessor Unusable for coprocessor 1 rather than FPE or RI. Run over the same reserved
/// encodings so both dimensions are covered.
pub struct COP1UnusableTakesPrecedence {}

impl Test for COP1UnusableTakesPrecedence {
    fn name(&self) -> &str {
        "COP1 unusable beats reserved-decode (CopUnusable)"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        cop1_reserved_encodings()
            .into_iter()
            .map(|w| Box::new(w) as Box<dyn Any>)
            .collect()
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let word = *value.downcast_ref::<u32>().unwrap();

        match run_isolated_cop1(word, false) {
            None => Err(format!(
                "Expected Coprocessor Unusable but {:#010x} was ignored",
                word
            )),
            Some((context, count)) => {
                soft_assert_eq(count, 1, "Expected exactly one exception")?;
                let code = (context.cause.raw_value() >> 2) & 0x1F;
                soft_assert_eq(
                    code,
                    COP_UNUSABLE_CODE,
                    "Exception code (want CopUnusable=11)",
                )?;
                let coprocessor = (context.cause.raw_value() >> 28) & 0x3;
                soft_assert_eq(coprocessor, 1, "Coprocessor number (want COP1)")?;
                Ok(())
            }
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Are the CO-space operand bits don't-care?
//
// The sweeps above only exercise CO-space words with all operand bits zero. These probe whether
// the remaining bits - rs-low (24..=21) and the rt/rd/imm field (20..=6) - are ignored by the
// decoder. Classic MIPS decode dispatches on the funct field alone, so every pattern below should
// behave like the canonical (all-zero) encoding: defined functs execute, funct 0x10 raises RI, and
// reserved functs stay no-ops.
// -------------------------------------------------------------------------------------------------

/// Operand-bit patterns OR'd into a CO-space word (bits 24..=6). If only the funct field is
/// decoded, every one behaves like 0x00000000.
const CO_GARBAGE: [u32; 5] = [
    0x0000_0000, // control: canonical encoding
    0x01E0_0000, // rs-low (bits 24..=21) all set
    0x001F_FFC0, // rt/rd/imm (bits 20..=6) all set
    0x01FF_FFC0, // every don't-care bit (24..=6) set
    0x0020_0000, // a single low rs bit (bit 21)
];

/// A spare, high TLB index used to prime a distinctive entry for the TLBR/TLBP probes.
const CO_TLB_INDEX: u32 = 30;
const CO_TLB_ASID: u8 = 0xAB;

fn co_tlb_entry_hi() -> u64 {
    cop0::make_entry_hi(CO_TLB_ASID, u27::new(0x12345), u2::new(0))
}

/// Writes a distinctive entry to [CO_TLB_INDEX] so TLBR/TLBP have something recognisable to find.
fn prime_co_tlb_entry() {
    unsafe {
        cop0::write_tlb_untyped(
            CO_TLB_INDEX,
            0, // 4K page
            cop0::make_entry_lo(false, true, false, cop0::Coherency::Cached, 0x111),
            cop0::make_entry_lo(false, true, false, cop0::Coherency::Cached, 0x222),
            co_tlb_entry_hi(),
        );
    }
}

/// Invalidates [CO_TLB_INDEX] and restores EntryHi so neighbouring tests aren't disturbed.
fn restore_co_tlb_entry(saved_entry_hi: u64) {
    unsafe {
        cop0::write_tlb_untyped(
            CO_TLB_INDEX,
            0,
            0,
            0,
            cop0::make_entry_hi(1, u27::new(0), u2::new(0)),
        );
        cop0::set_entry_hi(saved_entry_hi);
    }
}

/// Loads recognisable non-matching values into the registers TLBR overwrites, so a no-op leaves
/// them detectably unchanged. PageMask differs from the primed 4K entry.
fn set_tlb_sentinels() {
    unsafe {
        cop0::set_entry_hi(0);
        cop0::set_entry_lo0(0);
        cop0::set_entry_lo1(0);
        cop0::set_pagemask(0x6000); // 16K
        cop0::set_index(CO_TLB_INDEX);
    }
}

fn read_tlb_regs() -> (u64, u32, u32, u32) {
    (
        cop0::entry_hi(),
        cop0::entry_lo0(),
        cop0::entry_lo1(),
        cop0::pagemask(),
    )
}

/// TLBR (funct 1) executes for any value of the don't-care operand bits: reading the primed entry
/// back through each garbage encoding yields the same registers as the canonical TLBR.
pub struct COOperandBitsTLBRExecutes {}

impl Test for COOperandBitsTLBRExecutes {
    fn name(&self) -> &str {
        "CO operand bits: TLBR executes regardless"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        CO_GARBAGE
            .iter()
            .map(|g| Box::new(*g) as Box<dyn Any>)
            .collect()
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let garbage = *value.downcast_ref::<u32>().unwrap();
        let saved_hi = cop0::entry_hi();
        prime_co_tlb_entry();

        // Reference: what a canonical TLBR of the primed index yields.
        set_tlb_sentinels();
        unsafe { cop0::tlbr() };
        let reference = read_tlb_regs();

        // The garbage-encoded TLBR.
        set_tlb_sentinels();
        let word = (COP0_CO | 1) | garbage;
        let exc = run_isolated(word);
        let got = read_tlb_regs();

        restore_co_tlb_entry(saved_hi);

        if let Some((context, count)) = exc {
            return Err(format!(
                "TLBR {word:#010x} raised {count} exception(s); CauseRaw={:#010x}",
                context.cause.raw_value()
            ));
        }
        soft_assert_eq(
            got.0,
            reference.0,
            &format!("TLBR {word:#010x}: EntryHi (sentinel value means it decoded as a no-op)"),
        )?;
        soft_assert_eq(got.1, reference.1, &format!("TLBR {word:#010x}: EntryLo0"))?;
        soft_assert_eq(got.2, reference.2, &format!("TLBR {word:#010x}: EntryLo1"))?;
        soft_assert_eq(got.3, reference.3, &format!("TLBR {word:#010x}: PageMask"))?;
        Ok(())
    }
}

/// TLBP (funct 8) executes for any value of the don't-care operand bits: it finds the primed entry
/// and writes its index, rather than leaving the sentinel index untouched.
pub struct COOperandBitsTLBPExecutes {}

impl Test for COOperandBitsTLBPExecutes {
    fn name(&self) -> &str {
        "CO operand bits: TLBP executes regardless"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        CO_GARBAGE
            .iter()
            .map(|g| Box::new(*g) as Box<dyn Any>)
            .collect()
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let garbage = *value.downcast_ref::<u32>().unwrap();
        let saved_hi = cop0::entry_hi();
        prime_co_tlb_entry();

        unsafe {
            cop0::set_entry_hi(co_tlb_entry_hi()); // match the primed entry
            cop0::set_index(0x1F); // sentinel (differs from the primed index, P-bit clear)
        }
        let word = (COP0_CO | 8) | garbage;
        let exc = run_isolated(word);
        let got_index = cop0::index();

        restore_co_tlb_entry(saved_hi);

        if let Some((context, count)) = exc {
            return Err(format!(
                "TLBP {word:#010x} raised {count} exception(s); CauseRaw={:#010x}",
                context.cause.raw_value()
            ));
        }
        soft_assert_eq(
            got_index,
            CO_TLB_INDEX,
            &format!(
                "TLBP {word:#010x}: Index (sentinel 0x1F means no-op; bit 31 set means no match)"
            ),
        )?;
        Ok(())
    }
}

/// ERET (funct 24) executes for any value of the don't-care operand bits: with Status.EXL set
/// beforehand, executing it clears EXL. A no-op would leave EXL set.
pub struct COOperandBitsERETExecutes {}

impl Test for COOperandBitsERETExecutes {
    fn name(&self) -> &str {
        "CO operand bits: ERET executes regardless"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        CO_GARBAGE
            .iter()
            .map(|g| Box::new(*g) as Box<dyn Any>)
            .collect()
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let garbage = *value.downcast_ref::<u32>().unwrap();
        let saved_status = cop0::status();

        unsafe { cop0::set_status(saved_status.with_exl(true)) };
        let word = (COP0_CO | 24) | garbage;
        // run_isolated pre-arms EPC to its landing pad, so ERET returns cleanly.
        let exc = run_isolated(word);
        let exl_after = cop0::status().exl();
        unsafe { cop0::set_status(saved_status) };

        if let Some((context, count)) = exc {
            return Err(format!(
                "ERET {word:#010x} raised {count} exception(s); CauseRaw={:#010x}",
                context.cause.raw_value()
            ));
        }
        soft_assert_eq(
            exl_after,
            false,
            &format!("ERET {word:#010x}: Status.EXL still set means it decoded as a no-op"),
        )?;
        Ok(())
    }
}

/// funct 0x10 (RFE) raises RI for any value of the don't-care operand bits.
pub struct COOperandBitsRFETraps {}

impl Test for COOperandBitsRFETraps {
    fn name(&self) -> &str {
        "CO operand bits: funct 0x10 raises RI regardless"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        CO_GARBAGE
            .iter()
            .map(|g| Box::new((COP0_CO | RFE_FUNCT) | g) as Box<dyn Any>)
            .collect()
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let word = *value.downcast_ref::<u32>().unwrap();
        match run_isolated(word) {
            None => Err(format!(
                "Expected RI but {word:#010x} was ignored (no exception)"
            )),
            Some((context, count)) => {
                soft_assert_eq(
                    count,
                    1,
                    &format!("{word:#010x}: expected exactly one exception"),
                )?;
                let code = (context.cause.raw_value() >> 2) & 0x1F;
                soft_assert_eq(
                    code,
                    RI_CODE,
                    &format!("{word:#010x}: exception code (want RI=10)"),
                )?;
                Ok(())
            }
        }
    }
}

/// A few reserved functs (not used by emux) stay no-ops even with operand bits set.
const CO_RESERVED_PROBE_FUNCTS: [u32; 4] = [0x03, 0x09, 0x1F, 0x3F];

pub struct COOperandBitsReservedStayNops {}

impl Test for COOperandBitsReservedStayNops {
    fn name(&self) -> &str {
        "CO operand bits: reserved functs stay no-ops"
    }

    fn level(&self) -> Level {
        Level::Weird
    }

    fn values(&self) -> Vec<Box<dyn Any>> {
        let mut words = Vec::new();
        for funct in CO_RESERVED_PROBE_FUNCTS.iter() {
            for garbage in CO_GARBAGE.iter() {
                words.push(Box::new((COP0_CO | funct) | garbage) as Box<dyn Any>);
            }
        }
        words
    }

    fn run(&self, value: &Box<dyn Any>) -> Result<(), String> {
        let word = *value.downcast_ref::<u32>().unwrap();

        let index = cop0::index();
        let entry_hi = cop0::entry_hi();
        let entry_lo0 = cop0::entry_lo0_64();
        let entry_lo1 = cop0::entry_lo1_64();
        let pagemask = cop0::pagemask();

        if let Some((context, count)) = run_isolated(word) {
            return Err(format!(
                "Expected no-op but {word:#010x} raised {count} exception(s); CauseRaw={:#010x}",
                context.cause.raw_value()
            ));
        }

        soft_assert_eq(
            cop0::index(),
            index,
            &format!("{word:#010x}: Index changed"),
        )?;
        soft_assert_eq(
            cop0::entry_hi(),
            entry_hi,
            &format!("{word:#010x}: EntryHi changed"),
        )?;
        soft_assert_eq(
            cop0::entry_lo0_64(),
            entry_lo0,
            &format!("{word:#010x}: EntryLo0 changed"),
        )?;
        soft_assert_eq(
            cop0::entry_lo1_64(),
            entry_lo1,
            &format!("{word:#010x}: EntryLo1 changed"),
        )?;
        soft_assert_eq(
            cop0::pagemask(),
            pagemask,
            &format!("{word:#010x}: PageMask changed"),
        )?;
        Ok(())
    }
}
