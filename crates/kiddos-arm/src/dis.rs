//! Words back into text. What `dis` prints, and what the debugger shows
//! when there is no source for an address.

use crate::insn::{self, Addr, Bitfield, CondSel, Insn, LdStOp, LitKind, Logic, Shift, Wide, COND_NAMES};

fn r(n: u8, sf: bool) -> String {
    match (n, sf) {
        (31, true) => "xzr".into(),
        (31, false) => "wzr".into(),
        (n, true) => format!("x{n}"),
        (n, false) => format!("w{n}"),
    }
}

/// Register `n` where 31 means the stack pointer.
fn rs(n: u8, sf: bool) -> String {
    match (n, sf) {
        (31, true) => "sp".into(),
        (31, false) => "wsp".into(),
        _ => r(n, sf),
    }
}

fn shift_suffix(shift: Shift, amount: u8) -> String {
    if amount == 0 && shift == Shift::Lsl {
        String::new()
    } else {
        format!(", {} #{amount}", shift.name())
    }
}

fn imm(v: i64) -> String {
    if (-9..=9).contains(&v) {
        format!("#{v}")
    } else if v < 0 {
        format!("#-0x{:x}", -v)
    } else {
        format!("#0x{v:x}")
    }
}

/// `target` shown as a label when one is known, else as an address.
pub fn format(word: u32, pc: u64, name_of: &dyn Fn(u64) -> String) -> String {
    let i = insn::decode(word);
    let target = |off: i64| name_of(pc.wrapping_add(off as u64));
    match i {
        Insn::AddSubImm {
            sf,
            sub,
            flags,
            rd,
            rn,
            imm: v,
            shift12,
        } => {
            let val = if shift12 { (v as i64) << 12 } else { v as i64 };
            if flags && rd == 31 {
                return format!("{} {}, {}", if sub { "cmp" } else { "cmn" }, rs(rn, sf), imm(val));
            }
            if !sub && !flags && val == 0 && (rd == 31 || rn == 31) {
                return format!("mov {}, {}", rs(rd, sf), rs(rn, sf));
            }
            let m = match (sub, flags) {
                (false, false) => "add",
                (false, true) => "adds",
                (true, false) => "sub",
                (true, true) => "subs",
            };
            format!(
                "{m} {}, {}, {}",
                if flags { r(rd, sf) } else { rs(rd, sf) },
                rs(rn, sf),
                imm(val)
            )
        }
        Insn::AddSubReg {
            sf,
            sub,
            flags,
            rd,
            rn,
            rm,
            shift,
            amount,
        } => {
            let sfx = shift_suffix(shift, amount);
            if flags && rd == 31 {
                return format!("{} {}, {}{sfx}", if sub { "cmp" } else { "cmn" }, r(rn, sf), r(rm, sf));
            }
            if sub && rn == 31 {
                return format!(
                    "{} {}, {}{sfx}",
                    if flags { "negs" } else { "neg" },
                    r(rd, sf),
                    r(rm, sf)
                );
            }
            let m = match (sub, flags) {
                (false, false) => "add",
                (false, true) => "adds",
                (true, false) => "sub",
                (true, true) => "subs",
            };
            format!("{m} {}, {}, {}{sfx}", r(rd, sf), r(rn, sf), r(rm, sf))
        }
        Insn::LogicReg {
            sf,
            op,
            invert,
            rd,
            rn,
            rm,
            shift,
            amount,
        } => {
            let sfx = shift_suffix(shift, amount);
            if op == Logic::Orr && rn == 31 && sfx.is_empty() {
                return format!("{} {}, {}", if invert { "mvn" } else { "mov" }, r(rd, sf), r(rm, sf));
            }
            if op == Logic::Ands && rd == 31 {
                return format!("tst {}, {}{sfx}", r(rn, sf), r(rm, sf));
            }
            let m = match (op, invert) {
                (Logic::And, false) => "and",
                (Logic::And, true) => "bic",
                (Logic::Orr, false) => "orr",
                (Logic::Orr, true) => "orn",
                (Logic::Eor, false) => "eor",
                (Logic::Eor, true) => "eon",
                (Logic::Ands, false) => "ands",
                (Logic::Ands, true) => "bics",
            };
            format!("{m} {}, {}, {}{sfx}", r(rd, sf), r(rn, sf), r(rm, sf))
        }
        Insn::LogicImm {
            sf, op, rd, rn, imm: v, ..
        } => {
            if op == Logic::Ands && rd == 31 {
                return format!("tst {}, #0x{v:x}", r(rn, sf));
            }
            if op == Logic::Orr && rn == 31 {
                return format!("mov {}, #0x{v:x}", rs(rd, sf));
            }
            let m = match op {
                Logic::And => "and",
                Logic::Orr => "orr",
                Logic::Eor => "eor",
                Logic::Ands => "ands",
            };
            let rd = if op == Logic::Ands { r(rd, sf) } else { rs(rd, sf) };
            format!("{m} {rd}, {}, #0x{v:x}", r(rn, sf))
        }
        Insn::MovWide {
            sf,
            kind,
            rd,
            imm16,
            hw,
        } => {
            let shift = hw as u32 * 16;
            match kind {
                Wide::Movz if hw == 0 || imm16 != 0 => {
                    let v = (imm16 as u64) << shift;
                    if hw == 0 {
                        format!("mov {}, {}", r(rd, sf), imm(v as i64))
                    } else {
                        format!("mov {}, #0x{v:x}", r(rd, sf))
                    }
                }
                Wide::Movn if hw == 0 || imm16 != 0 => {
                    let v = !((imm16 as u64) << shift);
                    let v = if sf { v as i64 } else { (v as u32) as i32 as i64 };
                    format!("mov {}, {}", r(rd, sf), imm(v))
                }
                _ => {
                    let m = match kind {
                        Wide::Movz => "movz",
                        Wide::Movn => "movn",
                        Wide::Movk => "movk",
                    };
                    if hw == 0 {
                        format!("{m} {}, #0x{imm16:x}", r(rd, sf))
                    } else {
                        format!("{m} {}, #0x{imm16:x}, lsl #{shift}", r(rd, sf))
                    }
                }
            }
        }
        Insn::Bitfield {
            sf,
            kind,
            rd,
            rn,
            immr,
            imms,
        } => {
            let bits = if sf { 64u8 } else { 32 };
            match (kind, immr, imms) {
                (Bitfield::Ubfm, _, s) if s == bits - 1 => format!("lsr {}, {}, #{immr}", r(rd, sf), r(rn, sf)),
                (Bitfield::Sbfm, _, s) if s == bits - 1 => format!("asr {}, {}, #{immr}", r(rd, sf), r(rn, sf)),
                (Bitfield::Ubfm, rr, s) if s + 1 == rr => format!("lsl {}, {}, #{}", r(rd, sf), r(rn, sf), bits - rr),
                (Bitfield::Ubfm, 0, 7) => format!("uxtb {}, {}", r(rd, false), r(rn, false)),
                (Bitfield::Ubfm, 0, 15) => format!("uxth {}, {}", r(rd, false), r(rn, false)),
                (Bitfield::Sbfm, 0, 7) => format!("sxtb {}, {}", r(rd, sf), r(rn, false)),
                (Bitfield::Sbfm, 0, 15) => format!("sxth {}, {}", r(rd, sf), r(rn, false)),
                (Bitfield::Sbfm, 0, 31) if sf => format!("sxtw {}, {}", r(rd, true), r(rn, false)),
                _ => {
                    let m = match kind {
                        Bitfield::Sbfm => "sbfm",
                        Bitfield::Bfm => "bfm",
                        Bitfield::Ubfm => "ubfm",
                    };
                    format!("{m} {}, {}, #{immr}, #{imms}", r(rd, sf), r(rn, sf))
                }
            }
        }
        Insn::ShiftReg { sf, shift, rd, rn, rm } => {
            format!("{} {}, {}, {}", shift.name(), r(rd, sf), r(rn, sf), r(rm, sf))
        }
        Insn::Div { sf, signed, rd, rn, rm } => format!(
            "{} {}, {}, {}",
            if signed { "sdiv" } else { "udiv" },
            r(rd, sf),
            r(rn, sf),
            r(rm, sf)
        ),
        Insn::MulAdd {
            sf,
            sub,
            rd,
            rn,
            rm,
            ra,
        } => {
            if ra == 31 {
                format!(
                    "{} {}, {}, {}",
                    if sub { "mneg" } else { "mul" },
                    r(rd, sf),
                    r(rn, sf),
                    r(rm, sf)
                )
            } else {
                format!(
                    "{} {}, {}, {}, {}",
                    if sub { "msub" } else { "madd" },
                    r(rd, sf),
                    r(rn, sf),
                    r(rm, sf),
                    r(ra, sf)
                )
            }
        }
        Insn::CondSel {
            sf,
            kind,
            rd,
            rn,
            rm,
            cond,
        } => {
            let c = COND_NAMES[cond as usize];
            let inv = COND_NAMES[(cond ^ 1) as usize];
            match kind {
                CondSel::Csinc if rn == 31 && rm == 31 && cond < 14 => format!("cset {}, {inv}", r(rd, sf)),
                CondSel::Csinv if rn == 31 && rm == 31 && cond < 14 => format!("csetm {}, {inv}", r(rd, sf)),
                CondSel::Csinc if rn == rm && cond < 14 => format!("cinc {}, {}, {inv}", r(rd, sf), r(rn, sf)),
                CondSel::Csneg if rn == rm && cond < 14 => format!("cneg {}, {}, {inv}", r(rd, sf), r(rn, sf)),
                _ => {
                    let m = match kind {
                        CondSel::Csel => "csel",
                        CondSel::Csinc => "csinc",
                        CondSel::Csinv => "csinv",
                        CondSel::Csneg => "csneg",
                    };
                    format!("{m} {}, {}, {}, {c}", r(rd, sf), r(rn, sf), r(rm, sf))
                }
            }
        }
        Insn::B { offset } => format!("b {}", target(offset)),
        Insn::Bl { offset } => format!("bl {}", target(offset)),
        Insn::BCond { cond, offset } => format!("b.{} {}", COND_NAMES[cond as usize], target(offset)),
        Insn::Cbz {
            sf,
            nonzero,
            rt,
            offset,
        } => format!(
            "{} {}, {}",
            if nonzero { "cbnz" } else { "cbz" },
            r(rt, sf),
            target(offset)
        ),
        Insn::Tbz {
            nonzero,
            rt,
            bit,
            offset,
        } => format!(
            "{} {}, #{bit}, {}",
            if nonzero { "tbnz" } else { "tbz" },
            r(rt, bit >= 32),
            target(offset)
        ),
        Insn::Br { rn } => format!("br x{rn}"),
        Insn::Blr { rn } => format!("blr x{rn}"),
        Insn::Ret { rn } => {
            if rn == 30 {
                "ret".into()
            } else {
                format!("ret x{rn}")
            }
        }
        Insn::Adr { page, rd, imm: off } => {
            let base = if page { pc & !0xfff } else { pc };
            format!(
                "{} x{rd}, {}",
                if page { "adrp" } else { "adr" },
                name_of(base.wrapping_add(off as u64))
            )
        }
        Insn::Svc { imm: v } => format!("svc #{v}"),
        Insn::Brk { imm: v } => format!("brk #{v}"),
        Insn::Nop => "nop".into(),
        Insn::LdSt { size, op, rt, rn, addr } => {
            let unscaled = insn::is_unscaled(word);
            let (base, sf) = match op {
                LdStOp::Store => ("st", size == 3),
                LdStOp::Load => ("ld", size == 3),
                LdStOp::LoadS64 => ("ld", true),
                LdStOp::LoadS32 => ("ld", false),
            };
            let signed = matches!(op, LdStOp::LoadS64 | LdStOp::LoadS32);
            let width = match size {
                0 => "b",
                1 => "h",
                2 if signed => "w",
                _ => "",
            };
            let m = format!(
                "{base}{}r{}{width}",
                if unscaled { "u" } else { "" },
                if signed { "s" } else { "" }
            );
            let mem = match addr {
                Addr::Off(0) => format!("[{}]", rs(rn, true)),
                Addr::Off(i) => format!("[{}, {}]", rs(rn, true), imm(i)),
                Addr::Pre(i) => format!("[{}, {}]!", rs(rn, true), imm(i)),
                Addr::Post(i) => format!("[{}], {}", rs(rn, true), imm(i)),
                Addr::Reg { rm, option, scaled } => {
                    let ext = match option {
                        2 => "uxtw",
                        3 => "lsl",
                        6 => "sxtw",
                        7 => "sxtx",
                        _ => "?",
                    };
                    let wide = option & 1 == 1;
                    if !scaled && option == 3 {
                        format!("[{}, {}]", rs(rn, true), r(rm, true))
                    } else if scaled {
                        format!("[{}, {}, {ext} #{size}]", rs(rn, true), r(rm, wide))
                    } else {
                        format!("[{}, {}, {ext}]", rs(rn, true), r(rm, wide))
                    }
                }
            };
            format!("{m} {}, {mem}", r(rt, sf))
        }
        Insn::LdStPair {
            sf,
            load,
            rt,
            rt2,
            rn,
            imm: v,
            addr,
        } => {
            let mem = match addr {
                Addr::Off(0) => format!("[{}]", rs(rn, true)),
                Addr::Off(_) => format!("[{}, {}]", rs(rn, true), imm(v)),
                Addr::Pre(_) => format!("[{}, {}]!", rs(rn, true), imm(v)),
                Addr::Post(_) => format!("[{}], {}", rs(rn, true), imm(v)),
                Addr::Reg { .. } => "?".into(),
            };
            format!(
                "{} {}, {}, {mem}",
                if load { "ldp" } else { "stp" },
                r(rt, sf),
                r(rt2, sf)
            )
        }
        Insn::LdrLit { kind, rt, offset } => {
            let (m, sf) = match kind {
                LitKind::W => ("ldr", false),
                LitKind::X => ("ldr", true),
                LitKind::Sw => ("ldrsw", true),
            };
            format!("{m} {}, {}", r(rt, sf), target(offset))
        }
        Insn::Unknown(w) => format!(".word 0x{w:08x}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(w: u32) -> String {
        format(w, 0x10000, &|a| format!("0x{a:x}"))
    }

    #[test]
    fn common_forms() {
        assert_eq!(d(0xD2800020), "mov x0, #1");
        assert_eq!(d(0xD65F03C0), "ret");
        assert_eq!(d(0xA9BF7BFD), "stp x29, x30, [sp, #-0x10]!");
        assert_eq!(d(0x91000400), "add x0, x0, #1");
        assert_eq!(d(0xF1000C1F), "cmp x0, #3");
        assert_eq!(d(0x54000041), "b.ne 0x10008");
        assert_eq!(d(0xD4000001), "svc #0");
        assert_eq!(d(0x39400020), "ldrb w0, [x1]");
        assert_eq!(d(0xD37CEC00), "lsl x0, x0, #4");
        assert_eq!(d(0x9AC10802), "udiv x2, x0, x1");
        assert_eq!(d(0xAA0103E0), "mov x0, x1");
        assert_eq!(d(0x10000041), "adr x1, 0x10008");
    }
}
