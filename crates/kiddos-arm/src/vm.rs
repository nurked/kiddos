//! The machine: 31 registers, a stack pointer, a program counter, four
//! flags and a flat block of memory. One `step` runs one instruction.
//!
//! Faults are sentences for a kid, not signal numbers. Memory below the
//! program is "nothing", the program's own instructions are read-only
//! (writing over them is always a bug at this level), and every address
//! in a message is shown the way the debugger shows it.

use crate::insn::{self, Addr, Bitfield, CondSel, Insn, LdStOp, LitKind, Logic, Shift, Wide};

/// Where a program's first instruction lives. Below it there is nothing.
pub const TEXT_BASE: u64 = 0x10000;
/// Memory size unless a cartridge asks for more.
pub const DEFAULT_MEMORY: usize = 1024 * 1024;
pub const MAX_MEMORY: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// Reading or writing where nothing is mapped.
    Nothing {
        addr: u64,
        write: bool,
    },
    /// Past the end of memory.
    OutOfMemory {
        addr: u64,
        write: bool,
    },
    /// Storing into the instructions.
    WroteCode {
        addr: u64,
    },
    /// `pc` points at bytes that are not instructions.
    BadPc {
        addr: u64,
    },
    UnknownInsn {
        addr: u64,
        word: u32,
    },
    UnknownSyscall {
        number: u64,
    },
    /// A system call complained (message already in kid words).
    Sys(String),
    /// The program asked to stop (Ctrl-C, kill).
    Interrupted,
}

impl Fault {
    /// One sentence, with the address as the debugger shows it.
    pub fn explain(&self, name_of: &dyn Fn(u64) -> String) -> String {
        match self {
            Fault::Nothing { addr, write } => format!(
                "The program {} address {} - there is nothing there. Your program lives at 0x{:x} and up.",
                if *write { "wrote to" } else { "read from" },
                name_of(*addr),
                TEXT_BASE
            ),
            Fault::OutOfMemory { addr, write } => format!(
                "The program {} address {}, past the end of its memory.",
                if *write { "wrote to" } else { "read from" },
                name_of(*addr)
            ),
            Fault::WroteCode { addr } => format!(
                "The program wrote over its own instructions at {}. Data belongs in .data, not .text.",
                name_of(*addr)
            ),
            Fault::BadPc { addr } => format!(
                "The program jumped to {}, where there are no instructions. (A missing ret? Running off the end?)",
                name_of(*addr)
            ),
            Fault::UnknownInsn { addr, word } => format!(
                "The bytes at {} (0x{word:08x}) are not an instruction I know.",
                name_of(*addr)
            ),
            Fault::UnknownSyscall { number } => {
                format!("System call {number} does not exist. See: man syscalls")
            }
            Fault::Sys(s) => s.clone(),
            Fault::Interrupted => "Stopped.".into(),
        }
    }
}

/// What one step did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Ran,
    /// `brk`: a breakpoint instruction in the program.
    Brk(u16),
    Exit(i32),
}

/// The outside world a program reaches through `svc`.
pub trait Sys {
    /// Handle system call `number`. Arguments are in `x0..x5`; put the
    /// result in `x0`. Return `Some(code)` when the program exits.
    fn syscall(&mut self, vm: &mut Vm, number: u64) -> Result<Option<i32>, Fault>;
}

pub struct Vm {
    pub x: [u64; 31],
    pub sp: u64,
    pub pc: u64,
    pub n: bool,
    pub z: bool,
    pub c: bool,
    pub v: bool,
    pub mem: Vec<u8>,
    pub text_end: u64,
    pub data_end: u64,
    /// Instructions executed so far.
    pub steps: u64,
}

impl Vm {
    /// A machine with `memory` bytes, code at `TEXT_BASE`, data right
    /// after it and the stack at the top.
    pub fn new(memory: usize) -> Vm {
        let memory = memory.clamp(TEXT_BASE as usize + 4096, MAX_MEMORY);
        Vm {
            x: [0; 31],
            sp: memory as u64,
            pc: TEXT_BASE,
            n: false,
            z: false,
            c: false,
            v: false,
            mem: vec![0; memory],
            text_end: TEXT_BASE,
            data_end: TEXT_BASE,
            steps: 0,
        }
    }

    pub fn memory_size(&self) -> u64 {
        self.mem.len() as u64
    }

    /// Put a program in memory and point `pc` at `entry`.
    pub fn load(&mut self, text: &[u8], data: &[u8], bss: usize, entry: u64) {
        let tb = TEXT_BASE as usize;
        self.mem[tb..tb + text.len()].copy_from_slice(text);
        self.text_end = TEXT_BASE + text.len() as u64;
        let db = ((self.text_end + 15) & !15) as usize;
        self.mem[db..db + data.len()].copy_from_slice(data);
        self.data_end = (db + data.len() + bss) as u64;
        self.pc = entry;
        self.sp = self.memory_size();
        self.x = [0; 31];
        self.steps = 0;
        (self.n, self.z, self.c, self.v) = (false, false, false, false);
    }

    pub fn data_base(&self) -> u64 {
        (self.text_end + 15) & !15
    }

    // ---- registers -----------------------------------------------------

    /// Read register `r`; 31 is `xzr` unless `sp_ok`.
    #[inline]
    pub fn reg(&self, r: u8, sp_ok: bool) -> u64 {
        if r == 31 {
            if sp_ok {
                self.sp
            } else {
                0
            }
        } else {
            self.x[r as usize]
        }
    }

    #[inline]
    pub fn set_reg(&mut self, r: u8, v: u64, sf: bool, sp_ok: bool) {
        let v = if sf { v } else { v & 0xffff_ffff };
        if r == 31 {
            if sp_ok {
                self.sp = v;
            }
        } else {
            self.x[r as usize] = v;
        }
    }

    pub fn flags(&self) -> String {
        format!(
            "{}{}{}{}",
            if self.n { 'N' } else { 'n' },
            if self.z { 'Z' } else { 'z' },
            if self.c { 'C' } else { 'c' },
            if self.v { 'V' } else { 'v' }
        )
    }

    // ---- memory ----------------------------------------------------------

    fn check(&self, addr: u64, len: u64, write: bool) -> Result<usize, Fault> {
        if addr < TEXT_BASE {
            return Err(Fault::Nothing { addr, write });
        }
        if addr.checked_add(len).is_none_or(|end| end > self.memory_size()) {
            return Err(Fault::OutOfMemory { addr, write });
        }
        if write && addr < self.text_end {
            return Err(Fault::WroteCode { addr });
        }
        Ok(addr as usize)
    }

    pub fn read(&self, addr: u64, len: u64) -> Result<&[u8], Fault> {
        let a = self.check(addr, len, false)?;
        Ok(&self.mem[a..a + len as usize])
    }

    pub fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), Fault> {
        let a = self.check(addr, data.len() as u64, true)?;
        self.mem[a..a + data.len()].copy_from_slice(data);
        Ok(())
    }

    pub fn read_u(&self, addr: u64, size: u8) -> Result<u64, Fault> {
        let n = 1u64 << size;
        let b = self.read(addr, n)?;
        let mut v = 0u64;
        for (i, byte) in b.iter().enumerate() {
            v |= (*byte as u64) << (8 * i);
        }
        Ok(v)
    }

    pub fn write_u(&mut self, addr: u64, size: u8, v: u64) -> Result<(), Fault> {
        let n = 1usize << size;
        let bytes = v.to_le_bytes();
        self.write(addr, &bytes[..n])
    }

    /// Bytes for a message: `addr` in the debugger's notation.
    pub fn fetch(&self, addr: u64) -> Result<u32, Fault> {
        if addr < TEXT_BASE || addr + 4 > self.text_end || addr % 4 != 0 {
            return Err(Fault::BadPc { addr });
        }
        let a = addr as usize;
        Ok(u32::from_le_bytes([
            self.mem[a],
            self.mem[a + 1],
            self.mem[a + 2],
            self.mem[a + 3],
        ]))
    }

    /// A NUL-terminated string at `addr`, at most `max` bytes.
    pub fn read_cstr(&self, addr: u64, max: usize) -> Result<String, Fault> {
        let mut out = Vec::new();
        let mut a = addr;
        while out.len() < max {
            let b = self.read(a, 1)?[0];
            if b == 0 {
                break;
            }
            out.push(b);
            a += 1;
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    // ---- flags -----------------------------------------------------------

    pub fn cond(&self, cond: u8) -> bool {
        let base = match cond >> 1 {
            0 => self.z,
            1 => self.c,
            2 => self.n,
            3 => self.v,
            4 => self.c && !self.z,
            5 => self.n == self.v,
            6 => self.n == self.v && !self.z,
            _ => true,
        };
        if cond & 1 == 1 && cond != 15 {
            !base
        } else {
            base
        }
    }

    fn add_with_flags(&mut self, a: u64, b: u64, carry_in: bool, sf: bool, set: bool) -> u64 {
        if sf {
            let (r1, c1) = a.overflowing_add(b);
            let (r, c2) = r1.overflowing_add(carry_in as u64);
            if set {
                self.n = (r as i64) < 0;
                self.z = r == 0;
                self.c = c1 || c2;
                let (s1, v1) = (a as i64).overflowing_add(b as i64);
                let (_, v2) = s1.overflowing_add(carry_in as i64);
                self.v = v1 ^ v2;
            }
            r
        } else {
            let (a, b) = (a as u32, b as u32);
            let (r1, c1) = a.overflowing_add(b);
            let (r, c2) = r1.overflowing_add(carry_in as u32);
            if set {
                self.n = (r as i32) < 0;
                self.z = r == 0;
                self.c = c1 || c2;
                let (s1, v1) = (a as i32).overflowing_add(b as i32);
                let (_, v2) = s1.overflowing_add(carry_in as i32);
                self.v = v1 ^ v2;
            }
            r as u64
        }
    }

    fn logic_flags(&mut self, r: u64, sf: bool) {
        self.n = if sf { (r as i64) < 0 } else { (r as i32) < 0 };
        self.z = r == 0;
        self.c = false;
        self.v = false;
    }

    fn shifted(v: u64, shift: Shift, amount: u8, sf: bool) -> u64 {
        let bits = if sf { 64 } else { 32 };
        let amount = (amount as u32) % bits;
        if sf {
            match shift {
                Shift::Lsl => v << amount,
                Shift::Lsr => v >> amount,
                Shift::Asr => ((v as i64) >> amount) as u64,
                Shift::Ror => v.rotate_right(amount),
            }
        } else {
            let v = v as u32;
            (match shift {
                Shift::Lsl => v << amount,
                Shift::Lsr => v >> amount,
                Shift::Asr => ((v as i32) >> amount) as u32,
                Shift::Ror => v.rotate_right(amount),
            }) as u64
        }
    }

    // ---- execution ------------------------------------------------------

    /// Run one instruction.
    pub fn step(&mut self, sys: &mut dyn Sys) -> Result<Step, Fault> {
        let pc = self.pc;
        let word = self.fetch(pc)?;
        let insn = insn::decode(word);
        self.steps += 1;
        let mut next = pc + 4;
        match insn {
            Insn::AddSubImm {
                sf,
                sub,
                flags,
                rd,
                rn,
                imm,
                shift12,
            } => {
                let a = self.reg(rn, true);
                let imm = if shift12 { (imm as u64) << 12 } else { imm as u64 };
                let r = if sub {
                    self.add_with_flags(a, if sf { !imm } else { !imm & 0xffff_ffff }, true, sf, flags)
                } else {
                    self.add_with_flags(a, imm, false, sf, flags)
                };
                self.set_reg(rd, r, sf, !flags);
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
                let a = self.reg(rn, false);
                let b = Self::shifted(self.reg(rm, false), shift, amount, sf);
                let r = if sub {
                    self.add_with_flags(a, if sf { !b } else { !b & 0xffff_ffff }, true, sf, flags)
                } else {
                    self.add_with_flags(a, b, false, sf, flags)
                };
                self.set_reg(rd, r, sf, false);
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
                let a = self.reg(rn, false);
                let mut b = Self::shifted(self.reg(rm, false), shift, amount, sf);
                if invert {
                    b = !b;
                }
                let r = match op {
                    Logic::And | Logic::Ands => a & b,
                    Logic::Orr => a | b,
                    Logic::Eor => a ^ b,
                };
                let r = if sf { r } else { r & 0xffff_ffff };
                if op == Logic::Ands {
                    self.logic_flags(r, sf);
                }
                self.set_reg(rd, r, sf, false);
            }
            Insn::LogicImm {
                sf, op, rd, rn, imm, ..
            } => {
                let a = self.reg(rn, false);
                let r = match op {
                    Logic::And | Logic::Ands => a & imm,
                    Logic::Orr => a | imm,
                    Logic::Eor => a ^ imm,
                };
                let r = if sf { r } else { r & 0xffff_ffff };
                if op == Logic::Ands {
                    self.logic_flags(r, sf);
                }
                self.set_reg(rd, r, sf, op != Logic::Ands);
            }
            Insn::MovWide {
                sf,
                kind,
                rd,
                imm16,
                hw,
            } => {
                let shift = hw as u32 * 16;
                let r = match kind {
                    Wide::Movz => (imm16 as u64) << shift,
                    Wide::Movn => !((imm16 as u64) << shift),
                    Wide::Movk => {
                        let old = self.reg(rd, false);
                        (old & !(0xffffu64 << shift)) | ((imm16 as u64) << shift)
                    }
                };
                self.set_reg(rd, r, sf, false);
            }
            Insn::Bitfield {
                sf,
                kind,
                rd,
                rn,
                immr,
                imms,
            } => {
                let bits = if sf { 64u32 } else { 32 };
                let src = self.reg(rn, false);
                let (immr, imms) = (immr as u32, imms as u32);
                // ROR by immr, then keep bits 0..=imms of the rotated value
                let rotated = if sf {
                    src.rotate_right(immr)
                } else {
                    ((src as u32).rotate_right(immr)) as u64
                };
                let width = if imms >= immr {
                    imms - immr + 1
                } else {
                    bits - immr + imms + 1
                };
                let mask: u64 = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
                let field = rotated & mask;
                let r = match kind {
                    Bitfield::Ubfm => field,
                    Bitfield::Sbfm => {
                        // sign bit is the source bit imms
                        let sign = (src >> imms) & 1 == 1;
                        if sign {
                            field | !mask
                        } else {
                            field
                        }
                    }
                    Bitfield::Bfm => {
                        let old = self.reg(rd, false);
                        (old & !mask) | field
                    }
                };
                self.set_reg(rd, r, sf, false);
            }
            Insn::ShiftReg { sf, shift, rd, rn, rm } => {
                let amount = (self.reg(rm, false) % if sf { 64 } else { 32 }) as u8;
                let r = Self::shifted(self.reg(rn, false), shift, amount, sf);
                self.set_reg(rd, r, sf, false);
            }
            Insn::Div { sf, signed, rd, rn, rm } => {
                let a = self.reg(rn, false);
                let b = self.reg(rm, false);
                // a real ARM gives 0 for division by zero, no trap
                let r = if sf {
                    if b == 0 {
                        0
                    } else if signed {
                        (a as i64).wrapping_div(b as i64) as u64
                    } else {
                        a / b
                    }
                } else {
                    let (a, b) = (a as u32, b as u32);
                    (if b == 0 {
                        0
                    } else if signed {
                        (a as i32).wrapping_div(b as i32) as u32
                    } else {
                        a / b
                    }) as u64
                };
                self.set_reg(rd, r, sf, false);
            }
            Insn::MulAdd {
                sf,
                sub,
                rd,
                rn,
                rm,
                ra,
            } => {
                let p = self.reg(rn, false).wrapping_mul(self.reg(rm, false));
                let a = self.reg(ra, false);
                let r = if sub { a.wrapping_sub(p) } else { a.wrapping_add(p) };
                self.set_reg(rd, r, sf, false);
            }
            Insn::CondSel {
                sf,
                kind,
                rd,
                rn,
                rm,
                cond,
            } => {
                let r = if self.cond(cond) {
                    self.reg(rn, false)
                } else {
                    let m = self.reg(rm, false);
                    match kind {
                        CondSel::Csel => m,
                        CondSel::Csinc => m.wrapping_add(1),
                        CondSel::Csinv => !m,
                        CondSel::Csneg => m.wrapping_neg(),
                    }
                };
                self.set_reg(rd, r, sf, false);
            }
            Insn::B { offset } => next = pc.wrapping_add(offset as u64),
            Insn::Bl { offset } => {
                self.x[30] = pc + 4;
                next = pc.wrapping_add(offset as u64);
            }
            Insn::BCond { cond, offset } => {
                if self.cond(cond) {
                    next = pc.wrapping_add(offset as u64);
                }
            }
            Insn::Cbz {
                sf,
                nonzero,
                rt,
                offset,
            } => {
                let v = self.reg(rt, false);
                let v = if sf { v } else { v & 0xffff_ffff };
                if (v != 0) == nonzero {
                    next = pc.wrapping_add(offset as u64);
                }
            }
            Insn::Tbz {
                nonzero,
                rt,
                bit,
                offset,
            } => {
                let set = (self.reg(rt, false) >> bit) & 1 == 1;
                if set == nonzero {
                    next = pc.wrapping_add(offset as u64);
                }
            }
            Insn::Br { rn } => next = self.reg(rn, false),
            Insn::Blr { rn } => {
                next = self.reg(rn, false);
                self.x[30] = pc + 4;
            }
            Insn::Ret { rn } => next = self.reg(rn, false),
            Insn::Adr { page, rd, imm } => {
                let base = if page { pc & !0xfff } else { pc };
                self.set_reg(rd, base.wrapping_add(imm as u64), true, false);
            }
            Insn::Svc { .. } => {
                self.pc = next;
                let number = self.x[8];
                if let Some(code) = sys.syscall(self, number)? {
                    return Ok(Step::Exit(code));
                }
                return Ok(Step::Ran);
            }
            Insn::Brk { imm } => {
                self.pc = next;
                return Ok(Step::Brk(imm));
            }
            Insn::Nop => {}
            Insn::LdSt { size, op, rt, rn, addr } => {
                let base = self.reg(rn, true);
                let (ea, writeback) = match addr {
                    Addr::Off(i) => (base.wrapping_add(i as u64), None),
                    Addr::Pre(i) => {
                        let a = base.wrapping_add(i as u64);
                        (a, Some(a))
                    }
                    Addr::Post(i) => (base, Some(base.wrapping_add(i as u64))),
                    Addr::Reg { rm, option, scaled } => {
                        let mut off = self.reg(rm, false);
                        match option {
                            2 => off &= 0xffff_ffff,               // uxtw
                            6 => off = (off as u32) as i32 as u64, // sxtw
                            _ => {}
                        }
                        if scaled {
                            off <<= size;
                        }
                        (base.wrapping_add(off), None)
                    }
                };
                match op {
                    LdStOp::Store => {
                        let v = self.reg(rt, false);
                        self.write_u(ea, size, v)?;
                    }
                    LdStOp::Load => {
                        let v = self.read_u(ea, size)?;
                        self.set_reg(rt, v, true, false);
                    }
                    LdStOp::LoadS64 | LdStOp::LoadS32 => {
                        let v = self.read_u(ea, size)?;
                        let bits = 8 << size;
                        let v = (((v << (64 - bits)) as i64) >> (64 - bits)) as u64;
                        self.set_reg(rt, v, op == LdStOp::LoadS64, false);
                    }
                }
                if let Some(wb) = writeback {
                    self.set_reg(rn, wb, true, true);
                }
            }
            Insn::LdStPair {
                sf,
                load,
                rt,
                rt2,
                rn,
                imm,
                addr,
            } => {
                let base = self.reg(rn, true);
                let size = if sf { 3 } else { 2 };
                let (ea, writeback) = match addr {
                    Addr::Off(_) => (base.wrapping_add(imm as u64), None),
                    Addr::Pre(_) => {
                        let a = base.wrapping_add(imm as u64);
                        (a, Some(a))
                    }
                    Addr::Post(_) => (base, Some(base.wrapping_add(imm as u64))),
                    Addr::Reg { .. } => unreachable!(),
                };
                let step = 1u64 << size;
                if load {
                    let a = self.read_u(ea, size)?;
                    let b = self.read_u(ea + step, size)?;
                    self.set_reg(rt, a, sf, false);
                    self.set_reg(rt2, b, sf, false);
                } else {
                    let a = self.reg(rt, false);
                    let b = self.reg(rt2, false);
                    self.write_u(ea, size, a)?;
                    self.write_u(ea + step, size, b)?;
                }
                if let Some(wb) = writeback {
                    self.set_reg(rn, wb, true, true);
                }
            }
            Insn::LdrLit { kind, rt, offset } => {
                let ea = pc.wrapping_add(offset as u64);
                let v = match kind {
                    LitKind::W => self.read_u(ea, 2)?,
                    LitKind::X => self.read_u(ea, 3)?,
                    LitKind::Sw => self.read_u(ea, 2)? as u32 as i32 as i64 as u64,
                };
                self.set_reg(rt, v, true, false);
            }
            Insn::Unknown(word) => return Err(Fault::UnknownInsn { addr: pc, word }),
        }
        self.pc = next;
        Ok(Step::Ran)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoSys;
    impl Sys for NoSys {
        fn syscall(&mut self, vm: &mut Vm, n: u64) -> Result<Option<i32>, Fault> {
            if n == 93 {
                Ok(Some(vm.x[0] as i32))
            } else {
                Err(Fault::UnknownSyscall { number: n })
            }
        }
    }

    fn run(words: &[u32]) -> (Vm, Result<Step, Fault>) {
        let mut vm = Vm::new(DEFAULT_MEMORY);
        let text: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        vm.load(&text, &[], 0, TEXT_BASE);
        let mut last = Ok(Step::Ran);
        for _ in 0..1000 {
            last = vm.step(&mut NoSys);
            if last != Ok(Step::Ran) {
                break;
            }
        }
        (vm, last)
    }

    #[test]
    fn adds_and_exits() {
        // mov x0, #40; add x0, x0, #2; mov x8, #93; svc #0
        let (vm, r) = run(&[0xD2800500, 0x91000800, 0xD2800BA8, 0xD4000001]);
        assert_eq!(r, Ok(Step::Exit(42)));
        assert_eq!(vm.x[0], 42);
    }

    #[test]
    fn flags_after_subs() {
        // mov x0, #3; subs x1, x0, #3  -> Z set, C set (no borrow)
        let (vm, r) = run(&[0xD2800060, 0xF1000C01]);
        assert!(matches!(r, Err(Fault::BadPc { .. })), "{r:?}");
        assert!(vm.z && vm.c && !vm.n && !vm.v);
        assert!(vm.cond(0) && !vm.cond(1) && vm.cond(10)); // eq, ne, ge
    }

    #[test]
    fn reading_nothing_is_explained() {
        // mov x1, #0; ldr x0, [x1] -> address 0
        let (_, r) = run(&[0xD2800001, 0xF9400020]);
        assert_eq!(r, Err(Fault::Nothing { addr: 0, write: false }));
        assert!(Fault::Nothing { addr: 0, write: false }
            .explain(&|a| format!("0x{a:x}"))
            .contains("nothing there"));
    }

    #[test]
    fn stack_push_pop() {
        // mov x0, #7; str x0, [sp, #-16]!; ldr x1, [sp], #16
        let (vm, _) = run(&[0xD28000E0, 0xF81F0FE0, 0xF84107E1]);
        assert_eq!(vm.x[1], 7);
        assert_eq!(vm.sp, DEFAULT_MEMORY as u64);
    }

    #[test]
    fn division_by_zero_gives_zero_like_a_real_arm() {
        // mov x0, #5; mov x1, #0; udiv x2, x0, x1
        let (vm, _) = run(&[0xD28000A0, 0xD2800001, 0x9AC10802]);
        assert_eq!(vm.x[2], 0);
    }

    #[test]
    fn lsl_via_ubfm() {
        // mov x0, #1; lsl x0, x0, #4  (ubfm x0, x0, #60, #59)
        let (vm, _) = run(&[0xD2800020, 0xD37CEC00]);
        assert_eq!(vm.x[0], 16);
    }
}
