//! Two-pass assembler for the subset, GNU syntax: labels, `.text`/`.data`
//! /`.bss`, `.ascii`/`.asciz`/`.byte`/`.hword`/`.word`/`.quad`/`.space`/
//! `.align`, `.equ` and `name = expr`, `//` and `;` comments, `ldr x0, =sym`
//! with a literal pool, and every error with the line it came from.

use crate::image::Image;
use crate::insn::{cond_from_name, encode_bitmask, Shift};
use crate::vm::TEXT_BASE;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsmError {
    /// 1-based, 0 when the error is about the whole program.
    pub line: usize,
    pub msg: String,
}

impl std::fmt::Display for AsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line > 0 {
            write!(f, "line {}: {}", self.line, self.msg)
        } else {
            write!(f, "{}", self.msg)
        }
    }
}

type R<T> = Result<T, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Section {
    Text,
    Data,
    Bss,
}

#[derive(Debug, Clone)]
enum Kind {
    Insn {
        mnemonic: String,
        operands: Vec<String>,
    },
    /// Constant bytes (strings).
    Bytes(Vec<u8>),
    /// `size` bytes for each expression.
    Values {
        size: usize,
        exprs: Vec<String>,
    },
    Space,
}

#[derive(Debug, Clone)]
struct Item {
    line: usize,
    section: Section,
    offset: u64,
    kind: Kind,
}

#[derive(Debug, Clone)]
enum Sym {
    Label(Section, u64),
    /// `.equ`: the expression, and where `.` was when it was written.
    Equ(String, Section, u64),
}

pub const MNEMONICS: &[&str] = &[
    "mov", "movz", "movn", "movk", "mvn", "add", "adds", "sub", "subs", "cmp", "cmn", "neg", "negs", "and", "ands",
    "orr", "eor", "bic", "orn", "eon", "bics", "tst", "mul", "mneg", "madd", "msub", "udiv", "sdiv", "lsl", "lsr",
    "asr", "ror", "sxtb", "sxth", "sxtw", "uxtb", "uxth", "csel", "csinc", "csinv", "csneg", "cset", "csetm", "cinc",
    "cneg", "b", "bl", "cbz", "cbnz", "tbz", "tbnz", "br", "blr", "ret", "adr", "adrp", "svc", "brk", "nop", "ldr",
    "str", "ldrb", "strb", "ldrh", "strh", "ldrsb", "ldrsh", "ldrsw", "ldur", "stur", "ldurb", "sturb", "ldurh",
    "sturh", "ldursb", "ldursh", "ldursw", "stp", "ldp",
];

const DIRECTIVES: &[&str] = &[
    ".text", ".data", ".bss", ".section", ".global", ".globl", ".ascii", ".asciz", ".string", ".byte", ".hword",
    ".short", ".half", ".word", ".int", ".long", ".quad", ".xword", ".dword", ".space", ".skip", ".zero", ".align",
    ".balign", ".p2align", ".equ", ".set", ".type", ".size", ".arch", ".cpu", ".file", ".ident", ".extern", ".end",
];

/// Assemble a whole program.
pub fn assemble(src: &str) -> Result<Image, AsmError> {
    let mut a = Assembler::default();
    a.pass1(src)?;
    a.pass2(src)
}

#[derive(Default)]
struct Assembler {
    items: Vec<Item>,
    syms: HashMap<String, Sym>,
    /// Label names in the order they appeared (for the symbol table).
    order: Vec<String>,
    sizes: HashMap<Section, u64>,
    pool: Vec<String>,
    section: Option<Section>,
    // resolved after pass 1
    pool_base: u64,
    bases: HashMap<Section, u64>,
}

// ---- text helpers ----------------------------------------------------------

/// Cut the comment off a line (`//`, `;`, and `#` only at the very start
/// where it cannot be an immediate).
fn strip_comment(line: &str) -> &str {
    let t = line.trim_start();
    if t.starts_with('#') || t.starts_with('@') {
        return "";
    }
    let mut in_str = false;
    let mut prev = ' ';
    let mut in_char = false;
    for (i, c) in line.char_indices() {
        if in_str {
            if c == '"' && prev != '\\' {
                in_str = false;
            }
        } else if in_char {
            if c == '\'' && prev != '\\' {
                in_char = false;
            }
        } else if c == '"' {
            in_str = true;
        } else if c == '\'' {
            in_char = true;
        } else if c == ';' || (c == '/' && line[i..].starts_with("//")) {
            return &line[..i];
        }
        prev = c;
    }
    line
}

/// Split operands on commas that are not inside `[]`, `()` or quotes.
fn split_operands(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut in_char = false;
    let mut prev = ' ';
    for c in s.chars() {
        match c {
            '"' if !in_char && prev != '\\' => in_str = !in_str,
            '\'' if !in_str && prev != '\\' => in_char = !in_char,
            '[' | '(' if !in_str && !in_char => depth += 1,
            ']' | ')' if !in_str && !in_char => depth -= 1,
            ',' if depth == 0 && !in_str && !in_char => {
                out.push(cur.trim().to_string());
                cur.clear();
                prev = c;
                continue;
            }
            _ => {}
        }
        cur.push(c);
        prev = c;
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn parse_string(s: &str) -> R<Vec<u8>> {
    let s = s.trim();
    if !(s.len() >= 2 && s.starts_with('"') && s.ends_with('"')) {
        return Err(format!("I expected text in double quotes, like \"hello\", not {s}"));
    }
    let mut out = Vec::new();
    let mut chars = s[1..s.len() - 1].chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match chars.next() {
            Some('n') => out.push(b'\n'),
            Some('t') => out.push(b'\t'),
            Some('r') => out.push(b'\r'),
            Some('0') => out.push(0),
            Some('\\') => out.push(b'\\'),
            Some('"') => out.push(b'"'),
            Some('x') => {
                let mut v = 0u8;
                let mut n = 0;
                while let Some(h) = chars.peek().and_then(|c| c.to_digit(16)) {
                    v = v.wrapping_mul(16).wrapping_add(h as u8);
                    chars.next();
                    n += 1;
                    if n == 2 {
                        break;
                    }
                }
                out.push(v);
            }
            Some(o) => {
                return Err(format!(
                    "I don't know the escape \\{o}. I know \\n \\t \\0 \\\\ \\\" and \\xHH."
                ))
            }
            None => return Err("the text ends with a lonely backslash".into()),
        }
    }
    Ok(out)
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '.' || c == '$'
}
fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '$'
}

// ---- expressions -------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i64),
    Ident(String),
    Op(String),
}

fn tokenize(s: &str) -> R<Vec<Tok>> {
    let mut out = Vec::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '\'' {
            // 'A' character
            let mut j = i + 1;
            let ch = if cs.get(j) == Some(&'\\') {
                j += 1;
                match cs.get(j) {
                    Some('n') => '\n',
                    Some('t') => '\t',
                    Some('0') => '\0',
                    Some('\\') => '\\',
                    Some('\'') => '\'',
                    _ => return Err("I don't know that character escape".into()),
                }
            } else {
                *cs.get(j).ok_or("a lonely quote")?
            };
            j += 1;
            if cs.get(j) != Some(&'\'') {
                return Err("a character is one letter in single quotes, like 'A'".into());
            }
            out.push(Tok::Num(ch as i64));
            i = j + 1;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < cs.len() && (cs[i].is_ascii_alphanumeric() || cs[i] == '_') {
                i += 1;
            }
            let text: String = cs[start..i].iter().filter(|c| **c != '_').collect();
            let v = if let Some(h) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                u64::from_str_radix(h, 16).ok()
            } else if let Some(b) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
                u64::from_str_radix(b, 2).ok()
            } else {
                text.parse::<u64>().ok()
            };
            match v {
                Some(v) => out.push(Tok::Num(v as i64)),
                None => return Err(format!("{text} is not a number I understand")),
            }
        } else if is_ident_start(c) {
            let start = i;
            i += 1;
            while i < cs.len() && is_ident(cs[i]) {
                i += 1;
            }
            out.push(Tok::Ident(cs[start..i].iter().collect()));
        } else if c == '<' && cs.get(i + 1) == Some(&'<') || c == '>' && cs.get(i + 1) == Some(&'>') {
            out.push(Tok::Op(format!("{c}{c}")));
            i += 2;
        } else if "+-*/%&|^~()".contains(c) {
            out.push(Tok::Op(c.to_string()));
            i += 1;
        } else {
            return Err(format!("I don't understand '{c}' here"));
        }
    }
    Ok(out)
}

struct Expr<'a> {
    toks: &'a [Tok],
    pos: usize,
    lookup: &'a dyn Fn(&str) -> R<i64>,
}

impl Expr<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn eat(&mut self, op: &str) -> bool {
        if self.peek() == Some(&Tok::Op(op.into())) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn primary(&mut self) -> R<i64> {
        match self.toks.get(self.pos).cloned() {
            Some(Tok::Num(n)) => {
                self.pos += 1;
                Ok(n)
            }
            Some(Tok::Ident(name)) => {
                self.pos += 1;
                (self.lookup)(&name)
            }
            Some(Tok::Op(o)) if o == "(" => {
                self.pos += 1;
                let v = self.or()?;
                if !self.eat(")") {
                    return Err("a ( without its )".into());
                }
                Ok(v)
            }
            Some(Tok::Op(o)) if o == "-" => {
                self.pos += 1;
                Ok(self.primary()?.wrapping_neg())
            }
            Some(Tok::Op(o)) if o == "+" => {
                self.pos += 1;
                self.primary()
            }
            Some(Tok::Op(o)) if o == "~" => {
                self.pos += 1;
                Ok(!self.primary()?)
            }
            Some(t) => Err(format!("I did not expect {} here", tok_text(&t))),
            None => Err("the expression ends too early".into()),
        }
    }
    fn mul(&mut self) -> R<i64> {
        let mut v = self.primary()?;
        loop {
            if self.eat("*") {
                v = v.wrapping_mul(self.primary()?);
            } else if self.eat("/") {
                let d = self.primary()?;
                if d == 0 {
                    return Err("division by zero in the expression".into());
                }
                v = v.wrapping_div(d);
            } else if self.eat("%") {
                let d = self.primary()?;
                if d == 0 {
                    return Err("division by zero in the expression".into());
                }
                v = v.wrapping_rem(d);
            } else {
                return Ok(v);
            }
        }
    }
    fn add(&mut self) -> R<i64> {
        let mut v = self.mul()?;
        loop {
            if self.eat("+") {
                v = v.wrapping_add(self.mul()?);
            } else if self.eat("-") {
                v = v.wrapping_sub(self.mul()?);
            } else {
                return Ok(v);
            }
        }
    }
    fn shift(&mut self) -> R<i64> {
        let mut v = self.add()?;
        loop {
            if self.eat("<<") {
                v = v.wrapping_shl(self.add()? as u32);
            } else if self.eat(">>") {
                v = ((v as u64).wrapping_shr(self.add()? as u32)) as i64;
            } else {
                return Ok(v);
            }
        }
    }
    fn and(&mut self) -> R<i64> {
        let mut v = self.shift()?;
        while self.eat("&") {
            v &= self.shift()?;
        }
        Ok(v)
    }
    fn xor(&mut self) -> R<i64> {
        let mut v = self.and()?;
        while self.eat("^") {
            v ^= self.and()?;
        }
        Ok(v)
    }
    fn or(&mut self) -> R<i64> {
        let mut v = self.xor()?;
        while self.eat("|") {
            v |= self.xor()?;
        }
        Ok(v)
    }
}

fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Num(n) => n.to_string(),
        Tok::Ident(s) => s.clone(),
        Tok::Op(o) => format!("'{o}'"),
    }
}

/// Evaluate an expression with the caller's names (the debugger's `:mem sp+16`).
pub fn eval_with(s: &str, lookup: &dyn Fn(&str) -> R<i64>) -> R<i64> {
    eval(s, lookup)
}

fn eval(s: &str, lookup: &dyn Fn(&str) -> R<i64>) -> R<i64> {
    let s = s.trim();
    let s = s.strip_prefix('#').unwrap_or(s).trim();
    if s.is_empty() {
        return Err("I expected a number or a name here".into());
    }
    let toks = tokenize(s)?;
    let mut e = Expr {
        toks: &toks,
        pos: 0,
        lookup,
    };
    let v = e.or()?;
    if e.pos != toks.len() {
        return Err(format!("I did not expect {} here", tok_text(&toks[e.pos])));
    }
    Ok(v)
}

// ---- operands -------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reg {
    n: u8,
    sf: bool,
    /// Written as `sp`/`wsp` (31 as the stack pointer, not zero).
    sp: bool,
}

fn parse_reg(s: &str) -> Option<Reg> {
    let s = s.trim().to_ascii_lowercase();
    match s.as_str() {
        "sp" => {
            return Some(Reg {
                n: 31,
                sf: true,
                sp: true,
            })
        }
        "wsp" => {
            return Some(Reg {
                n: 31,
                sf: false,
                sp: true,
            })
        }
        "xzr" => {
            return Some(Reg {
                n: 31,
                sf: true,
                sp: false,
            })
        }
        "wzr" => {
            return Some(Reg {
                n: 31,
                sf: false,
                sp: false,
            })
        }
        "lr" => {
            return Some(Reg {
                n: 30,
                sf: true,
                sp: false,
            })
        }
        "fp" => {
            return Some(Reg {
                n: 29,
                sf: true,
                sp: false,
            })
        }
        _ => {}
    }
    let (sf, rest) = if let Some(r) = s.strip_prefix('x') {
        (true, r)
    } else if let Some(r) = s.strip_prefix('w') {
        (false, r)
    } else {
        return None;
    };
    let n: u8 = rest.parse().ok()?;
    if n > 30 {
        return None;
    }
    Some(Reg { n, sf, sp: false })
}

#[derive(Debug, Clone)]
enum Mem {
    Off(String),
    Pre(String),
    Post(String),
    Reg {
        rm: Reg,
        ext: String,
        shift: Option<String>,
    },
}

/// `[x0]`, `[x0, #8]`, `[x0, #8]!`, `[x0, x1]`, `[x0, x1, lsl #3]`, and
/// the post-index `[x0], #8` whose immediate arrives as the next operand.
fn parse_mem(s: &str, post: Option<&str>) -> R<(Reg, Mem)> {
    let s = s.trim();
    let (inner, pre) = if let Some(i) = s.strip_suffix('!') {
        (i.trim(), true)
    } else {
        (s, false)
    };
    let inner = inner
        .strip_prefix('[')
        .and_then(|i| i.strip_suffix(']'))
        .ok_or("a memory address is written in square brackets, like [x0] or [x0, #8]")?;
    let parts = split_operands(inner);
    let base = parts
        .first()
        .and_then(|p| parse_reg(p))
        .filter(|r| r.sf)
        .ok_or("the first thing in [ ] must be an x register (or sp) holding the address")?;
    if let Some(p) = post {
        if parts.len() != 1 || pre {
            return Err("post-index is written [x0], #8 with nothing else in the brackets".into());
        }
        return Ok((base, Mem::Post(p.to_string())));
    }
    match parts.len() {
        1 => Ok((
            base,
            if pre {
                Mem::Pre("0".into())
            } else {
                Mem::Off("0".into())
            },
        )),
        2 => {
            if let Some(rm) = parse_reg(&parts[1]) {
                if pre {
                    return Err("! only goes with a number offset, like [x0, #8]!".into());
                }
                Ok((
                    base,
                    Mem::Reg {
                        rm,
                        ext: String::new(),
                        shift: None,
                    },
                ))
            } else {
                let off = parts[1].clone();
                Ok((base, if pre { Mem::Pre(off) } else { Mem::Off(off) }))
            }
        }
        3 => {
            let rm = parse_reg(&parts[1]).ok_or("with three things in [ ], the second must be a register")?;
            let ext = parts[2].to_ascii_lowercase();
            let mut words = ext.split_whitespace();
            let kind = words.next().unwrap_or("").to_string();
            let shift = words.next().map(|s| s.to_string());
            Ok((base, Mem::Reg { rm, ext: kind, shift }))
        }
        _ => Err("too many things in [ ]".into()),
    }
}

fn parse_shift_op(s: &str) -> Option<(Shift, String)> {
    let s = s.trim();
    let mut it = s.splitn(2, char::is_whitespace);
    let kind = match it.next()?.to_ascii_lowercase().as_str() {
        "lsl" => Shift::Lsl,
        "lsr" => Shift::Lsr,
        "asr" => Shift::Asr,
        "ror" => Shift::Ror,
        _ => return None,
    };
    Some((kind, it.next().unwrap_or("0").trim().to_string()))
}

// ---- the passes ---------------------------------------------------------------------

impl Assembler {
    fn size(&self, s: Section) -> u64 {
        *self.sizes.get(&s).unwrap_or(&0)
    }

    fn bump(&mut self, s: Section, n: u64) {
        *self.sizes.entry(s).or_insert(0) += n;
    }

    fn define(&mut self, name: &str, sym: Sym, line: usize) -> Result<(), AsmError> {
        if parse_reg(name).is_some() {
            return Err(AsmError {
                line,
                msg: format!("{name} is a register; a label needs another name"),
            });
        }
        if self.syms.contains_key(name) {
            return Err(AsmError {
                line,
                msg: format!("{name} is defined twice. A label can only be in one place."),
            });
        }
        self.syms.insert(name.to_string(), sym);
        self.order.push(name.to_string());
        Ok(())
    }

    fn pass1(&mut self, src: &str) -> Result<(), AsmError> {
        for (idx, raw) in src.lines().enumerate() {
            let line = idx + 1;
            let mut text = strip_comment(raw).trim().to_string();
            // labels: `name:` possibly several, possibly followed by code
            while let Some(colon) = text.find(':') {
                let name = text[..colon].trim();
                if name.is_empty() || !name.chars().all(is_ident) || !name.starts_with(is_ident_start) {
                    break;
                }
                let sec = self.section.unwrap_or(Section::Text);
                let off = self.size(sec);
                self.define(name, Sym::Label(sec, off), line)?;
                text = text[colon + 1..].trim().to_string();
            }
            if text.is_empty() {
                continue;
            }
            // `name = expr`
            if let Some(eq) = text.find('=') {
                let name = text[..eq].trim();
                if !name.is_empty() && name.chars().all(is_ident) && !text[eq + 1..].starts_with('=') {
                    let sec = self.section.unwrap_or(Section::Text);
                    let dot = self.size(sec);
                    self.define(name, Sym::Equ(text[eq + 1..].trim().to_string(), sec, dot), line)?;
                    continue;
                }
            }
            let (head, rest) = match text.find(char::is_whitespace) {
                Some(i) => (text[..i].to_ascii_lowercase(), text[i..].trim().to_string()),
                None => (text.to_ascii_lowercase(), String::new()),
            };
            if head.starts_with('.') {
                self.directive(&head, &rest, line)?;
                continue;
            }
            let sec = self.section.unwrap_or(Section::Text);
            if sec != Section::Text {
                return Err(AsmError {
                    line,
                    msg: format!("instructions go in .text, not in .{}", section_name(sec)),
                });
            }
            self.section = Some(Section::Text);
            if !MNEMONICS.contains(&head.as_str()) && !head.starts_with("b.") && !is_bcond_short(&head) {
                return Err(AsmError {
                    line,
                    msg: unknown_mnemonic(&head),
                });
            }
            let operands = split_operands(&rest);
            // ldr rt, =expr reserves a slot in the literal pool
            if head == "ldr" {
                if let Some(lit) = operands.get(1).and_then(|o| o.strip_prefix('=')) {
                    let lit = lit.trim().to_string();
                    if !self.pool.contains(&lit) {
                        self.pool.push(lit);
                    }
                }
            }
            let off = self.size(Section::Text);
            self.items.push(Item {
                line,
                section: Section::Text,
                offset: off,
                kind: Kind::Insn {
                    mnemonic: head,
                    operands,
                },
            });
            self.bump(Section::Text, 4);
        }
        // lay the sections out
        let text_len = self.size(Section::Text);
        self.pool_base = (text_len + 7) & !7;
        let text_total = if self.pool.is_empty() {
            text_len
        } else {
            self.pool_base + 8 * self.pool.len() as u64
        };
        let data_base = TEXT_BASE + ((text_total + 15) & !15);
        let bss_base = data_base + self.size(Section::Data);
        self.bases.insert(Section::Text, TEXT_BASE);
        self.bases.insert(Section::Data, data_base);
        self.bases.insert(Section::Bss, bss_base);
        Ok(())
    }

    fn directive(&mut self, head: &str, rest: &str, line: usize) -> Result<(), AsmError> {
        let err = |msg: String| AsmError { line, msg };
        let sec = self.section.unwrap_or(Section::Text);
        match head {
            ".text" => self.section = Some(Section::Text),
            ".data" => self.section = Some(Section::Data),
            ".bss" => self.section = Some(Section::Bss),
            ".section" => {
                let name = rest.split(',').next().unwrap_or("").trim();
                self.section = Some(match name {
                    ".text" => Section::Text,
                    ".data" | ".rodata" => Section::Data,
                    ".bss" => Section::Bss,
                    other => return Err(err(format!("I know the sections .text, .data and .bss, not {other}"))),
                });
            }
            ".global" | ".globl" | ".type" | ".size" | ".arch" | ".cpu" | ".file" | ".ident" | ".extern" | ".end" => {}
            ".ascii" | ".asciz" | ".string" => {
                if sec == Section::Text {
                    return Err(err("text goes in the .data section, after a .data line".into()));
                }
                if sec == Section::Bss {
                    return Err(err(".bss holds only empty space (.space); put text in .data".into()));
                }
                let mut bytes = Vec::new();
                for part in split_operands(rest) {
                    bytes.extend(parse_string(&part).map_err(err)?);
                    if head != ".ascii" {
                        bytes.push(0);
                    }
                }
                let n = bytes.len() as u64;
                self.items.push(Item {
                    line,
                    section: sec,
                    offset: self.size(sec),
                    kind: Kind::Bytes(bytes),
                });
                self.bump(sec, n);
            }
            ".byte" | ".hword" | ".short" | ".half" | ".word" | ".int" | ".long" | ".quad" | ".xword" | ".dword" => {
                if sec == Section::Bss {
                    return Err(err(".bss holds only empty space (.space); put numbers in .data".into()));
                }
                let size = match head {
                    ".byte" => 1,
                    ".hword" | ".short" | ".half" => 2,
                    ".word" | ".int" | ".long" => 4,
                    _ => 8,
                };
                let exprs = split_operands(rest);
                if exprs.is_empty() {
                    return Err(err(format!("{head} needs at least one number")));
                }
                let n = (size * exprs.len()) as u64;
                self.items.push(Item {
                    line,
                    section: sec,
                    offset: self.size(sec),
                    kind: Kind::Values { size, exprs },
                });
                self.bump(sec, n);
            }
            ".space" | ".skip" | ".zero" => {
                let parts = split_operands(rest);
                let n = parts
                    .first()
                    .ok_or_else(|| err(format!("{head} needs a size, like {head} 16")))?;
                let n = self.eval_const(n).map_err(err)?;
                if !(0..=16 * 1024 * 1024).contains(&n) {
                    return Err(err(format!("{n} bytes? That is more than the machine has.")));
                }
                self.items.push(Item {
                    line,
                    section: sec,
                    offset: self.size(sec),
                    kind: Kind::Space,
                });
                self.bump(sec, n as u64);
            }
            ".align" | ".balign" | ".p2align" => {
                let n = self
                    .eval_const(rest.split(',').next().unwrap_or("").trim())
                    .map_err(err)?;
                let bytes = if head == ".balign" {
                    n
                } else {
                    if !(0..=16).contains(&n) {
                        return Err(err(format!("{head} takes a power of two: .align 3 means 8 bytes")));
                    }
                    1i64 << n
                };
                if bytes <= 0 || bytes > 65536 {
                    return Err(err(format!("{bytes} is not an alignment I can do")));
                }
                let cur = self.size(sec);
                let pad = (bytes as u64 - cur % bytes as u64) % bytes as u64;
                if pad > 0 {
                    self.items.push(Item {
                        line,
                        section: sec,
                        offset: cur,
                        kind: Kind::Space,
                    });
                    self.bump(sec, pad);
                }
            }
            ".equ" | ".set" => {
                let parts = split_operands(rest);
                if parts.len() != 2 {
                    return Err(err(format!("{head} is written {head} name, value")));
                }
                let dot = self.size(sec);
                self.define(&parts[0], Sym::Equ(parts[1].clone(), sec, dot), line)?;
            }
            other => {
                let near = DIRECTIVES
                    .iter()
                    .filter(|d| strsim::levenshtein(d, other) <= 2)
                    .min_by_key(|d| strsim::levenshtein(d, other));
                let mut msg = format!("I don't know the directive {other}.");
                if let Some(n) = near {
                    msg.push_str(&format!(" Did you mean {n}?"));
                }
                return Err(err(msg));
            }
        }
        Ok(())
    }

    /// Pass-1 evaluation: only numbers and names already known.
    fn eval_const(&self, s: &str) -> R<i64> {
        eval(s, &|name| match self.syms.get(name) {
            Some(Sym::Equ(e, _, _)) => self.eval_const(e),
            Some(Sym::Label(..)) => Err(format!("{name} is a label; a size must be a plain number")),
            None => Err(format!("{name} is not a name I know (yet). Sizes must be numbers.")),
        })
    }

    fn base(&self, s: Section) -> u64 {
        *self.bases.get(&s).unwrap_or(&TEXT_BASE)
    }

    /// Pass-2 lookup: every symbol is an absolute address or a value.
    fn lookup(&self, name: &str, dot: u64, depth: u32) -> R<i64> {
        if name == "." {
            return Ok(dot as i64);
        }
        if depth > 32 {
            return Err(format!("{name} is defined in terms of itself"));
        }
        match self.syms.get(name) {
            Some(Sym::Label(sec, off)) => Ok((self.base(*sec) + off) as i64),
            Some(Sym::Equ(e, sec, off)) => {
                let dot = self.base(*sec) + off;
                eval(e, &|n| self.lookup(n, dot, depth + 1))
            }
            None => {
                let near = self
                    .syms
                    .keys()
                    .filter(|k| strsim::levenshtein(k, name) <= 2)
                    .min_by_key(|k| strsim::levenshtein(k, name));
                let mut msg = format!("I have not seen a label called {name}.");
                if let Some(n) = near {
                    msg.push_str(&format!(" Did you mean {n}?"));
                }
                Err(msg)
            }
        }
    }

    fn eval_at(&self, s: &str, dot: u64) -> R<i64> {
        eval(s, &|n| self.lookup(n, dot, 0))
    }

    fn pass2(&mut self, src: &str) -> Result<Image, AsmError> {
        let mut text = vec![0u8; self.size(Section::Text) as usize];
        let mut data = vec![0u8; self.size(Section::Data) as usize];
        let mut lines = Vec::new();
        for item in &self.items {
            let err = |msg: String| AsmError { line: item.line, msg };
            let addr = self.base(item.section) + item.offset;
            match &item.kind {
                Kind::Insn { mnemonic, operands } => {
                    let word = self.encode(mnemonic, operands, addr).map_err(err)?;
                    let o = item.offset as usize;
                    text[o..o + 4].copy_from_slice(&word.to_le_bytes());
                    lines.push((addr as u32, item.line as u32));
                }
                Kind::Bytes(b) => {
                    let o = item.offset as usize;
                    if item.section == Section::Data {
                        data[o..o + b.len()].copy_from_slice(b);
                    } else {
                        text[o..o + b.len()].copy_from_slice(b);
                    }
                }
                Kind::Values { size, exprs } => {
                    let mut o = item.offset as usize;
                    for e in exprs {
                        let v = self.eval_at(e, addr).map_err(err)?;
                        let bits = 8 * size;
                        if *size < 8 {
                            let lo = -(1i64 << (bits - 1));
                            let hi = (1i64 << bits) - 1;
                            if v < lo || v > hi {
                                return Err(err(format!("{v} does not fit in {size} byte(s)")));
                            }
                        }
                        let bytes = v.to_le_bytes();
                        let dst = if item.section == Section::Data {
                            &mut data
                        } else {
                            &mut text
                        };
                        dst[o..o + size].copy_from_slice(&bytes[..*size]);
                        o += size;
                    }
                }
                Kind::Space => {}
            }
        }
        // the literal pool sits after the code
        let pool: Vec<String> = self.pool.clone();
        if !pool.is_empty() {
            text.resize(self.pool_base as usize + 8 * pool.len(), 0);
        }
        for (i, e) in pool.iter().enumerate() {
            let dot = TEXT_BASE + self.pool_base + 8 * i as u64;
            let v = self.eval_at(e, dot).map_err(|m| AsmError {
                line: self
                    .items
                    .iter()
                    .find(|it| matches!(&it.kind, Kind::Insn { mnemonic, operands } if mnemonic == "ldr" && operands.get(1).map(|o| o.trim_start_matches('=').trim()) == Some(e.as_str())))
                    .map(|it| it.line)
                    .unwrap_or(0),
                msg: m,
            })?;
            let o = self.pool_base as usize + 8 * i;
            text[o..o + 8].copy_from_slice(&v.to_le_bytes());
        }
        let entry = ["_start", "main", "start"]
            .iter()
            .find_map(|n| match self.syms.get(*n) {
                Some(Sym::Label(Section::Text, off)) => Some(TEXT_BASE + off),
                _ => None,
            })
            .unwrap_or(TEXT_BASE);
        if text.is_empty() {
            return Err(AsmError {
                line: 0,
                msg: "there are no instructions in this program".into(),
            });
        }
        let mut symbols = Vec::new();
        for name in &self.order {
            if let Some(Sym::Label(sec, off)) = self.syms.get(name) {
                symbols.push((name.clone(), self.base(*sec) + off));
            }
        }
        Ok(Image {
            text,
            data,
            bss: self.size(Section::Bss) as u32,
            entry,
            lines,
            symbols,
            source: src.to_string(),
        })
    }

    // ---- encoding one instruction ------------------------------------------------

    fn encode(&self, mn: &str, ops: &[String], pc: u64) -> R<u32> {
        let ev = |s: &str| self.eval_at(s, pc);
        let reg = |i: usize| -> R<Reg> {
            let s = ops.get(i).ok_or_else(|| format!("{mn} needs more operands"))?;
            parse_reg(s).ok_or_else(|| format!("{s} is not a register (x0-x30, w0-w30, sp)"))
        };
        let want = |n: usize| -> R<()> {
            if ops.len() != n {
                Err(format!(
                    "{mn} takes {n} operand{}, not {}",
                    if n == 1 { "" } else { "s" },
                    ops.len()
                ))
            } else {
                Ok(())
            }
        };
        let is_imm = |s: &str| s.starts_with('#') || (parse_reg(s).is_none() && !s.starts_with('['));
        let range = |v: i64, lo: i64, hi: i64, what: &str| -> R<u32> {
            if v < lo || v > hi {
                Err(format!("{v} does not fit here: {what} must be {lo} to {hi}"))
            } else {
                Ok(v as u32)
            }
        };
        let branch_off = |target: i64, bits: u32| -> R<u32> {
            let off = target.wrapping_sub(pc as i64);
            if off % 4 != 0 {
                return Err("a branch target must be an instruction (a multiple of 4)".into());
            }
            let lim = 1i64 << (bits + 1);
            if off < -lim || off >= lim {
                return Err("that is too far away to branch to".into());
            }
            Ok(((off / 4) as u32) & ((1u32 << bits) - 1))
        };
        let same_size = |a: Reg, b: Reg| -> R<()> {
            if a.sf != b.sf {
                Err("mixing x and w registers in one instruction is not allowed".into())
            } else {
                Ok(())
            }
        };
        let sfb = |r: Reg| (r.sf as u32) << 31;
        let shift_of = |i: usize, sf: bool| -> R<(Shift, u32)> {
            match ops.get(i) {
                None => Ok((Shift::Lsl, 0)),
                Some(s) => {
                    let (kind, amt) = parse_shift_op(s).ok_or_else(|| format!("{s} is not a shift like lsl #2"))?;
                    let amt = ev(&amt)?;
                    Ok((kind, range(amt, 0, if sf { 63 } else { 31 }, "the shift")?))
                }
            }
        };

        // b.cond written as beq/bne...
        let (mn, cond) = if let Some(c) = mn.strip_prefix("b.") {
            (
                "b.cond",
                Some(
                    cond_from_name(c)
                        .ok_or_else(|| format!("{c} is not a condition (eq ne lt le gt ge hi lo hs ls mi pl vs vc)"))?,
                ),
            )
        } else if is_bcond_short(mn) {
            ("b.cond", cond_from_name(&mn[1..]))
        } else {
            (mn, None)
        };

        match mn {
            "nop" => {
                want(0)?;
                Ok(0xD503201F)
            }
            "svc" | "brk" => {
                want(1)?;
                let v = range(ev(&ops[0])?, 0, 65535, "the number")?;
                Ok(if mn == "svc" { 0xD4000001 } else { 0xD4200000 } | (v << 5))
            }
            "ret" => {
                let rn = if ops.is_empty() { 30 } else { reg(0)?.n };
                Ok(0xD65F0000 | ((rn as u32) << 5))
            }
            "br" | "blr" => {
                want(1)?;
                let rn = reg(0)?;
                Ok(if mn == "br" { 0xD61F0000 } else { 0xD63F0000 } | ((rn.n as u32) << 5))
            }
            "b" | "bl" => {
                want(1)?;
                let t = ev(&ops[0])?;
                let off = branch_off(t, 26)?;
                Ok(if mn == "b" { 0x14000000 } else { 0x94000000 } | off)
            }
            "b.cond" => {
                want(1)?;
                let t = ev(&ops[0])?;
                let off = branch_off(t, 19)?;
                Ok(0x54000000 | (off << 5) | cond.unwrap_or(14) as u32)
            }
            "cbz" | "cbnz" => {
                want(2)?;
                let rt = reg(0)?;
                let off = branch_off(ev(&ops[1])?, 19)?;
                Ok(sfb(rt) | if mn == "cbz" { 0x34000000 } else { 0x35000000 } | (off << 5) | rt.n as u32)
            }
            "tbz" | "tbnz" => {
                want(3)?;
                let rt = reg(0)?;
                let bit = range(ev(&ops[1])?, 0, if rt.sf { 63 } else { 31 }, "the bit")?;
                let off = branch_off(ev(&ops[2])?, 14)?;
                Ok(((bit >> 5) << 31)
                    | if mn == "tbz" { 0x36000000 } else { 0x37000000 }
                    | ((bit & 31) << 19)
                    | (off << 5)
                    | rt.n as u32)
            }
            "adr" | "adrp" => {
                want(2)?;
                let rd = reg(0)?;
                let t = ev(&ops[1])?;
                let off = if mn == "adr" {
                    t.wrapping_sub(pc as i64)
                } else {
                    (t & !0xfff).wrapping_sub((pc & !0xfff) as i64) >> 12
                };
                if !(-(1 << 20)..(1 << 20)).contains(&off) {
                    return Err("that address is too far away for adr".into());
                }
                let off = off as u32 & 0x1fffff;
                Ok(if mn == "adr" { 0x10000000 } else { 0x90000000 }
                    | ((off & 3) << 29)
                    | ((off >> 2) << 5)
                    | rd.n as u32)
            }
            "movz" | "movn" | "movk" => {
                if ops.len() != 2 && ops.len() != 3 {
                    return Err(format!("{mn} is written {mn} x0, #imm  or  {mn} x0, #imm, lsl #16"));
                }
                let rd = reg(0)?;
                let v = range(ev(&ops[1])?, 0, 65535, "the number")?;
                let (kind, sh) = shift_of(2, true)?;
                if kind != Shift::Lsl || sh % 16 != 0 || sh > if rd.sf { 48 } else { 16 } {
                    return Err("the shift must be lsl #0, #16, #32 or #48".into());
                }
                let opc = match mn {
                    "movn" => 0,
                    "movz" => 2,
                    _ => 3,
                };
                Ok(sfb(rd) | (opc << 29) | 0x12800000 | ((sh / 16) << 21) | (v << 5) | rd.n as u32)
            }
            "mov" => {
                want(2)?;
                let rd = reg(0)?;
                if let Some(rm) = parse_reg(&ops[1]) {
                    same_size(rd, rm)?;
                    if rd.sp || rm.sp {
                        // add rd, rm, #0
                        return Ok(sfb(rd) | 0x11000000 | ((rm.n as u32) << 5) | rd.n as u32);
                    }
                    return Ok(sfb(rd) | 0x2A0003E0 | ((rm.n as u32) << 16) | rd.n as u32);
                }
                let v = ev(&ops[1])?;
                let bits = if rd.sf { 64 } else { 32 };
                let u = if rd.sf { v as u64 } else { (v as u64) & 0xffff_ffff };
                if !rd.sf && !(-(1i64 << 31)..(1i64 << 32)).contains(&v) {
                    return Err(format!("{v} does not fit in a w register (32 bits)"));
                }
                for hw in 0..bits / 16 {
                    let sh = hw * 16;
                    if u & !(0xffffu64 << sh) == 0 {
                        return Ok(sfb(rd)
                            | 0x52800000
                            | (hw << 21)
                            | (((u >> sh) as u32 & 0xffff) << 5)
                            | rd.n as u32);
                    }
                    let nu = if rd.sf { !u } else { !u & 0xffff_ffff };
                    if nu & !(0xffffu64 << sh) == 0 {
                        return Ok(sfb(rd)
                            | 0x12800000
                            | (hw << 21)
                            | (((nu >> sh) as u32 & 0xffff) << 5)
                            | rd.n as u32);
                    }
                }
                if let Some((n, immr, imms)) = encode_bitmask(u, rd.sf) {
                    return Ok(sfb(rd) | 0x320003E0 | (n << 22) | (immr << 16) | (imms << 10) | rd.n as u32);
                }
                Err(format!(
                    "{v} is too complicated for one mov. Use  ldr {}, ={v}  (the assembler keeps it in memory for you)",
                    if rd.sf { "x0" } else { "w0" }
                ))
            }
            "mvn" => {
                want(2)?;
                let rd = reg(0)?;
                let rm = reg(1)?;
                same_size(rd, rm)?;
                Ok(sfb(rd) | 0x2A2003E0 | ((rm.n as u32) << 16) | rd.n as u32)
            }
            "add" | "adds" | "sub" | "subs" | "cmp" | "cmn" | "neg" | "negs" => {
                let (rd, rn, src_i) = match mn {
                    "cmp" | "cmn" => {
                        if ops.len() < 2 {
                            return Err(format!("{mn} is written {mn} x0, x1  or  {mn} x0, #5"));
                        }
                        let rn = reg(0)?;
                        (
                            Reg {
                                n: 31,
                                sf: rn.sf,
                                sp: false,
                            },
                            rn,
                            1,
                        )
                    }
                    "neg" | "negs" => {
                        if ops.len() < 2 {
                            return Err(format!("{mn} is written {mn} x0, x1"));
                        }
                        let rd = reg(0)?;
                        (
                            rd,
                            Reg {
                                n: 31,
                                sf: rd.sf,
                                sp: false,
                            },
                            1,
                        )
                    }
                    _ => {
                        if ops.len() < 3 {
                            return Err(format!("{mn} is written {mn} x0, x1, x2  or  {mn} x0, x1, #5"));
                        }
                        (reg(0)?, reg(1)?, 2)
                    }
                };
                same_size(rd, rn)?;
                let sub = matches!(mn, "sub" | "subs" | "cmp" | "neg" | "negs");
                let flags = matches!(mn, "adds" | "subs" | "cmp" | "cmn" | "negs");
                let src = &ops[src_i];
                if is_imm(src) {
                    if ops.len() > src_i + 1 {
                        return Err(format!("{mn} with a number takes nothing after it"));
                    }
                    let mut v = ev(src)?;
                    let mut sub = sub;
                    if v < 0 && !matches!(mn, "neg" | "negs") {
                        v = -v;
                        sub = !sub;
                    }
                    let (imm, sh) = if (0..4096).contains(&v) {
                        (v as u32, 0)
                    } else if v % 4096 == 0 && (0..4096).contains(&(v / 4096)) {
                        ((v / 4096) as u32, 1)
                    } else {
                        return Err(format!(
                            "{v} is too big for {mn}: numbers go up to 4095. Put it in a register first (mov x2, #{v})"
                        ));
                    };
                    let op = (sub as u32) << 30 | (flags as u32) << 29;
                    return Ok(sfb(rd)
                        | op
                        | 0x11000000
                        | (sh << 22)
                        | (imm << 10)
                        | ((rn.n as u32) << 5)
                        | rd.n as u32);
                }
                let rm = parse_reg(src).ok_or_else(|| format!("{src} is not a register or a number"))?;
                same_size(rd, rm)?;
                if rd.sp || rn.sp || rm.sp {
                    return Err("sp with a register operand is not in this subset; use a number, or move sp to a register first".into());
                }
                if ops.len() > src_i + 2 {
                    return Err(format!("too many operands for {mn}"));
                }
                let (kind, amt) = shift_of(src_i + 1, rd.sf)?;
                if kind == Shift::Ror {
                    return Err("ror does not go with add/sub".into());
                }
                let op = (sub as u32) << 30 | (flags as u32) << 29;
                Ok(sfb(rd)
                    | op
                    | 0x0B000000
                    | (kind.bits() << 22)
                    | ((rm.n as u32) << 16)
                    | (amt << 10)
                    | ((rn.n as u32) << 5)
                    | rd.n as u32)
            }
            "and" | "ands" | "orr" | "eor" | "bic" | "orn" | "eon" | "bics" | "tst" => {
                let (rd, rn, src_i) = if mn == "tst" {
                    if ops.len() < 2 {
                        return Err("tst is written tst x0, x1  or  tst x0, #1".into());
                    }
                    let rn = reg(0)?;
                    (
                        Reg {
                            n: 31,
                            sf: rn.sf,
                            sp: false,
                        },
                        rn,
                        1,
                    )
                } else {
                    if ops.len() < 3 {
                        return Err(format!("{mn} is written {mn} x0, x1, x2  or  {mn} x0, x1, #0xff"));
                    }
                    (reg(0)?, reg(1)?, 2)
                };
                same_size(rd, rn)?;
                let (opc, invert) = match mn {
                    "and" => (0, false),
                    "orr" => (1, false),
                    "eor" => (2, false),
                    "ands" | "tst" => (3, false),
                    "bic" => (0, true),
                    "orn" => (1, true),
                    "eon" => (2, true),
                    _ => (3, true),
                };
                let src = &ops[src_i];
                if is_imm(src) {
                    if invert {
                        return Err(format!("{mn} takes a register, not a number"));
                    }
                    let v = ev(src)?;
                    let u = if rd.sf { v as u64 } else { (v as u64) & 0xffff_ffff };
                    let (n, immr, imms) = encode_bitmask(u, rd.sf).ok_or_else(|| {
                        format!("0x{u:x} cannot be a bit pattern here (it must be a repeated run of ones); put it in a register first")
                    })?;
                    return Ok(sfb(rd)
                        | (opc << 29)
                        | 0x12000000
                        | (n << 22)
                        | (immr << 16)
                        | (imms << 10)
                        | ((rn.n as u32) << 5)
                        | rd.n as u32);
                }
                let rm = parse_reg(src).ok_or_else(|| format!("{src} is not a register or a number"))?;
                same_size(rd, rm)?;
                let (kind, amt) = shift_of(src_i + 1, rd.sf)?;
                Ok(sfb(rd)
                    | (opc << 29)
                    | 0x0A000000
                    | (kind.bits() << 22)
                    | ((invert as u32) << 21)
                    | ((rm.n as u32) << 16)
                    | (amt << 10)
                    | ((rn.n as u32) << 5)
                    | rd.n as u32)
            }
            "mul" | "mneg" | "madd" | "msub" => {
                let four = mn == "madd" || mn == "msub";
                want(if four { 4 } else { 3 })?;
                let (rd, rn, rm) = (reg(0)?, reg(1)?, reg(2)?);
                same_size(rd, rn)?;
                same_size(rd, rm)?;
                let ra = if four { reg(3)?.n } else { 31 };
                let sub = mn == "mneg" || mn == "msub";
                Ok(sfb(rd)
                    | 0x1B000000
                    | ((rm.n as u32) << 16)
                    | ((sub as u32) << 15)
                    | ((ra as u32) << 10)
                    | ((rn.n as u32) << 5)
                    | rd.n as u32)
            }
            "udiv" | "sdiv" => {
                want(3)?;
                let (rd, rn, rm) = (reg(0)?, reg(1)?, reg(2)?);
                same_size(rd, rn)?;
                same_size(rd, rm)?;
                Ok(sfb(rd)
                    | 0x1AC00800
                    | ((rm.n as u32) << 16)
                    | (((mn == "sdiv") as u32) << 10)
                    | ((rn.n as u32) << 5)
                    | rd.n as u32)
            }
            "lsl" | "lsr" | "asr" | "ror" => {
                want(3)?;
                let (rd, rn) = (reg(0)?, reg(1)?);
                same_size(rd, rn)?;
                let bits = if rd.sf { 64u32 } else { 32 };
                if let Some(rm) = parse_reg(&ops[2]) {
                    same_size(rd, rm)?;
                    let op2 = match mn {
                        "lsl" => 0,
                        "lsr" => 1,
                        "asr" => 2,
                        _ => 3,
                    };
                    return Ok(sfb(rd)
                        | 0x1AC02000
                        | ((rm.n as u32) << 16)
                        | (op2 << 10)
                        | ((rn.n as u32) << 5)
                        | rd.n as u32);
                }
                let s = range(ev(&ops[2])?, 0, bits as i64 - 1, "the shift")?;
                let n = (rd.sf as u32) << 22;
                let (opc, immr, imms) = match mn {
                    "lsl" => (2, (bits - s) % bits, bits - 1 - s),
                    "lsr" => (2, s, bits - 1),
                    "asr" => (0, s, bits - 1),
                    _ => {
                        // extr rd, rn, rn, #s
                        return Ok(sfb(rd)
                            | 0x13800000
                            | n
                            | ((rn.n as u32) << 16)
                            | (s << 10)
                            | ((rn.n as u32) << 5)
                            | rd.n as u32);
                    }
                };
                Ok(sfb(rd)
                    | (opc << 29)
                    | 0x13000000
                    | n
                    | (immr << 16)
                    | (imms << 10)
                    | ((rn.n as u32) << 5)
                    | rd.n as u32)
            }
            "sxtb" | "sxth" | "sxtw" | "uxtb" | "uxth" => {
                want(2)?;
                let (rd, rn) = (reg(0)?, reg(1)?);
                let imms = match mn {
                    "sxtb" | "uxtb" => 7,
                    "sxth" | "uxth" => 15,
                    _ => 31,
                };
                if mn == "sxtw" && !rd.sf {
                    return Err("sxtw needs an x register as its first operand".into());
                }
                let signed = mn.starts_with('s');
                let sf = rd.sf && signed;
                let opc = if signed { 0 } else { 2 };
                Ok(((sf as u32) << 31)
                    | (opc << 29)
                    | 0x13000000
                    | ((sf as u32) << 22)
                    | (imms << 10)
                    | ((rn.n as u32) << 5)
                    | rd.n as u32)
            }
            "csel" | "csinc" | "csinv" | "csneg" | "cset" | "csetm" | "cinc" | "cneg" => {
                let (rd, rn, rm, cond_s, kind, invert) = match mn {
                    "cset" | "csetm" => {
                        want(2)?;
                        let rd = reg(0)?;
                        (rd, 31, 31, &ops[1], if mn == "cset" { 1 } else { 2 }, true)
                    }
                    "cinc" | "cneg" => {
                        want(3)?;
                        let rd = reg(0)?;
                        let rn = reg(1)?;
                        same_size(rd, rn)?;
                        (rd, rn.n, rn.n, &ops[2], if mn == "cinc" { 1 } else { 3 }, true)
                    }
                    _ => {
                        want(4)?;
                        let (rd, rn, rm) = (reg(0)?, reg(1)?, reg(2)?);
                        same_size(rd, rn)?;
                        same_size(rd, rm)?;
                        let kind = match mn {
                            "csel" => 0,
                            "csinc" => 1,
                            "csinv" => 2,
                            _ => 3,
                        };
                        (rd, rn.n, rm.n, &ops[3], kind, false)
                    }
                };
                let mut c = cond_from_name(&cond_s.to_ascii_lowercase())
                    .ok_or_else(|| format!("{cond_s} is not a condition"))?;
                if invert {
                    if c >= 14 {
                        return Err("al and nv make no sense here".into());
                    }
                    c ^= 1;
                }
                let (op, o2) = match kind {
                    0 => (0, 0),
                    1 => (0, 1),
                    2 => (1, 0),
                    _ => (1, 1),
                };
                Ok(sfb(rd)
                    | (op << 30)
                    | 0x1A800000
                    | ((rm as u32) << 16)
                    | ((c as u32) << 12)
                    | (o2 << 10)
                    | ((rn as u32) << 5)
                    | rd.n as u32)
            }
            "stp" | "ldp" => {
                if ops.len() != 3 && ops.len() != 4 {
                    return Err(format!("{mn} is written {mn} x0, x1, [sp, #-16]!"));
                }
                let (rt, rt2) = (reg(0)?, reg(1)?);
                same_size(rt, rt2)?;
                let (base, mem) = parse_mem(&ops[2], ops.get(3).map(|s| s.as_str()))?;
                let scale = if rt.sf { 8 } else { 4 };
                let (kind, off) = match mem {
                    Mem::Off(e) => (2, e),
                    Mem::Pre(e) => (3, e),
                    Mem::Post(e) => (1, e),
                    Mem::Reg { .. } => return Err(format!("{mn} takes a number offset, not a register")),
                };
                let v = ev(&off)?;
                if v % scale != 0 {
                    return Err(format!("the offset must be a multiple of {scale}"));
                }
                let imm7 = range(v / scale, -64, 63, "the offset (divided by the register size)")? & 0x7f;
                Ok(((rt.sf as u32) << 31)
                    | 0x28000000
                    | (kind << 23)
                    | (((mn == "ldp") as u32) << 22)
                    | (imm7 << 15)
                    | ((rt2.n as u32) << 10)
                    | ((base.n as u32) << 5)
                    | rt.n as u32)
            }
            "ldr" | "str" | "ldrb" | "strb" | "ldrh" | "strh" | "ldrsb" | "ldrsh" | "ldrsw" | "ldur" | "stur"
            | "ldurb" | "sturb" | "ldurh" | "sturh" | "ldursb" | "ldursh" | "ldursw" => {
                if ops.len() != 2 && ops.len() != 3 {
                    return Err(format!("{mn} is written {mn} x0, [x1]  or  {mn} x0, [x1, #8]"));
                }
                let rt = reg(0)?;
                let unscaled = mn.starts_with("ldu") || mn.starts_with("stu");
                let base_mn = mn.replace("ldur", "ldr").replace("stur", "str");
                let store = base_mn.starts_with("st");
                let (size, opc): (u32, u32) = match base_mn.as_str() {
                    "ldr" | "str" => (if rt.sf { 3 } else { 2 }, if store { 0 } else { 1 }),
                    "ldrb" | "strb" => (0, if store { 0 } else { 1 }),
                    "ldrh" | "strh" => (1, if store { 0 } else { 1 }),
                    "ldrsb" => (0, if rt.sf { 2 } else { 3 }),
                    "ldrsh" => (1, if rt.sf { 2 } else { 3 }),
                    "ldrsw" => {
                        if !rt.sf {
                            return Err("ldrsw needs an x register".into());
                        }
                        (2, 2)
                    }
                    _ => unreachable!(),
                };
                if matches!(base_mn.as_str(), "ldrb" | "strb" | "ldrh" | "strh") && rt.sf {
                    return Err(format!("{mn} moves one small value; use a w register (w0-w30)"));
                }
                let src = &ops[1];
                // ldr rt, =expr  and  ldr rt, label  are pc-relative loads
                if !src.starts_with('[') {
                    if store {
                        return Err("str needs an address in brackets, like [x1]".into());
                    }
                    if size < 2 || (unscaled) {
                        return Err(format!("{mn} needs an address in brackets, like [x1]"));
                    }
                    let target = if let Some(lit) = src.strip_prefix('=') {
                        let lit = lit.trim();
                        let idx = self.pool.iter().position(|p| p == lit).ok_or("literal pool mix-up")?;
                        (TEXT_BASE + self.pool_base + 8 * idx as u64) as i64
                    } else {
                        ev(src)?
                    };
                    let off = branch_off(target, 19)?;
                    let opc = if base_mn == "ldrsw" { 2 } else { rt.sf as u32 };
                    return Ok((opc << 30) | 0x18000000 | (off << 5) | rt.n as u32);
                }
                let (base, mem) = parse_mem(src, ops.get(2).map(|s| s.as_str()))?;
                let common = (size << 30) | (opc << 22) | ((base.n as u32) << 5) | rt.n as u32;
                match mem {
                    Mem::Off(e) => {
                        let v = ev(&e)?;
                        let scaled = v >= 0 && v % (1 << size) == 0 && (v >> size) < 4096;
                        if scaled && !unscaled {
                            Ok(common | 0x39000000 | (((v >> size) as u32) << 10))
                        } else if (-256..=255).contains(&v) {
                            Ok(common | 0x38000000 | (((v as u32) & 0x1ff) << 12))
                        } else {
                            Err(format!(
                                "the offset {v} does not fit: use a multiple of {} up to {}, or -256 to 255",
                                1 << size,
                                4095 << size
                            ))
                        }
                    }
                    Mem::Pre(ref e) | Mem::Post(ref e) => {
                        let v = ev(e)?;
                        let v = range(v, -256, 255, "the offset")? & 0x1ff;
                        let idx = if matches!(mem, Mem::Pre(_)) { 3 } else { 1 };
                        Ok(common | 0x38000000 | (v << 12) | (idx << 10))
                    }
                    Mem::Reg { rm, ext, shift } => {
                        let option: u32 = match ext.as_str() {
                            "" | "lsl" => {
                                if !rm.sf {
                                    return Err("the index register must be an x register (or say uxtw/sxtw)".into());
                                }
                                3
                            }
                            "uxtw" => 2,
                            "sxtw" => 6,
                            "sxtx" => 7,
                            other => return Err(format!("{other} is not an extension I know (lsl, uxtw, sxtw, sxtx)")),
                        };
                        let s = match shift {
                            None => 0,
                            Some(e) => {
                                let v = ev(&e)?;
                                if v == 0 {
                                    0
                                } else if v == size as i64 {
                                    1
                                } else {
                                    return Err(format!(
                                        "the shift here must be #{size} (the size of what you load), or none"
                                    ));
                                }
                            }
                        };
                        Ok(common | 0x38200800 | ((rm.n as u32) << 16) | (option << 13) | (s << 12))
                    }
                }
            }
            other => Err(unknown_mnemonic(other)),
        }
    }
}

fn section_name(s: Section) -> &'static str {
    match s {
        Section::Text => "text",
        Section::Data => "data",
        Section::Bss => "bss",
    }
}

fn is_bcond_short(mn: &str) -> bool {
    mn.len() == 3 && mn.starts_with('b') && cond_from_name(&mn[1..]).is_some() && mn != "bic"
}

fn unknown_mnemonic(mn: &str) -> String {
    let near = MNEMONICS
        .iter()
        .filter(|m| strsim::levenshtein(m, mn) <= 2)
        .min_by_key(|m| strsim::levenshtein(m, mn));
    let mut msg = format!("I don't know the instruction '{mn}'.");
    if let Some(n) = near {
        msg.push_str(&format!(" Did you mean {n}?"));
    } else {
        msg.push_str(" See: man as");
    }
    msg
}

/// Re-export for callers that want to check a mnemonic.
pub fn known_mnemonic(mn: &str) -> bool {
    MNEMONICS.contains(&mn) || mn.starts_with("b.") || is_bcond_short(mn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(src: &str) -> Vec<u32> {
        let img = assemble(src).unwrap_or_else(|e| panic!("{e}"));
        img.text
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    #[test]
    fn hello_layout() {
        let img = assemble(
            "// hello\n.text\n_start:\n    mov x0, #1\n    adr x1, msg\n    mov x2, len\n    mov x8, #64\n    svc #0\n    mov x0, #0\n    mov x8, #93\n    svc #0\n.data\nmsg: .ascii \"Hello\\n\"\nlen = . - msg\n",
        )
        .unwrap();
        assert_eq!(img.text.len(), 32);
        assert_eq!(img.entry, TEXT_BASE);
        assert_eq!(img.symbol("msg"), Some(TEXT_BASE + 32));
        assert_eq!(img.data, b"Hello\n");
        assert_eq!(img.lines[0], (TEXT_BASE as u32, 4));
        // mov x2, len -> movz x2, #6
        assert_eq!(u32::from_le_bytes(img.text[8..12].try_into().unwrap()), 0xD28000C2);
    }

    #[test]
    fn encodings_match_the_real_ones() {
        let cases: &[(&str, u32)] = &[
            ("mov x0, #1", 0xD2800020),
            ("mov w0, #1", 0x52800020),
            ("mov x0, #-1", 0x92800000),
            ("mov x0, #0xff", 0xD2801FE0),
            ("mov x0, #0x10000", 0xD2A00020),
            ("mov x0, #0xffff0000ffff0000", 0xB2103FE0),
            ("mov x0, x1", 0xAA0103E0),
            ("mov x0, sp", 0x910003E0),
            ("mov sp, x0", 0x9100001F),
            ("add x0, x0, #1", 0x91000400),
            ("add x0, x1, x2", 0x8B020020),
            ("add x0, x1, x2, lsl #3", 0x8B020C20),
            ("sub sp, sp, #16", 0xD10043FF),
            ("add x0, x0, #-1", 0xD1000400),
            ("subs x0, x1, #2", 0xF1000820),
            ("cmp x0, #3", 0xF1000C1F),
            ("cmp x0, x1", 0xEB01001F),
            ("cmp w0, #10", 0x7100281F),
            ("neg x0, x1", 0xCB0103E0),
            ("and x0, x1, #0xff", 0x92401C20),
            ("orr x0, x1, x2", 0xAA020020),
            ("eor w0, w0, w0", 0x4A000000),
            ("tst x0, #1", 0xF240001F),
            ("mul x0, x1, x2", 0x9B027C20),
            ("udiv x2, x0, x1", 0x9AC10802),
            ("sdiv x2, x0, x1", 0x9AC10C02),
            ("msub x0, x1, x2, x3", 0x9B028C20),
            ("lsl x0, x0, #4", 0xD37CEC00),
            ("lsr x0, x0, #4", 0xD344FC00),
            ("asr x0, x0, #4", 0x9344FC00),
            ("lsl w0, w0, #1", 0x531F7800),
            ("lsl x0, x1, x2", 0x9AC22020),
            ("cset x0, eq", 0x9A9F17E0),
            ("cset w0, lt", 0x1A9FA7E0),
            ("csel x0, x1, x2, ne", 0x9A821020),
            ("b .", 0x14000000),
            ("bl .", 0x94000000),
            ("b.ne .", 0x54000001),
            ("bne .", 0x54000001),
            ("cbz x0, .", 0xB4000000),
            ("cbnz w1, .", 0x35000001),
            ("tbz x0, #3, .", 0x36180000),
            ("br x1", 0xD61F0020),
            ("blr x1", 0xD63F0020),
            ("ret", 0xD65F03C0),
            ("adr x1, .", 0x10000001),
            ("svc #0", 0xD4000001),
            ("brk #1", 0xD4200020),
            ("nop", 0xD503201F),
            ("ldr x0, [x1]", 0xF9400020),
            ("ldr x0, [x1, #8]", 0xF9400420),
            ("ldr w0, [x1, #4]", 0xB9400420),
            ("ldrb w0, [x1]", 0x39400020),
            ("strb w0, [x1, #1]", 0x39000420),
            ("ldrh w0, [x1]", 0x79400020),
            ("ldrsb x0, [x1]", 0x39800020),
            ("ldrsw x0, [x1]", 0xB9800020),
            ("str x0, [sp, #-16]!", 0xF81F0FE0),
            ("ldr x0, [sp], #16", 0xF84107E0),
            ("ldur x0, [x1, #-8]", 0xF85F8020),
            ("ldr x0, [x1, x2]", 0xF8626820),
            ("ldr x0, [x1, x2, lsl #3]", 0xF8627820),
            ("ldrb w0, [x1, x2]", 0x38626820),
            ("stp x29, x30, [sp, #-16]!", 0xA9BF7BFD),
            ("ldp x29, x30, [sp], #16", 0xA8C17BFD),
            ("stp x0, x1, [sp]", 0xA90007E0),
            ("movk x0, #0x1234, lsl #16", 0xF2A24680),
            ("movz x0, #5", 0xD28000A0),
            ("mvn x0, x1", 0xAA2103E0),
            ("sxtw x0, w1", 0x93407C20),
            ("uxtb w0, w1", 0x53001C20),
            ("ldr x0, .", 0x58000000),
        ];
        for (src, want) in cases {
            let got = words(src)[0];
            assert_eq!(got, *want, "{src}: got {got:08x} want {want:08x}");
            // and the disassembler reads them back as something the assembler accepts
            let text = crate::dis::format(got, TEXT_BASE, &|a| format!("0x{a:x}"));
            let again = assemble(&text).unwrap_or_else(|e| panic!("{src} -> {text}: {e}"));
            let w2 = u32::from_le_bytes(again.text[..4].try_into().unwrap());
            assert_eq!(w2, *want, "{src} -> {text} -> {w2:08x}");
        }
    }

    #[test]
    fn literal_pool_and_data_directives() {
        let img = assemble(
            ".text\nmain:\n ldr x0, =big\n ldr x1, =0x123456789\n ldr x2, =big\n ret\n.data\nbig: .quad 1, 2\n.word 7\n.byte 'A', -1\n.align 3\nq: .hword 5\n.bss\nbuf: .space 100\n",
        )
        .unwrap();
        // 4 instructions = 16 bytes, pool at 16, two entries
        assert_eq!(img.text.len(), 32);
        let big = img.symbol("big").unwrap();
        assert_eq!(u64::from_le_bytes(img.text[16..24].try_into().unwrap()), big);
        assert_eq!(u64::from_le_bytes(img.text[24..32].try_into().unwrap()), 0x123456789);
        assert_eq!(&img.data[..16], &[1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&img.data[16..22], &[7, 0, 0, 0, b'A', 0xff]);
        assert_eq!(img.symbol("q"), Some(big + 24));
        assert_eq!(img.bss, 100);
        assert_eq!(img.symbol("buf"), Some(big + 26)); // 16 + 4 + 2, padded to 24, + 2
    }

    #[test]
    fn errors_name_the_line() {
        let e = assemble("mov x0, #1\nmvo x1, #2\n").unwrap_err();
        assert_eq!(e.line, 2);
        assert!(e.msg.contains("Did you mean m"), "{e}");
        let e = assemble("b nowhere\n").unwrap_err();
        assert!(e.msg.contains("not seen a label called nowhere"), "{e}");
        let e = assemble("add x0, x0, #5000\n").unwrap_err();
        assert!(e.msg.contains("4095"), "{e}");
        let e = assemble("mov x0, w1\n").unwrap_err();
        assert!(e.msg.contains("mixing"), "{e}");
        let e = assemble("loop:\nloop:\n b loop\n").unwrap_err();
        assert!(e.msg.contains("defined twice"), "{e}");
        let e = assemble(".data\nmsg: .ascii \"hi\"\n.text\n.ascii \"x\"\n").unwrap_err();
        assert!(e.msg.contains(".data"), "{e}");
        let e = assemble("mov x0, #0x123456789\n").unwrap_err();
        assert!(e.msg.contains("ldr x0, ="), "{e}");
    }

    #[test]
    fn comments_labels_and_expressions() {
        let img = assemble(
            "// a comment\n; another\n   start: mov x0, #(2+3)*4  // twenty\nmov x1, #'A'\nmov x2, #1<<4\n.equ ten, 10\nmov x3, #ten\nmov x4, #ten*2\n",
        )
        .unwrap();
        let w = |i: usize| u32::from_le_bytes(img.text[i * 4..i * 4 + 4].try_into().unwrap());
        assert_eq!(w(0), 0xD2800280); // mov x0, #20
        assert_eq!(w(1), 0xD2800821); // mov x1, #65
        assert_eq!(w(2), 0xD2800202); // mov x2, #16
        assert_eq!(w(3), 0xD2800143); // mov x3, #10
        assert_eq!(w(4), 0xD2800284); // mov x4, #20
        assert_eq!(img.entry, TEXT_BASE);
    }
}
