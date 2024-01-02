use core::iter::Step;

use arbitrary_int::{u26, u5, u6};
use bitbybit::bitenum;

use crate::cop0::{CacheOp, RegisterIndex};
use crate::rsp::rsp_assembler::EMUXFunction;

#[bitenum(u5, exhaustive: true)]
#[allow(dead_code)]
#[derive(Debug, PartialOrd, PartialEq, Eq)]
pub enum GPR {
    R0 = 0,
    AT = 1,
    V0 = 2,
    V1 = 3,
    A0 = 4,
    A1 = 5,
    A2 = 6,
    A3 = 7,
    T0 = 8,
    T1 = 9,
    T2 = 10,
    T3 = 11,
    T4 = 12,
    T5 = 13,
    T6 = 14,
    T7 = 15,
    S0 = 16,
    S1 = 17,
    S2 = 18,
    S3 = 19,
    S4 = 20,
    S5 = 21,
    S6 = 22,
    S7 = 23,
    T8 = 24,
    T9 = 25,
    K0 = 26,
    K1 = 27,
    GP = 28,
    SP = 29,
    S8 = 30,
    RA = 31,
}

impl Step for GPR {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if (*start as usize) < (*end as usize) {
            Some(*end as usize - *start as usize)
        } else {
            None
        }
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        let next = start.raw_value().value() as usize + count;
        if next >= 32 {
            None
        } else {
            Some(Self::new_with_raw_value(u5::new(next as u8)))
        }
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        let next = start.raw_value().value() as usize - count;
        if next >= 32 {
            None
        } else {
            Some(Self::new_with_raw_value(u5::new(next as u8)))
        }
    }
}

#[bitenum(u5, exhaustive: true)]
#[allow(dead_code)]
#[derive(Debug, PartialOrd, PartialEq, Eq)]
pub enum FR {
    F0 = 0,
    F1 = 1,
    F2 = 2,
    F3 = 3,
    F4 = 4,
    F5 = 5,
    F6 = 6,
    F7 = 7,
    F8 = 8,
    F9 = 9,
    F10 = 10,
    F11 = 11,
    F12 = 12,
    F13 = 13,
    F14 = 14,
    F15 = 15,
    F16 = 16,
    F17 = 17,
    F18 = 18,
    F19 = 19,
    F20 = 20,
    F21 = 21,
    F22 = 22,
    F23 = 23,
    F24 = 24,
    F25 = 25,
    F26 = 26,
    F27 = 27,
    F28 = 28,
    F29 = 29,
    F30 = 30,
    F31 = 31,
}

impl Step for FR {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if (*start as usize) < (*end as usize) {
            Some(*end as usize - *start as usize)
        } else {
            None
        }
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        let next = start.raw_value().value() as usize + count;
        if next >= 32 {
            None
        } else {
            Some(Self::new_with_raw_value(u5::new(next as u8)))
        }
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        let next = start.raw_value().value() as usize - count;
        if next >= 32 {
            None
        } else {
            Some(Self::new_with_raw_value(u5::new(next as u8)))
        }
    }
}

#[allow(dead_code)]
pub enum Opcode {
    SPECIAL = 0,
    REGIMM = 1,
    J = 2,
    JAL = 3,
    BEQ = 4,
    BNE = 5,
    BLEZ = 6,
    BGTZ = 7,
    ADDI = 8,
    ADDIU = 9,
    SLTI = 10,
    SLTIU = 11,
    ANDI = 12,
    ORI = 13,
    XORI = 14,
    LUI = 15,
    COP0 = 16,
    COP1 = 17,
    COP2 = 18,
    COP3 = 19,
    BEQL = 20,
    BNEL = 21,
    BLEZL = 22,
    BGTZL = 23,
    DADDI = 24,
    DADDIU = 25,
    LDL = 26,
    LDR = 27,
    _I28 = 28,
    LB = 32,
    LH = 33,
    LWL = 34,
    LW = 35,
    LBU = 36,
    LHU = 37,
    LWR = 38,
    LWU = 39,
    SB = 40,
    SH = 41,
    SWL = 42,
    SW = 43,
    SDL = 44,
    SDR = 45,
    SWR = 46,
    CACHE = 47,
    LL = 48,
    LWC1 = 49,
    LLD = 52,
    LDC1 = 53,
    LD = 55,
    SC = 56,
    SWC1 = 57,
    SCD = 60,
    SDC1 = 61,
    SD = 63,
}

#[allow(dead_code)]
pub enum SpecialOpcode {
    SLL = 0,
    SRL = 2,
    SRA = 3,
    SLLV = 4,
    SRLV = 6,
    SRAV = 7,
    JR = 8,
    JALR = 9,
    SYSCALL = 12,
    BREAK = 13,
    SYNC = 15,
    MFHI = 16,
    MTHI = 17,
    MFLO = 18,
    MTLO = 19,
    DSLLV = 20,
    DSRLV = 22,
    DSRAV = 23,
    MULT = 24,
    MULTU = 25,
    DIV = 26,
    DIVU = 27,
    DMULT = 28,
    DMULTU = 29,
    DDIV = 30,
    DDIVU = 31,
    ADD = 32,
    ADDU = 33,
    SUB = 34,
    SUBU = 35,
    AND = 36,
    OR = 37,
    XOR = 38,
    NOR = 39,
    SLT = 42,
    SLTU = 43,
    DADD = 44,
    DADDU = 45,
    DSUB = 46,
    DSUBU = 47,
    TGE = 48,
    TGEU = 49,
    TLT = 50,
    TLTU = 51,
    TEQ = 52,
    TNE = 54,
    DSLL = 56,
    DSRL = 58,
    DSRA = 59,
    DSLL32 = 60,
    DSRL32 = 62,
    DSRA32 = 63,
}

#[allow(dead_code)]
pub enum RegimmOpcode {
    BLTZ = 0,
    BGEZ = 1,
    BLTZL = 2,
    BGEZL = 3,
    TGEI = 8,
    TGEIU = 9,
    TLTI = 10,
    TLTIU = 11,
    TEQI = 12,
    TNEI = 14,
    BLTZAL = 16,
    BGEZAL = 17,
    BLTZALL = 18,
    BGEZALL = 19,
}

#[allow(dead_code)]
pub enum Cop0Opcode {
    MFC0 = 0,
    DMFC0 = 1,
    MTC0 = 4,
    DMTC0 = 5,
    TLB = 16,
}

#[allow(dead_code)]
pub enum Cop0TLBInstruction {
    TLBR = 1,
    TLBWI = 2,
    TLBWR = 6,
    TLBP = 8,
    ERET = 24,
}

#[allow(dead_code)]
pub enum Cop1Opcode {
    MFC1 = 0,
    DMFC1 = 1,
    CFC1 = 2,
    _DCFC1 = 3,
    MTC1 = 4,
    DMTC1 = 5,
    CTC1 = 6,
    _DCTC1 = 7,
    BC1 = 8,
    S = 16,
    D = 17,
    W = 20,
    L = 21,
}

#[bitenum(u6, exhaustive: true)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
pub enum Cop1FloatInstruction {
    ADD = 0,
    SUB = 1,
    MUL = 2,
    DIV = 3,
    SQRT = 4,
    ABS = 5,
    MOV = 6,
    NEG = 7,
    ROUND_L = 8,
    TRUNC_L = 9,
    CEIL_L = 10,
    FLOOR_L = 11,
    ROUND_W = 12,
    TRUNC_W = 13,
    CEIL_W = 14,
    FLOOR_W = 15,
    _F16 = 16,
    _Invalid_17 = 17,
    _Invalid_18 = 18,
    _Invalid_19 = 19,
    _Invalid_20 = 20,
    _Invalid_21 = 21,
    _Invalid_22 = 22,
    _Invalid_23 = 23,
    _Invalid_24 = 24,
    _Invalid_25 = 25,
    _Invalid_26 = 26,
    _Invalid_27 = 27,
    _Invalid_28 = 28,
    _Invalid_29 = 29,
    _Invalid_30 = 30,
    _F31 = 31,
    CVT_S = 32,
    CVT_D = 33,
    _F34 = 34,
    _F35 = 35,
    CVT_W = 36,
    CVT_L = 37,
    _F38 = 38,
    _Invalid_39 = 39,
    _Invalid_40 = 40,
    _Invalid_41 = 41,
    _Invalid_42 = 42,
    _Invalid_43 = 43,
    _Invalid_44 = 44,
    _Invalid_45 = 45,
    _Invalid_46 = 46,
    _F47 = 47,
    C_F = 48,
    C_UN = 49,
    C_EQ = 50,
    C_UEQ = 51,
    C_OLT = 52,
    C_ULT = 53,
    C_OLE = 54,
    C_ULE = 55,
    C_SF = 56,
    C_NGLE = 57,
    C_SEQ = 58,
    C_NGL = 59,
    C_LT = 60,
    C_NGE = 61,
    C_LE = 62,
    C_NGT = 63,
}

#[bitenum(u6, exhaustive: false)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum Cop1Condition {
    F = 48,
    UN = 49,
    EQ = 50,
    UEQ = 51,
    OLT = 52,
    ULT = 53,
    OLE = 54,
    ULE = 55,
    SF = 56,
    NGLE = 57,
    SEQ = 58,
    NGL = 59,
    LT = 60,
    NGE = 61,
    LE = 62,
    NGT = 63,
}

impl Step for Cop1Condition {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if (*start as usize) < (*end as usize) {
            Some(*end as usize - *start as usize)
        } else {
            None
        }
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        let next = start.raw_value().value() as usize + count;
        if next >= 48 && next < 64 {
            Self::new_with_raw_value(u6::new(next as u8)).ok()
        } else {
            None
        }
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        let next = start.raw_value().value() as usize - count;
        if next >= 48 && next < 64 {
            Self::new_with_raw_value(u6::new(next as u8)).ok()
        } else {
            None
        }
    }
}

#[allow(dead_code)]
pub enum Cop2Opcode {
    MFC2 = 0,
    DMFC2 = 1,
    CFC2 = 2,
    _DCFC2 = 3,
    MTC2 = 4,
    DMTC2 = 5,
    CTC2 = 6,
    _DCTC2 = 7,
}

#[allow(dead_code)]
pub enum Cop3Opcode {
    MFC3 = 0,
    DMFC3 = 1,
    MTC3 = 4,
    DMTC3 = 5,
}

pub struct FPUFloatInstruction {
    value: u32,
}

impl FPUFloatInstruction {
    const fn new(value: u32) -> Self {
        assert!((value >> 21) & 0b11111 == 0);

        Self { value }
    }

    pub const fn s(&self) -> u32 {
        self.value | ((Cop1Opcode::S as u32) << 21)
    }
    pub const fn d(&self) -> u32 {
        self.value | ((Cop1Opcode::D as u32) << 21)
    }
    pub const fn w(&self) -> u32 {
        self.value | ((Cop1Opcode::W as u32) << 21)
    }
    pub const fn l(&self) -> u32 {
        self.value | ((Cop1Opcode::L as u32) << 21)
    }
}

pub struct Assembler {}

impl Assembler {
    pub const fn make_main_immediate(op: Opcode, rt: GPR, rs: GPR, imm: u16) -> u32 {
        (imm as u32)
            | ((rt.raw_value().value() as u32) << 16)
            | ((rs.raw_value().value() as u32) << 21)
            | ((op as u32) << 26)
    }

    pub const fn make_special(op: SpecialOpcode, sa: u5, rd: u5, rs: u5, rt: u5) -> u32 {
        (op as u32)
            | ((sa.value() as u32) << 6)
            | ((rd.value() as u32) << 11)
            | ((rt.value() as u32) << 16)
            | ((rs.value() as u32) << 21)
            | ((Opcode::SPECIAL as u32) << 26)
    }

    const fn make_regimm(op: RegimmOpcode, rs: GPR, imm: u16) -> u32 {
        (imm as u32)
            | ((op as u32) << 16)
            | ((rs.raw_value().value() as u32) << 21)
            | ((Opcode::REGIMM as u32) << 26)
    }

    pub const fn make_regimm_trap(op: RegimmOpcode, rs: u5, imm: u16) -> u32 {
        Self::make_regimm(op, GPR::new_with_raw_value(rs), imm)
    }

    const fn make_cop0instruction(instruction: Cop0Opcode, rt: u5, rd: u5) -> u32 {
        ((rd.value() as u32) << 11)
            | ((rt.value() as u32) << 16)
            | ((instruction as u32) << 21)
            | ((Opcode::COP0 as u32) << 26)
    }

    const fn make_cop0tlbinstruction(instruction: Cop0TLBInstruction) -> u32 {
        (instruction as u32) | ((Cop0Opcode::TLB as u32) << 21) | ((Opcode::COP0 as u32) << 26)
    }

    const fn make_cop1instruction(instruction: Cop1Opcode, rt: u5, rd: u5) -> u32 {
        ((rd.value() as u32) << 11)
            | ((rt.value() as u32) << 16)
            | ((instruction as u32) << 21)
            | ((Opcode::COP1 as u32) << 26)
    }

    const fn make_cop2instruction(instruction: Cop2Opcode, rt: u5, rd: u5) -> u32 {
        ((rd.value() as u32) << 11)
            | ((rt.value() as u32) << 16)
            | ((instruction as u32) << 21)
            | ((Opcode::COP2 as u32) << 26)
    }

    const fn make_cop3instruction(instruction: Cop3Opcode, rt: u5, rd: u5) -> u32 {
        ((rd.value() as u32) << 11)
            | ((rt.value() as u32) << 16)
            | ((instruction as u32) << 21)
            | ((Opcode::COP3 as u32) << 26)
    }

    pub const fn make_cop1_float_instruction(
        instruction: Cop1FloatInstruction,
        fd: FR,
        fs: FR,
        ft: FR,
    ) -> FPUFloatInstruction {
        FPUFloatInstruction::new(
            (instruction as u32)
                | ((fd.raw_value().value() as u32) << 6)
                | ((fs.raw_value().value() as u32) << 11)
                | ((ft.raw_value().value() as u32) << 16)
                | ((Opcode::COP1 as u32) << 26),
        )
    }

    pub const fn make_lui(rt: GPR, immediate: i16) -> u32 {
        Self::make_lui_with_rs(rt, GPR::R0, immediate)
    }

    pub const fn make_lui_with_rs(rt: GPR, rs: GPR, immediate: i16) -> u32 {
        Self::make_main_immediate(Opcode::LUI, rt, rs, immediate as u16)
    }

    pub const fn make_jal(imm26: u26) -> u32 {
        (imm26.value()) | ((Opcode::JAL as u32) << 26)
    }

    pub const fn make_addi(rt: GPR, rs: GPR, immediate: i16) -> u32 {
        Self::make_main_immediate(Opcode::ADDI, rt, rs, immediate as u16)
    }

    pub const fn make_addiu(rt: GPR, rs: GPR, immediate: i16) -> u32 {
        Self::make_main_immediate(Opcode::ADDIU, rt, rs, immediate as u16)
    }

    pub const fn make_daddi(rt: GPR, rs: GPR, immediate: i16) -> u32 {
        Self::make_main_immediate(Opcode::DADDI, rt, rs, immediate as u16)
    }

    pub const fn make_daddiu(rt: GPR, rs: GPR, immediate: i16) -> u32 {
        Self::make_main_immediate(Opcode::DADDIU, rt, rs, immediate as u16)
    }

    pub const fn make_slti(rt: GPR, rs: GPR, immediate: i16) -> u32 {
        Self::make_main_immediate(Opcode::SLTI, rt, rs, immediate as u16)
    }

    pub const fn make_sltiu(rt: GPR, rs: GPR, immediate: i16) -> u32 {
        Self::make_main_immediate(Opcode::SLTIU, rt, rs, immediate as u16)
    }

    pub const fn make_andi(rt: GPR, rs: GPR, immediate: u16) -> u32 {
        Self::make_main_immediate(Opcode::ANDI, rt, rs, immediate)
    }

    pub const fn make_ori(rt: GPR, rs: GPR, immediate: u16) -> u32 {
        Self::make_main_immediate(Opcode::ORI, rt, rs, immediate)
    }

    pub const fn make_xori(rt: GPR, rs: GPR, immediate: u16) -> u32 {
        Self::make_main_immediate(Opcode::XORI, rt, rs, immediate)
    }

    pub const fn make_b(offset_as_instruction_count: i16) -> u32 {
        Self::make_beq(GPR::R0, GPR::R0, offset_as_instruction_count)
    }

    pub const fn make_beq(rt: GPR, rs: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_main_immediate(Opcode::BEQ, rt, rs, offset_as_instruction_count as u16)
    }

    pub const fn make_beql(rt: GPR, rs: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_main_immediate(Opcode::BEQL, rt, rs, offset_as_instruction_count as u16)
    }

    pub const fn make_bne(rt: GPR, rs: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_main_immediate(Opcode::BNE, rt, rs, offset_as_instruction_count as u16)
    }

    pub const fn make_bnel(rt: GPR, rs: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_main_immediate(Opcode::BNEL, rt, rs, offset_as_instruction_count as u16)
    }

    pub const fn make_bgtz(rs: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_bgtz_with_extras(rs, GPR::R0, offset_as_instruction_count)
    }

    pub const fn make_bgtz_with_extras(rs: GPR, rt: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_main_immediate(Opcode::BGTZ, rt, rs, offset_as_instruction_count as u16)
    }

    pub const fn make_bgtzl(rs: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_bgtzl_with_extras(rs, GPR::R0, offset_as_instruction_count)
    }

    pub const fn make_bgtzl_with_extras(rs: GPR, rt: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_main_immediate(Opcode::BGTZL, rt, rs, offset_as_instruction_count as u16)
    }

    pub const fn make_blez(rs: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_blez_with_extras(rs, GPR::R0, offset_as_instruction_count)
    }

    pub const fn make_blez_with_extras(rs: GPR, rt: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_main_immediate(Opcode::BLEZ, rt, rs, offset_as_instruction_count as u16)
    }

    pub const fn make_blezl(rs: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_blezl_with_extras(rs, GPR::R0, offset_as_instruction_count)
    }

    pub const fn make_blezl_with_extras(rs: GPR, rt: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_main_immediate(Opcode::BLEZL, rt, rs, offset_as_instruction_count as u16)
    }

    pub const fn make_add(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::ADD,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_addu(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::ADDU,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_sub(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::SUB,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_subu(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::SUBU,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_and(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::AND,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_or(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::OR,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_xor(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::XOR,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_nor(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::NOR,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_slt(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::SLT,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_sltu(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::SLTU,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dadd(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DADD,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_daddu(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DADDU,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dsub(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DSUB,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dsubu(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DSUBU,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_tne(rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::TNE,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    /// emux is an emulator only instruction, which uses TNE with the same registers.
    /// See https://hackmd.io/@rasky/r1k7na6Jn
    #[allow(dead_code)]
    pub const fn make_emux(r: GPR, function: EMUXFunction) -> u32 {
        (SpecialOpcode::TNE as u32)
            | ((function.raw_value().value() as u32) << 6)
            | ((r.raw_value().value() as u32) << 16)
            | ((r.raw_value().value() as u32) << 21)
            | ((Opcode::SPECIAL as u32) << 26)
    }

    pub const fn make_teq(rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::TEQ,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_tge(rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::TGE,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_tlt(rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::TLT,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_tgeu(rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::TGEU,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_tltu(rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::TLTU,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_jr(rs: GPR) -> u32 {
        Self::make_jr_with_extras(rs, GPR::R0)
    }

    pub const fn make_jr_with_extras(rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::JR,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_jalr(return_reg: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::JALR,
            u5::new(0),
            return_reg.raw_value(),
            rs.raw_value(),
            u5::new(0),
        )
    }

    pub const fn make_sll(rd: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_sll_with_extras(rd, rt, GPR::R0, sa)
    }

    pub const fn make_sll_with_extras(rd: GPR, rt: GPR, rs: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::SLL,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_nop() -> u32 {
        Self::make_sll(GPR::R0, GPR::R0, u5::new(0))
    }

    pub const fn make_srl(rd: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_srl_with_extras(rd, rt, GPR::R0, sa)
    }

    pub const fn make_srl_with_extras(rd: GPR, rt: GPR, rs: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::SRL,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_sra(rd: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_sra_with_extras(rd, rt, GPR::R0, sa)
    }

    pub const fn make_sra_with_extras(rd: GPR, rt: GPR, rs: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::SRA,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dsll(rd: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_dsll_with_extras(rd, rt, GPR::R0, sa)
    }

    pub const fn make_dsll_with_extras(rd: GPR, rt: GPR, rs: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::DSLL,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dsrl(rd: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_dsrl_with_extras(rd, rt, GPR::R0, sa)
    }

    pub const fn make_dsrl_with_extras(rd: GPR, rt: GPR, rs: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::DSRL,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dsra(rd: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_dsra_with_extras(rd, rt, GPR::R0, sa)
    }

    pub const fn make_dsra_with_extras(rd: GPR, rt: GPR, rs: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::DSRA,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dsll32(rd: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_dsll32_with_extras(rd, rt, GPR::R0, sa)
    }

    pub const fn make_dsll32_with_extras(rd: GPR, rt: GPR, rs: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::DSLL32,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dsrl32(rd: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_dsrl32_with_extras(rd, rt, GPR::R0, sa)
    }

    pub const fn make_dsrl32_with_extras(rd: GPR, rt: GPR, rs: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::DSRL32,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dsra32(rd: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_dsra32_with_extras(rd, rt, GPR::R0, sa)
    }

    pub const fn make_dsra32_with_extras(rd: GPR, rt: GPR, rs: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::DSRA32,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_sllv(rd: GPR, rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::SLLV,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_srlv(rd: GPR, rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::SRLV,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_srav(rd: GPR, rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::SRAV,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dsllv(rd: GPR, rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DSLLV,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dsrlv(rd: GPR, rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DSRLV,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dsrav(rd: GPR, rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DSRAV,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_div(rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DIV,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_divu(rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DIVU,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_ddiv(rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DDIV,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_ddivu(rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DDIVU,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_mult(rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::MULT,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_multu(rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::MULTU,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dmult(rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DMULT,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_dmultu(rt: GPR, rs: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::DMULTU,
            u5::new(0),
            u5::new(0),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_mflo(rd: GPR) -> u32 {
        Self::make_mflo_with_extras(rd, GPR::R0, GPR::R0)
    }

    pub const fn make_mflo_with_extras(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::MFLO,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_mtlo(rs: GPR) -> u32 {
        Self::make_mtlo_with_extras(GPR::R0, rs, GPR::R0)
    }

    pub const fn make_mtlo_with_extras(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::MTLO,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_sync() -> u32 {
        Self::make_sync_with_extras(GPR::R0, GPR::R0, GPR::R0, u5::new(0))
    }

    pub const fn make_sync_with_extras(rd: GPR, rs: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::SYNC,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_break() -> u32 {
        Self::make_break_with_extras(GPR::R0, GPR::R0, GPR::R0, u5::new(0))
    }

    pub const fn make_break_with_extras(rd: GPR, rs: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::BREAK,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_syscall() -> u32 {
        Self::make_syscall_with_extras(GPR::R0, GPR::R0, GPR::R0, u5::new(0))
    }

    pub const fn make_syscall_with_extras(rd: GPR, rs: GPR, rt: GPR, sa: u5) -> u32 {
        Self::make_special(
            SpecialOpcode::SYSCALL,
            sa,
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_bgezal(rs: GPR, offset_as_instruction_count: i16) -> u32 {
        Self::make_regimm(RegimmOpcode::BGEZAL, rs, offset_as_instruction_count as u16)
    }

    pub const fn make_mfhi(rd: GPR) -> u32 {
        Self::make_mfhi_with_extras(rd, GPR::R0, GPR::R0)
    }

    pub const fn make_mfhi_with_extras(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::MFHI,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_mthi(rs: GPR) -> u32 {
        Self::make_mthi_with_extras(GPR::R0, rs, GPR::R0)
    }

    pub const fn make_mthi_with_extras(rd: GPR, rs: GPR, rt: GPR) -> u32 {
        Self::make_special(
            SpecialOpcode::MTHI,
            u5::new(0),
            rd.raw_value(),
            rs.raw_value(),
            rt.raw_value(),
        )
    }

    pub const fn make_cop0_tlbr() -> u32 {
        Self::make_cop0tlbinstruction(Cop0TLBInstruction::TLBR)
    }

    pub const fn make_cop0_tlbp() -> u32 {
        Self::make_cop0tlbinstruction(Cop0TLBInstruction::TLBP)
    }

    pub const fn make_cop0_tlbwi() -> u32 {
        Self::make_cop0tlbinstruction(Cop0TLBInstruction::TLBWI)
    }

    pub const fn make_cop0_tlbwr() -> u32 {
        Self::make_cop0tlbinstruction(Cop0TLBInstruction::TLBWR)
    }

    pub const fn make_cop1_c_cond(condition: Cop1Condition, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(
            Cop1FloatInstruction::new_with_raw_value(condition.raw_value()),
            FR::F0,
            fs,
            ft,
        )
    }

    pub const fn make_cfc1(rt: GPR, rd: u5) -> u32 {
        Self::make_cop1instruction(Cop1Opcode::CFC1, rt.raw_value(), rd)
    }

    pub const fn make_dcfc1(rt: GPR, rd: u5) -> u32 {
        Self::make_cop1instruction(Cop1Opcode::_DCFC1, rt.raw_value(), rd)
    }

    pub const fn make_ctc1(rt: GPR, rd: u5) -> u32 {
        Self::make_cop1instruction(Cop1Opcode::CTC1, rt.raw_value(), rd)
    }

    pub const fn make_dctc1(rt: GPR, rd: u5) -> u32 {
        Self::make_cop1instruction(Cop1Opcode::_DCTC1, rt.raw_value(), rd)
    }

    pub const fn make_cop1_abs(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_abs_with_ft(fd, fs, FR::F0)
    }

    pub const fn make_cop1_abs_with_ft(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::ABS, fd, fs, ft)
    }

    pub const fn make_cop1_add(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::ADD, fd, fs, ft)
    }

    pub const fn make_cop1_cvt_d(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_cvt_d_with_ft(fd, fs, FR::F0)
    }

    pub const fn make_cop1_cvt_d_with_ft(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::CVT_D, fd, fs, ft)
    }

    pub const fn make_cop1_cvt_l(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_cvt_l_with_ft(fd, fs, FR::F0)
    }

    pub const fn make_cop1_cvt_l_with_ft(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::CVT_L, fd, fs, ft)
    }

    pub const fn make_cop1_cvt_s(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_cvt_s_with_ft(fd, fs, FR::F0)
    }

    pub const fn make_cop1_cvt_s_with_ft(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::CVT_S, fd, fs, ft)
    }

    pub const fn make_cop1_cvt_w(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_cvt_w_with_ft(fd, fs, FR::F0)
    }

    pub const fn make_cop1_cvt_w_with_ft(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::CVT_W, fd, fs, ft)
    }

    pub const fn make_cop1_round_w(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::ROUND_W, fd, fs, FR::F0)
    }

    pub const fn make_cop1_round_l(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::ROUND_L, fd, fs, FR::F0)
    }

    pub const fn make_cop1_trunc_w(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::TRUNC_W, fd, fs, FR::F0)
    }

    pub const fn make_cop1_trunc_l(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::TRUNC_L, fd, fs, FR::F0)
    }

    pub const fn make_cop1_floor_w(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::FLOOR_W, fd, fs, FR::F0)
    }

    pub const fn make_cop1_floor_l(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::FLOOR_L, fd, fs, FR::F0)
    }

    pub const fn make_cop1_ceil_w(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::CEIL_W, fd, fs, FR::F0)
    }

    pub const fn make_cop1_ceil_l(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::CEIL_L, fd, fs, FR::F0)
    }

    pub const fn make_cop1_div(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::DIV, fd, fs, ft)
    }

    pub const fn make_cop1_mov(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_mov_with_ft(fd, fs, FR::F0)
    }

    pub const fn make_cop1_mov_with_ft(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::MOV, fd, fs, ft)
    }

    pub const fn make_cop1_mul(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::MUL, fd, fs, ft)
    }

    pub const fn make_cop1_neg(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_neg_with_ft(fd, fs, FR::F0)
    }

    pub const fn make_cop1_neg_with_ft(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::NEG, fd, fs, ft)
    }

    pub const fn make_cop1_sqrt(fd: FR, fs: FR) -> FPUFloatInstruction {
        Self::make_cop1_sqrt_with_ft(fd, fs, FR::F0)
    }

    pub const fn make_cop1_sqrt_with_ft(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::SQRT, fd, fs, ft)
    }

    pub const fn make_cop1_sub(fd: FR, fs: FR, ft: FR) -> FPUFloatInstruction {
        Self::make_cop1_float_instruction(Cop1FloatInstruction::SUB, fd, fs, ft)
    }

    pub const fn make_sd(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::SD, rt, base, offset as u16)
    }

    pub const fn make_scd(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::SCD, rt, base, offset as u16)
    }

    pub const fn make_sdl(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::SDL, rt, base, offset as u16)
    }

    pub const fn make_sdr(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::SDR, rt, base, offset as u16)
    }

    pub const fn make_sw(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::SW, rt, base, offset as u16)
    }

    pub const fn make_sc(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::SC, rt, base, offset as u16)
    }

    pub const fn make_swl(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::SWL, rt, base, offset as u16)
    }

    pub const fn make_swr(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::SWR, rt, base, offset as u16)
    }

    pub const fn make_sh(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::SH, rt, base, offset as u16)
    }

    pub const fn make_sb(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::SB, rt, base, offset as u16)
    }

    pub const fn make_lb(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LB, rt, base, offset as u16)
    }

    pub const fn make_lbu(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LBU, rt, base, offset as u16)
    }

    pub const fn make_lh(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LH, rt, base, offset as u16)
    }

    pub const fn make_lhu(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LHU, rt, base, offset as u16)
    }

    pub const fn make_lw(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LW, rt, base, offset as u16)
    }

    pub const fn make_lwl(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LWL, rt, base, offset as u16)
    }

    pub const fn make_lwr(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LWR, rt, base, offset as u16)
    }

    pub const fn make_ldl(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LDL, rt, base, offset as u16)
    }

    pub const fn make_ldr(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LDR, rt, base, offset as u16)
    }

    pub const fn make_ll(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LL, rt, base, offset as u16)
    }

    pub const fn make_lld(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LLD, rt, base, offset as u16)
    }

    pub const fn make_lwu(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LWU, rt, base, offset as u16)
    }

    pub const fn make_ld(rt: GPR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(Opcode::LD, rt, base, offset as u16)
    }

    pub const fn make_cache(op: CacheOp, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(
            Opcode::CACHE,
            GPR::new_with_raw_value(op.raw_value()),
            base,
            offset as u16,
        )
    }

    pub const fn make_mfc0(rt: GPR, rd: RegisterIndex) -> u32 {
        Self::make_cop0instruction(Cop0Opcode::MFC0, rt.raw_value(), rd.raw_value())
    }

    pub const fn make_dmfc0(rt: GPR, rd: RegisterIndex) -> u32 {
        Self::make_cop0instruction(Cop0Opcode::DMFC0, rt.raw_value(), rd.raw_value())
    }

    pub const fn make_mtc0(rt: GPR, rd: RegisterIndex) -> u32 {
        Self::make_cop0instruction(Cop0Opcode::MTC0, rt.raw_value(), rd.raw_value())
    }

    pub const fn make_dmtc0(rt: GPR, rd: RegisterIndex) -> u32 {
        Self::make_cop0instruction(Cop0Opcode::DMTC0, rt.raw_value(), rd.raw_value())
    }

    pub const fn make_lwc1(rt: FR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(
            Opcode::LWC1,
            GPR::new_with_raw_value(rt.raw_value()),
            base,
            offset as u16,
        )
    }

    pub const fn make_ldc1(rt: FR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(
            Opcode::LDC1,
            GPR::new_with_raw_value(rt.raw_value()),
            base,
            offset as u16,
        )
    }

    pub const fn make_swc1(rt: FR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(
            Opcode::SWC1,
            GPR::new_with_raw_value(rt.raw_value()),
            base,
            offset as u16,
        )
    }

    pub const fn make_sdc1(rt: FR, offset: i16, base: GPR) -> u32 {
        Self::make_main_immediate(
            Opcode::SDC1,
            GPR::new_with_raw_value(rt.raw_value()),
            base,
            offset as u16,
        )
    }

    pub const fn make_mfc1(rt: GPR, rd: FR) -> u32 {
        Self::make_cop1instruction(Cop1Opcode::MFC1, rt.raw_value(), rd.raw_value())
    }

    pub const fn make_mtc1(rt: GPR, rd: FR) -> u32 {
        Self::make_cop1instruction(Cop1Opcode::MTC1, rt.raw_value(), rd.raw_value())
    }

    pub const fn make_dmfc1(rt: GPR, rd: FR) -> u32 {
        Self::make_cop1instruction(Cop1Opcode::DMFC1, rt.raw_value(), rd.raw_value())
    }

    pub const fn make_dmtc1(rt: GPR, rd: FR) -> u32 {
        Self::make_cop1instruction(Cop1Opcode::DMTC1, rt.raw_value(), rd.raw_value())
    }

    pub const fn make_mfc2(rt: GPR, rd: u5) -> u32 {
        Self::make_cop2instruction(Cop2Opcode::MFC2, rt.raw_value(), rd)
    }

    pub const fn make_mtc2(rt: GPR, rd: u5) -> u32 {
        Self::make_cop2instruction(Cop2Opcode::MTC2, rt.raw_value(), rd)
    }

    pub const fn make_dmfc2(rt: GPR, rd: u5) -> u32 {
        Self::make_cop2instruction(Cop2Opcode::DMFC2, rt.raw_value(), rd)
    }

    pub const fn make_dmtc2(rt: GPR, rd: u5) -> u32 {
        Self::make_cop2instruction(Cop2Opcode::DMTC2, rt.raw_value(), rd)
    }

    pub const fn make_cfc2(rt: GPR, rd: u5) -> u32 {
        Self::make_cop2instruction(Cop2Opcode::CFC2, rt.raw_value(), rd)
    }

    pub const fn make_ctc2(rt: GPR, rd: u5) -> u32 {
        Self::make_cop2instruction(Cop2Opcode::CTC2, rt.raw_value(), rd)
    }

    pub const fn make_dcfc2(rt: GPR, rd: u5) -> u32 {
        Self::make_cop2instruction(Cop2Opcode::_DCFC2, rt.raw_value(), rd)
    }

    pub const fn make_dctc2(rt: GPR, rd: u5) -> u32 {
        Self::make_cop2instruction(Cop2Opcode::_DCTC2, rt.raw_value(), rd)
    }

    pub const fn make_mfc3(rt: GPR, rd: u5) -> u32 {
        Self::make_cop3instruction(Cop3Opcode::MFC3, rt.raw_value(), rd)
    }
}
