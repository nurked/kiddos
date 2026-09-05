//! The instruction subset: one enum, a decoder from the real AArch64
//! encodings, and the helpers the assembler and the disassembler share.
//!
//! Every encoding here is the genuine one, so a program assembled by
//! clang for a real ARM runs unchanged, and what a kid reads in `dis`
//! is what a real CPU would read too.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shift {
    Lsl,
    Lsr,
    Asr,
    Ror,
}

impl Shift {
    pub fn name(self) -> &'static str {
        match self {
            Shift::Lsl => "lsl",
            Shift::Lsr => "lsr",
            Shift::Asr => "asr",
            Shift::Ror => "ror",
        }
    }
    pub fn from_bits(b: u32) -> Shift {
        match b & 3 {
            0 => Shift::Lsl,
            1 => Shift::Lsr,
            2 => Shift::Asr,
            _ => Shift::Ror,
        }
    }
    pub fn bits(self) -> u32 {
        match self {
            Shift::Lsl => 0,
            Shift::Lsr => 1,
            Shift::Asr => 2,
            Shift::Ror => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Logic {
    And,
    Orr,
    Eor,
    Ands,
}

impl Logic {
    pub fn from_bits(b: u32) -> Logic {
        match b & 3 {
            0 => Logic::And,
            1 => Logic::Orr,
            2 => Logic::Eor,
            _ => Logic::Ands,
        }
    }
    pub fn bits(self) -> u32 {
        match self {
            Logic::And => 0,
            Logic::Orr => 1,
            Logic::Eor => 2,
            Logic::Ands => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wide {
    Movn,
    Movz,
    Movk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bitfield {
    Sbfm,
    Bfm,
    Ubfm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondSel {
    Csel,
    Csinc,
    Csinv,
    Csneg,
}

/// What a load or store moves: size 0..3 is 1, 2, 4, 8 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LdStOp {
    Store,
    /// Zero-extending load.
    Load,
    /// Sign-extending load into a 64-bit register.
    LoadS64,
    /// Sign-extending load into a 32-bit register.
    LoadS32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Addr {
    /// `[rn, #imm]`
    Off(i64),
    /// `[rn, #imm]!`
    Pre(i64),
    /// `[rn], #imm`
    Post(i64),
    /// `[rn, rm]` or `[rn, rm, lsl #s]` (option 3 = uxtx/lsl; others decode too)
    Reg { rm: u8, option: u8, scaled: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitKind {
    W,
    X,
    Sw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Insn {
    AddSubImm {
        sf: bool,
        sub: bool,
        flags: bool,
        rd: u8,
        rn: u8,
        imm: u32,
        shift12: bool,
    },
    AddSubReg {
        sf: bool,
        sub: bool,
        flags: bool,
        rd: u8,
        rn: u8,
        rm: u8,
        shift: Shift,
        amount: u8,
    },
    LogicReg {
        sf: bool,
        op: Logic,
        invert: bool,
        rd: u8,
        rn: u8,
        rm: u8,
        shift: Shift,
        amount: u8,
    },
    LogicImm {
        sf: bool,
        op: Logic,
        rd: u8,
        rn: u8,
        imm: u64,
        n: u32,
        immr: u32,
        imms: u32,
    },
    MovWide {
        sf: bool,
        kind: Wide,
        rd: u8,
        imm16: u16,
        hw: u8,
    },
    Bitfield {
        sf: bool,
        kind: Bitfield,
        rd: u8,
        rn: u8,
        immr: u8,
        imms: u8,
    },
    ShiftReg {
        sf: bool,
        shift: Shift,
        rd: u8,
        rn: u8,
        rm: u8,
    },
    Div {
        sf: bool,
        signed: bool,
        rd: u8,
        rn: u8,
        rm: u8,
    },
    MulAdd {
        sf: bool,
        sub: bool,
        rd: u8,
        rn: u8,
        rm: u8,
        ra: u8,
    },
    CondSel {
        sf: bool,
        kind: CondSel,
        rd: u8,
        rn: u8,
        rm: u8,
        cond: u8,
    },
    B {
        offset: i64,
    },
    Bl {
        offset: i64,
    },
    BCond {
        cond: u8,
        offset: i64,
    },
    Cbz {
        sf: bool,
        nonzero: bool,
        rt: u8,
        offset: i64,
    },
    Tbz {
        nonzero: bool,
        rt: u8,
        bit: u8,
        offset: i64,
    },
    Br {
        rn: u8,
    },
    Blr {
        rn: u8,
    },
    Ret {
        rn: u8,
    },
    Adr {
        page: bool,
        rd: u8,
        imm: i64,
    },
    Svc {
        imm: u16,
    },
    Brk {
        imm: u16,
    },
    Nop,
    LdSt {
        size: u8,
        op: LdStOp,
        rt: u8,
        rn: u8,
        addr: Addr,
    },
    LdStPair {
        sf: bool,
        load: bool,
        rt: u8,
        rt2: u8,
        rn: u8,
        imm: i64,
        addr: Addr,
    },
    LdrLit {
        kind: LitKind,
        rt: u8,
        offset: i64,
    },
    Unknown(u32),
}

pub const COND_NAMES: [&str; 16] = [
    "eq", "ne", "cs", "cc", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le", "al", "nv",
];

pub fn cond_from_name(s: &str) -> Option<u8> {
    match s {
        "hs" => return Some(2),
        "lo" => return Some(3),
        _ => {}
    }
    COND_NAMES.iter().position(|c| *c == s).map(|i| i as u8)
}

fn bits(x: u32, hi: u32, lo: u32) -> u32 {
    (x >> lo) & ((1u32 << (hi - lo + 1)) - 1)
}

fn sext(x: u64, bits: u32) -> i64 {
    let shift = 64 - bits;
    ((x << shift) as i64) >> shift
}

/// The ARM "bitmask immediate": `(N, immr, imms)` to the value it means,
/// or None if the triple is not a legal encoding.
pub fn decode_bitmask(n: u32, immr: u32, imms: u32, sf: bool) -> Option<u64> {
    let len = 31 - ((n << 6) | (!imms & 0x3f)).leading_zeros();
    if len < 1 || (!sf && len > 5) {
        return None;
    }
    let esize = 1u32 << len;
    let levels = esize - 1;
    let s = imms & levels;
    let r = immr & levels;
    if s == levels {
        return None;
    }
    let ones: u64 = if s + 1 == 64 { u64::MAX } else { (1u64 << (s + 1)) - 1 };
    let elem = if esize == 64 {
        ones.rotate_right(r)
    } else {
        let m = (1u64 << esize) - 1;
        ((ones >> r) | (ones << (esize - r))) & m
    };
    let mut v = 0u64;
    let mut i = 0;
    while i < 64 {
        v |= elem << i;
        i += esize;
    }
    Some(if sf { v } else { v & 0xffff_ffff })
}

/// The inverse: a value to `(N, immr, imms)`, if it can be written as one.
pub fn encode_bitmask(value: u64, sf: bool) -> Option<(u32, u32, u32)> {
    let value = if sf { value } else { value & 0xffff_ffff };
    let width = if sf { 64 } else { 32 };
    if value == 0 || value == (if sf { u64::MAX } else { 0xffff_ffff }) {
        return None;
    }
    let mut esize = 2u32;
    while esize <= width {
        let m = if esize == 64 { u64::MAX } else { (1u64 << esize) - 1 };
        let elem = value & m;
        let mut ok = true;
        let mut i = 0;
        while i < width {
            if (value >> i) & m != elem {
                ok = false;
                break;
            }
            i += esize;
        }
        if ok {
            let s = elem.count_ones();
            if s == 0 || s == esize {
                return None;
            }
            let ones: u64 = (1u64 << s) - 1;
            for r in 0..esize {
                let rot = if esize == 64 {
                    ones.rotate_right(r)
                } else {
                    ((ones >> r) | (ones << (esize - r))) & m
                };
                if rot == elem {
                    let n = (esize == 64) as u32;
                    let imms = ((!(2 * esize - 1)) & 0x3f) | (s - 1);
                    return Some((n, r, imms));
                }
            }
            return None;
        }
        esize *= 2;
    }
    None
}

/// Decode one 32-bit word. Never fails: unknown words come back as
/// `Insn::Unknown` so the disassembler can still show them.
pub fn decode(w: u32) -> Insn {
    let sf = bits(w, 31, 31) == 1;
    let rd = bits(w, 4, 0) as u8;
    let rn = bits(w, 9, 5) as u8;
    let rm = bits(w, 20, 16) as u8;

    // add/sub immediate: sf op S 10001 0 sh imm12 Rn Rd
    if bits(w, 28, 23) == 0b100010 {
        let shift12 = bits(w, 22, 22) == 1;
        return Insn::AddSubImm {
            sf,
            sub: bits(w, 30, 30) == 1,
            flags: bits(w, 29, 29) == 1,
            rd,
            rn,
            imm: bits(w, 21, 10),
            shift12,
        };
    }
    // logical immediate: sf opc 100100 N immr imms Rn Rd
    if bits(w, 28, 23) == 0b100100 {
        let n = bits(w, 22, 22);
        let immr = bits(w, 21, 16);
        let imms = bits(w, 15, 10);
        if !sf && n == 1 {
            return Insn::Unknown(w);
        }
        return match decode_bitmask(n, immr, imms, sf) {
            Some(imm) => Insn::LogicImm {
                sf,
                op: Logic::from_bits(bits(w, 30, 29)),
                rd,
                rn,
                imm,
                n,
                immr,
                imms,
            },
            None => Insn::Unknown(w),
        };
    }
    // move wide: sf opc 100101 hw imm16 Rd
    if bits(w, 28, 23) == 0b100101 {
        let hw = bits(w, 22, 21) as u8;
        if !sf && hw > 1 {
            return Insn::Unknown(w);
        }
        let kind = match bits(w, 30, 29) {
            0 => Wide::Movn,
            2 => Wide::Movz,
            3 => Wide::Movk,
            _ => return Insn::Unknown(w),
        };
        return Insn::MovWide {
            sf,
            kind,
            rd,
            imm16: bits(w, 20, 5) as u16,
            hw,
        };
    }
    // bitfield: sf opc 100110 N immr imms Rn Rd
    if bits(w, 28, 23) == 0b100110 {
        let n = bits(w, 22, 22) == 1;
        if n != sf {
            return Insn::Unknown(w);
        }
        let kind = match bits(w, 30, 29) {
            0 => Bitfield::Sbfm,
            1 => Bitfield::Bfm,
            2 => Bitfield::Ubfm,
            _ => return Insn::Unknown(w),
        };
        return Insn::Bitfield {
            sf,
            kind,
            rd,
            rn,
            immr: bits(w, 21, 16) as u8,
            imms: bits(w, 15, 10) as u8,
        };
    }
    // adr/adrp: op immlo 10000 immhi Rd
    if bits(w, 28, 24) == 0b10000 {
        let page = sf;
        let immlo = bits(w, 30, 29) as u64;
        let immhi = bits(w, 23, 5) as u64;
        let imm = sext((immhi << 2) | immlo, 21);
        return Insn::Adr {
            page,
            rd,
            imm: if page { imm << 12 } else { imm },
        };
    }
    // add/sub shifted register: sf op S 01011 shift 0 Rm imm6 Rn Rd
    if bits(w, 28, 24) == 0b01011 && bits(w, 21, 21) == 0 {
        let shift = Shift::from_bits(bits(w, 23, 22));
        if shift == Shift::Ror {
            return Insn::Unknown(w);
        }
        return Insn::AddSubReg {
            sf,
            sub: bits(w, 30, 30) == 1,
            flags: bits(w, 29, 29) == 1,
            rd,
            rn,
            rm,
            shift,
            amount: bits(w, 15, 10) as u8,
        };
    }
    // logical shifted register: sf opc 01010 shift N Rm imm6 Rn Rd
    if bits(w, 28, 24) == 0b01010 {
        return Insn::LogicReg {
            sf,
            op: Logic::from_bits(bits(w, 30, 29)),
            invert: bits(w, 21, 21) == 1,
            rd,
            rn,
            rm,
            shift: Shift::from_bits(bits(w, 23, 22)),
            amount: bits(w, 15, 10) as u8,
        };
    }
    // data-processing 2 source: sf 0 S 11010110 Rm opcode Rn Rd
    if bits(w, 30, 21) == 0b0011010110 {
        let opcode = bits(w, 15, 10);
        return match opcode {
            0b000010 => Insn::Div {
                sf,
                signed: false,
                rd,
                rn,
                rm,
            },
            0b000011 => Insn::Div {
                sf,
                signed: true,
                rd,
                rn,
                rm,
            },
            0b001000..=0b001011 => Insn::ShiftReg {
                sf,
                shift: Shift::from_bits(opcode & 3),
                rd,
                rn,
                rm,
            },
            _ => Insn::Unknown(w),
        };
    }
    // madd/msub: sf 00 11011 000 Rm o0 Ra Rn Rd
    if bits(w, 30, 21) == 0b0011011000 {
        return Insn::MulAdd {
            sf,
            sub: bits(w, 15, 15) == 1,
            rd,
            rn,
            rm,
            ra: bits(w, 14, 10) as u8,
        };
    }
    // conditional select: sf op S 11010100 Rm cond op2 Rn Rd
    if bits(w, 29, 21) == 0b011010100 && bits(w, 11, 11) == 0 {
        let op = bits(w, 30, 30);
        let o2 = bits(w, 10, 10);
        let kind = match (op, o2) {
            (0, 0) => CondSel::Csel,
            (0, 1) => CondSel::Csinc,
            (1, 0) => CondSel::Csinv,
            _ => CondSel::Csneg,
        };
        return Insn::CondSel {
            sf,
            kind,
            rd,
            rn,
            rm,
            cond: bits(w, 15, 12) as u8,
        };
    }
    // b / bl: op 00101 imm26
    if bits(w, 30, 26) == 0b00101 {
        let offset = sext(bits(w, 25, 0) as u64, 26) * 4;
        return if sf { Insn::Bl { offset } } else { Insn::B { offset } };
    }
    // b.cond: 01010100 imm19 0 cond
    if bits(w, 31, 24) == 0b01010100 && bits(w, 4, 4) == 0 {
        return Insn::BCond {
            cond: bits(w, 3, 0) as u8,
            offset: sext(bits(w, 23, 5) as u64, 19) * 4,
        };
    }
    // cbz/cbnz: sf 011010 op imm19 Rt
    if bits(w, 30, 25) == 0b011010 {
        return Insn::Cbz {
            sf,
            nonzero: bits(w, 24, 24) == 1,
            rt: rd,
            offset: sext(bits(w, 23, 5) as u64, 19) * 4,
        };
    }
    // tbz/tbnz: b5 011011 op b40 imm14 Rt
    if bits(w, 30, 25) == 0b011011 {
        return Insn::Tbz {
            nonzero: bits(w, 24, 24) == 1,
            rt: rd,
            bit: ((bits(w, 31, 31) << 5) | bits(w, 23, 19)) as u8,
            offset: sext(bits(w, 18, 5) as u64, 14) * 4,
        };
    }
    // br/blr/ret: 1101011 0 0 op 11111 0000 0 0 Rn 00000
    if bits(w, 31, 25) == 0b1101011 && bits(w, 20, 10) == 0b11111000000 && bits(w, 4, 0) == 0 {
        return match bits(w, 24, 21) {
            0b0000 => Insn::Br { rn },
            0b0001 => Insn::Blr { rn },
            0b0010 => Insn::Ret { rn },
            _ => Insn::Unknown(w),
        };
    }
    if w == 0xD503201F {
        return Insn::Nop;
    }
    // svc: 11010100 000 imm16 000 01
    if bits(w, 31, 21) == 0b11010100000 && bits(w, 4, 0) == 0b00001 {
        return Insn::Svc {
            imm: bits(w, 20, 5) as u16,
        };
    }
    // brk: 11010100 001 imm16 000 00
    if bits(w, 31, 21) == 0b11010100001 && bits(w, 4, 0) == 0 {
        return Insn::Brk {
            imm: bits(w, 20, 5) as u16,
        };
    }
    // ldr literal: opc 011 0 00 imm19 Rt
    if bits(w, 29, 24) == 0b011000 {
        let kind = match bits(w, 31, 30) {
            0 => LitKind::W,
            1 => LitKind::X,
            2 => LitKind::Sw,
            _ => return Insn::Unknown(w),
        };
        return Insn::LdrLit {
            kind,
            rt: rd,
            offset: sext(bits(w, 23, 5) as u64, 19) * 4,
        };
    }
    // load/store pair: opc 101 0 type L imm7 Rt2 Rn Rt
    if bits(w, 29, 26) == 0b1010 && bits(w, 30, 30) == 0 {
        let sf = bits(w, 31, 31) == 1;
        let scale = if sf { 8 } else { 4 };
        let imm = sext(bits(w, 21, 15) as u64, 7) * scale;
        let addr = match bits(w, 24, 23) {
            0b01 => Addr::Post(imm),
            0b10 => Addr::Off(imm),
            0b11 => Addr::Pre(imm),
            _ => return Insn::Unknown(w),
        };
        return Insn::LdStPair {
            sf,
            load: bits(w, 22, 22) == 1,
            rt: rd,
            rt2: bits(w, 14, 10) as u8,
            rn,
            imm,
            addr,
        };
    }
    // load/store register: size 111 0 xx opc ...
    if bits(w, 29, 27) == 0b111 && bits(w, 26, 26) == 0 {
        let size = bits(w, 31, 30) as u8;
        let opc = bits(w, 23, 22);
        let op = match (opc, size) {
            (0, _) => LdStOp::Store,
            (1, _) => LdStOp::Load,
            (2, 3) => return Insn::Unknown(w), // prfm
            (2, _) => LdStOp::LoadS64,
            (3, 0) | (3, 1) => LdStOp::LoadS32,
            _ => return Insn::Unknown(w),
        };
        let rt = rd;
        match bits(w, 25, 24) {
            0b01 => {
                let imm = (bits(w, 21, 10) as i64) << size;
                return Insn::LdSt {
                    size,
                    op,
                    rt,
                    rn,
                    addr: Addr::Off(imm),
                };
            }
            0b00 => {
                if bits(w, 21, 21) == 0 {
                    let imm = sext(bits(w, 20, 12) as u64, 9);
                    let addr = match bits(w, 11, 10) {
                        0b00 => Addr::Off(imm), // unscaled (ldur/stur)
                        0b01 => Addr::Post(imm),
                        0b11 => Addr::Pre(imm),
                        _ => return Insn::Unknown(w),
                    };
                    return Insn::LdSt { size, op, rt, rn, addr };
                }
                if bits(w, 11, 10) == 0b10 {
                    let option = bits(w, 15, 13) as u8;
                    if option & 2 == 0 {
                        return Insn::Unknown(w);
                    }
                    return Insn::LdSt {
                        size,
                        op,
                        rt,
                        rn,
                        addr: Addr::Reg {
                            rm,
                            option,
                            scaled: bits(w, 12, 12) == 1,
                        },
                    };
                }
                return Insn::Unknown(w);
            }
            _ => return Insn::Unknown(w),
        }
    }
    Insn::Unknown(w)
}

/// Is this word an unscaled `ldur`/`stur` rather than `ldr [rn, #imm]`?
/// The decoder folds both into `Addr::Off`; the disassembler asks here.
pub fn is_unscaled(w: u32) -> bool {
    bits(w, 29, 27) == 0b111 && bits(w, 26, 26) == 0 && bits(w, 25, 24) == 0 && bits(w, 21, 21) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmask_round_trips() {
        for v in [
            1u64,
            0xff,
            0xf0f0f0f0f0f0f0f0,
            0x7ffffffffffffffe,
            0x0000_0001_0000_0001,
            0xffff,
            0x8000_0000_0000_0000,
        ] {
            let (n, r, s) = encode_bitmask(v, true).unwrap_or_else(|| panic!("{v:x}"));
            assert_eq!(decode_bitmask(n, r, s, true), Some(v), "{v:x}");
        }
        for v in [1u64, 0xff00, 0x8000_0001, 0x0f0f_0f0f] {
            let (n, r, s) = encode_bitmask(v, false).unwrap_or_else(|| panic!("{v:x}"));
            assert_eq!(decode_bitmask(n, r, s, false), Some(v), "{v:x}");
        }
        assert!(encode_bitmask(0, true).is_none());
        assert!(encode_bitmask(u64::MAX, true).is_none());
        assert!(encode_bitmask(0x1234, true).is_none());
    }

    #[test]
    fn decodes_known_words() {
        assert_eq!(
            decode(0xD2800020),
            Insn::MovWide {
                sf: true,
                kind: Wide::Movz,
                rd: 0,
                imm16: 1,
                hw: 0
            }
        );
        assert_eq!(decode(0xD65F03C0), Insn::Ret { rn: 30 });
        assert_eq!(decode(0xD4000001), Insn::Svc { imm: 0 });
        assert_eq!(decode(0xD503201F), Insn::Nop);
        assert_eq!(
            decode(0xA9BF7BFD),
            Insn::LdStPair {
                sf: true,
                load: false,
                rt: 29,
                rt2: 30,
                rn: 31,
                imm: -16,
                addr: Addr::Pre(-16)
            }
        );
        assert_eq!(decode(0x14000001), Insn::B { offset: 4 });
        assert_eq!(decode(0x54000041), Insn::BCond { cond: 1, offset: 8 });
        assert_eq!(
            decode(0x91000400),
            Insn::AddSubImm {
                sf: true,
                sub: false,
                flags: false,
                rd: 0,
                rn: 0,
                imm: 1,
                shift12: false
            }
        );
        assert_eq!(
            decode(0xF9400020),
            Insn::LdSt {
                size: 3,
                op: LdStOp::Load,
                rt: 0,
                rn: 1,
                addr: Addr::Off(0)
            }
        );
        assert_eq!(
            decode(0x39400020),
            Insn::LdSt {
                size: 0,
                op: LdStOp::Load,
                rt: 0,
                rn: 1,
                addr: Addr::Off(0)
            }
        );
    }
}
