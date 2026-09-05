//! `debug prog`: the full-screen debugger. Source on the left with the
//! current line lit, registers on the right (changed ones flash), a
//! memory window, the program's output, and one line for commands.
//!
//! Not gdb's prompt: a kid presses `s` and watches numbers change.

use crate::image::Image;
use crate::sys::{Io, ProcSys};
use crate::vm::{Fault, Step, Vm, TEXT_BASE};
use crate::{dis, insn, load_program, machine_for};
use kiddos_console::colors;
use kiddos_kernel::{CmdResult, Console, Key, Proc};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

const SRC_WIDTH: u16 = 48;
const HELP: &str = "s step  n next  c continue  b break  r restart  q quit  : command";

// CGA colors
const BLACK: u8 = 0;
const CYAN: u8 = 3;
const RED: u8 = 4;
const GRAY: u8 = 7;
const DARK: u8 = 8;
const LIGHT_GREEN: u8 = 10;
const LIGHT_CYAN: u8 = 11;
const LIGHT_RED: u8 = 12;
const YELLOW: u8 = 14;
const WHITE: u8 = 15;

pub fn cmd_debug(p: &Proc, args: &[String]) -> CmdResult {
    if !p.stdout_is_tty() || !p.stdin_is_tty() {
        p.eprintln("debug: I need the screen and the keyboard for this.");
        return Ok(1);
    }
    let Some(file) = args.first() else {
        p.println(&p.t(
            "usage",
            &[("usage", "debug <program>    (an assembled program, or a .s file)")],
        ));
        return Ok(1);
    };
    let img = match load_program(p, file) {
        Ok(i) => i,
        Err(e) => {
            p.eprintln(&format!("debug: {e}"));
            return Ok(1);
        }
    };
    let name = kiddos_vfs::basename(file).to_string();
    let mut d = Debugger::new(p, img, name);
    let r = d.run();
    p.set_color(colors::DEFAULT_FG, colors::DEFAULT_BG);
    p.print("\x1b[0m\x1b[2J\x1b[H");
    p.cursor_show(true);
    r
}

/// What the program is doing right now.
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Ready,
    Exited(i32),
    Faulted(String),
}

/// A screen full of cells, flushed with `put` where it changed.
struct Canvas {
    cols: u16,
    rows: u16,
    cells: Vec<(char, u8, u8)>,
    shown: Vec<(char, u8, u8)>,
}

impl Canvas {
    fn new(cols: u16, rows: u16) -> Canvas {
        let n = cols as usize * rows as usize;
        Canvas {
            cols,
            rows,
            cells: vec![(' ', GRAY, BLACK); n],
            shown: vec![('\0', 0, 0); n],
        }
    }
    fn clear(&mut self) {
        self.cells.iter_mut().for_each(|c| *c = (' ', GRAY, BLACK));
    }
    fn text(&mut self, x: u16, y: u16, s: &str, fg: u8, bg: u8, max: u16) {
        if y >= self.rows {
            return;
        }
        let mut cx = x;
        for ch in s.chars() {
            if cx >= self.cols || cx >= x + max {
                break;
            }
            self.cells[y as usize * self.cols as usize + cx as usize] = (ch, fg, bg);
            cx += 1;
        }
    }
    /// Fill a row segment with `bg` (and blank it).
    fn band(&mut self, x: u16, y: u16, w: u16, fg: u8, bg: u8) {
        for cx in x..(x + w).min(self.cols) {
            self.cells[y as usize * self.cols as usize + cx as usize] = (' ', fg, bg);
        }
    }
    fn flush(&mut self, p: &Proc) {
        for i in 0..self.cells.len() {
            if self.cells[i] != self.shown[i] {
                let (ch, fg, bg) = self.cells[i];
                p.put(
                    (i % self.cols as usize) as u16,
                    (i / self.cols as usize) as u16,
                    ch,
                    fg,
                    bg,
                );
                self.shown[i] = self.cells[i];
            }
        }
    }
}

/// Output into a shared buffer; input on the status row.
struct Capture {
    out: Rc<RefCell<Vec<u8>>>,
    status_row: u16,
    cols: u16,
}

impl Io for Capture {
    fn write(&mut self, _p: &Proc, _fd: u64, bytes: &[u8]) {
        self.out.borrow_mut().extend_from_slice(bytes);
    }
    fn read_line(&mut self, p: &Proc) -> Result<Option<String>, Fault> {
        let mut line = String::new();
        loop {
            let prompt = format!("The program wants a line of input: {line}");
            for x in 0..self.cols {
                p.put(x, self.status_row, ' ', BLACK, GRAY);
            }
            for (i, ch) in prompt.chars().take(self.cols as usize - 1).enumerate() {
                p.put(i as u16, self.status_row, ch, BLACK, GRAY);
            }
            p.cursor((prompt.chars().count() as u16).min(self.cols - 1), self.status_row);
            p.cursor_show(true);
            match p.readkey().map_err(|_| Fault::Interrupted)? {
                Key::Enter => return Ok(Some(line)),
                Key::Backspace => {
                    line.pop();
                }
                Key::Ctrl('c') | Key::Escape => return Err(Fault::Interrupted),
                Key::Ctrl('d') if line.is_empty() => return Ok(None),
                Key::Char(c) => line.push(c),
                _ => {}
            }
        }
    }
}

struct Debugger<'a> {
    p: &'a Proc,
    img: Image,
    name: String,
    vm: Vm,
    src: Vec<String>,
    breaks: BTreeSet<u32>,
    /// Selected source line (1-based) and the first line shown.
    sel: usize,
    scroll: usize,
    prev_x: [u64; 31],
    prev_sp: u64,
    prev_flags: String,
    mem_addr: u64,
    out: Rc<RefCell<Vec<u8>>>,
    status: String,
    state: State,
    canvas: Canvas,
    cmdline: Option<String>,
}

impl<'a> Debugger<'a> {
    fn new(p: &'a Proc, img: Image, name: String) -> Debugger<'a> {
        let (cols, rows) = p.size();
        let vm = machine_for(p, &img);
        let src: Vec<String> = img.source.lines().map(|l| l.trim_end().to_string()).collect();
        let mem_addr = img
            .symbols
            .iter()
            .find(|(_, a)| *a >= TEXT_BASE + img.text.len() as u64)
            .map(|(_, a)| *a)
            .unwrap_or(vm.data_base());
        let mut d = Debugger {
            p,
            img,
            name,
            prev_x: vm.x,
            prev_sp: vm.sp,
            prev_flags: vm.flags(),
            vm,
            src,
            breaks: BTreeSet::new(),
            sel: 1,
            scroll: 0,
            mem_addr,
            out: Rc::new(RefCell::new(Vec::new())),
            status: String::new(),
            state: State::Ready,
            canvas: Canvas::new(cols, rows),
            cmdline: None,
        };
        d.sel = d.pc_line().unwrap_or(1);
        d.status = format!("{}: ready at line {}. {HELP}", d.name, d.sel);
        d
    }

    fn pc_line(&self) -> Option<usize> {
        self.img.line_of(self.vm.pc).map(|l| l as usize)
    }

    fn sys(&self) -> ProcSys<'a> {
        let (cols, rows) = self.p.size();
        ProcSys::new(
            self.p,
            Box::new(Capture {
                out: self.out.clone(),
                status_row: rows - 1,
                cols,
            }),
        )
    }

    fn snapshot(&mut self) {
        self.prev_x = self.vm.x;
        self.prev_sp = self.vm.sp;
        self.prev_flags = self.vm.flags();
    }

    fn follow_pc(&mut self) {
        if let Some(l) = self.pc_line() {
            self.sel = l;
        }
    }

    /// One instruction. Returns false when the program is no longer runnable.
    fn step_once(&mut self, sys: &mut ProcSys<'a>) -> bool {
        if self.state != State::Ready {
            self.status = match &self.state {
                State::Exited(c) => format!("The program already finished (exit code {c}). r restarts it."),
                State::Faulted(_) => "The program crashed. r restarts it.".into(),
                State::Ready => String::new(),
            };
            return false;
        }
        match self.vm.step(sys) {
            Ok(Step::Ran) => true,
            Ok(Step::Exit(code)) => {
                self.state = State::Exited(code);
                self.status = format!("The program finished with exit code {code}. r restarts, q quits.");
                false
            }
            Ok(Step::Brk(n)) => {
                self.status = format!("brk #{n}: the program's own breakpoint. Look around, then s or c.");
                false
            }
            Err(Fault::Interrupted) => {
                if self.p.killed() {
                    self.state = State::Faulted("Stopped.".into());
                }
                self.status = "Stopped. s steps on, c continues.".into();
                false
            }
            Err(f) => {
                let msg = f.explain(&|a| self.img.name_of(a));
                self.state = State::Faulted(msg.clone());
                self.status = msg;
                false
            }
        }
    }

    fn step(&mut self) {
        self.snapshot();
        let mut sys = self.sys();
        let before = self.vm.pc;
        if self.step_once(&mut sys) {
            self.status = self.describe(before);
        }
        self.follow_pc();
    }

    /// Run until `stop(pc)` says so, a breakpoint, the end, or Ctrl-C.
    fn run_until(&mut self, stop: &dyn Fn(u64) -> bool) {
        self.snapshot();
        let mut sys = self.sys();
        let start = self.vm.pc;
        let mut n = 0u64;
        loop {
            if !self.step_once(&mut sys) {
                break;
            }
            n += 1;
            let pc = self.vm.pc;
            if stop(pc) {
                self.status = self.describe_stop(n);
                break;
            }
            if pc != start {
                if let Some(l) = self.img.line_of(pc) {
                    if self.breaks.contains(&l) {
                        self.status = format!("Breakpoint at line {l}, after {n} instructions.");
                        break;
                    }
                }
            }
            if n % 4096 == 0 {
                if self.p.killed() {
                    self.state = State::Faulted("Stopped.".into());
                    break;
                }
                if self.p.take_key_if(|k| *k == Key::Ctrl('c')).is_some() {
                    self.status = format!("Stopped by Ctrl-C after {n} instructions. Is the program looping?");
                    break;
                }
                std::thread::yield_now();
            }
        }
        self.follow_pc();
    }

    fn describe_stop(&self, n: u64) -> String {
        format!("Ran {n} instruction{}.", if n == 1 { "" } else { "s" })
    }

    /// What the instruction at `pc` did, from the registers that changed.
    fn describe(&self, pc: u64) -> String {
        let text = self
            .vm
            .fetch(pc)
            .map(|w| dis::format(w, pc, &|a| self.short_name(a)))
            .unwrap_or_default();
        let mut changes = Vec::new();
        for i in 0..31 {
            if self.vm.x[i] != self.prev_x[i] {
                changes.push(format!("x{i} = {}", self.fmt_value(self.vm.x[i])));
            }
        }
        if self.vm.sp != self.prev_sp {
            changes.push(format!("sp = {}", self.fmt_value(self.vm.sp)));
        }
        let flags = self.vm.flags();
        if flags != self.prev_flags {
            changes.push(format!("flags {flags}"));
        }
        if changes.is_empty() {
            if self.vm.pc != pc + 4 {
                format!("{text}  ->  jumped to line {}", self.pc_line().unwrap_or(0))
            } else {
                format!("{text}  ->  nothing changed")
            }
        } else {
            format!("{text}  ->  {}", changes.join(", "))
        }
    }

    fn short_name(&self, a: u64) -> String {
        match self.img.symbols.iter().find(|(_, x)| *x == a) {
            Some((n, _)) => n.clone(),
            None => format!("0x{a:x}"),
        }
    }

    /// A register value the way a kid reads it: small numbers in decimal,
    /// addresses with their label, the rest in hex.
    fn fmt_value(&self, v: u64) -> String {
        let s = v as i64;
        if (-999_999_999_999..=999_999_999_999).contains(&s) {
            if v >= TEXT_BASE && v <= self.vm.memory_size() && v > 4096 {
                let label = self.label_for(v);
                if !label.is_empty() {
                    return format!("0x{v:x} {label}");
                }
                if v >= self.vm.data_end && v > self.vm.memory_size() - 65536 {
                    return format!("0x{v:x} (stack)");
                }
                if v < self.vm.text_end {
                    return format!("0x{v:x} (line {})", self.img.line_of(v).unwrap_or(0));
                }
            }
            return s.to_string();
        }
        format!("0x{v:x}")
    }

    fn label_for(&self, v: u64) -> String {
        let mut best: Option<(&str, u64)> = None;
        for (name, a) in &self.img.symbols {
            if *a <= v && v - a < 256 && best.is_none_or(|(_, ba)| *a > ba) {
                best = Some((name, *a));
            }
        }
        match best {
            Some((n, a)) if a == v => n.to_string(),
            Some((n, a)) => format!("{n}+{}", v - a),
            None => String::new(),
        }
    }

    fn restart(&mut self) {
        self.vm = machine_for(self.p, &self.img);
        self.out.borrow_mut().clear();
        self.state = State::Ready;
        self.snapshot();
        self.follow_pc();
        self.status = format!("Back at the start. {HELP}");
    }

    fn toggle_break(&mut self, line: usize) {
        let l = line as u32;
        if self.img.addr_of_line(l).is_none() {
            self.status = format!("Line {line} has no instruction to stop at.");
            return;
        }
        if self.breaks.remove(&l) {
            self.status = format!("Breakpoint at line {line} removed.");
        } else {
            self.breaks.insert(l);
            self.status = format!("Breakpoint at line {line}: c will stop there.");
        }
    }

    /// `mem x1`, `mem msg`, `mem 0x10040`, `mem sp+16`: an address.
    fn eval_addr(&self, s: &str) -> Result<u64, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("which address? Try: mem msg  or  mem sp  or  mem 0x10040".into());
        }
        let lookup = |name: &str| -> Result<i64, String> {
            let n = name.to_ascii_lowercase();
            if n == "sp" {
                return Ok(self.vm.sp as i64);
            }
            if n == "pc" {
                return Ok(self.vm.pc as i64);
            }
            if let Some(r) = n
                .strip_prefix('x')
                .and_then(|r| r.parse::<usize>().ok())
                .filter(|r| *r < 31)
            {
                return Ok(self.vm.x[r] as i64);
            }
            if n == "lr" {
                return Ok(self.vm.x[30] as i64);
            }
            if n == "fp" {
                return Ok(self.vm.x[29] as i64);
            }
            self.img
                .symbol(name)
                .map(|a| a as i64)
                .ok_or_else(|| format!("{name} is not a label or a register I know"))
        };
        let img = crate::asm::eval_with(s, &lookup)?;
        Ok(img as u64)
    }

    fn command(&mut self, line: &str) -> bool {
        let line = line.trim();
        let (cmd, arg) = match line.find(' ') {
            Some(i) => (&line[..i], line[i..].trim()),
            None => (line, ""),
        };
        match cmd {
            "" => {}
            "q" | "quit" => return false,
            "mem" | "m" | "x" => match self.eval_addr(arg) {
                Ok(a) => {
                    self.mem_addr = a;
                    self.status = format!("Memory window at {}.", self.img.name_of(a));
                }
                Err(e) => self.status = e,
            },
            "break" | "b" => match arg.parse::<usize>() {
                Ok(n) => self.toggle_break(n),
                Err(_) => match self.img.symbol(arg).and_then(|a| self.img.line_of(a)) {
                    Some(l) => self.toggle_break(l as usize),
                    None => self.status = "break takes a line number, or a label: break 12, break loop".into(),
                },
            },
            "delete" | "d" => {
                self.breaks.clear();
                self.status = "All breakpoints removed.".into();
            }
            "reg" | "r" | "p" | "print" => match self.eval_addr(arg) {
                Ok(v) => self.status = format!("{arg} = {} (0x{v:x})", self.fmt_value(v)),
                Err(e) => self.status = e,
            },
            "step" | "s" => self.step(),
            "next" | "n" => self.next(),
            "cont" | "c" | "run" => self.run_until(&|_| false),
            "restart" => self.restart(),
            "help" | "h" | "?" => self.status = format!("{HELP}  :mem ADDR  :break LINE  :reg xN  :delete"),
            "goto" | "g" => match arg.parse::<usize>() {
                Ok(n) if n >= 1 && n <= self.src.len() => self.sel = n,
                _ => self.status = "goto takes a line number".into(),
            },
            other => self.status = format!("I don't know the command '{other}'. Try :help"),
        }
        true
    }

    /// Step over a `bl`: run until the instruction after it.
    fn next(&mut self) {
        let pc = self.vm.pc;
        let is_call = matches!(
            self.vm.fetch(pc).map(insn::decode),
            Ok(insn::Insn::Bl { .. }) | Ok(insn::Insn::Blr { .. })
        );
        if is_call {
            let after = pc + 4;
            self.run_until(&move |p| p == after);
        } else {
            self.step();
        }
    }

    // ---- drawing --------------------------------------------------------------

    fn draw(&mut self) {
        let (cols, rows) = (self.canvas.cols, self.canvas.rows);
        self.canvas.clear();
        let src_rows = rows.saturating_sub(7).max(3) as usize; // rows 1..=src_rows
        let mem_top = 1 + src_rows as u16;
        let out_top = mem_top + 2;
        let status_row = rows - 1;
        let reg_x = SRC_WIDTH + 1;
        let reg_w = cols.saturating_sub(reg_x);

        // headers
        self.canvas.band(0, 0, cols, BLACK, CYAN);
        let title = format!(" {} ", self.name);
        self.canvas.text(0, 0, &title, BLACK, CYAN, SRC_WIDTH);
        let st = match &self.state {
            State::Ready => format!(" pc {}  steps {} ", self.img.name_of(self.vm.pc), self.vm.steps),
            State::Exited(c) => format!(" finished (exit {c})  steps {} ", self.vm.steps),
            State::Faulted(_) => format!(" crashed at {}  steps {} ", self.img.name_of(self.vm.pc), self.vm.steps),
        };
        self.canvas.text(reg_x, 0, &st, BLACK, CYAN, reg_w);

        // source
        if self.sel < 1 {
            self.sel = 1;
        }
        if self.sel > self.src.len() {
            self.sel = self.src.len().max(1);
        }
        if self.sel - 1 < self.scroll {
            self.scroll = self.sel - 1;
        }
        if self.sel > self.scroll + src_rows {
            self.scroll = self.sel - src_rows;
        }
        let pc_line = self.pc_line();
        for r in 0..src_rows {
            let y = 1 + r as u16;
            let idx = self.scroll + r;
            let Some(text) = self.src.get(idx) else { break };
            let line = idx + 1;
            let is_pc = pc_line == Some(line) && self.state == State::Ready;
            let is_sel = line == self.sel;
            let has_insn = self.img.addr_of_line(line as u32).is_some();
            let (fg, bg) = match (&self.state, is_pc, is_sel) {
                (State::Faulted(_), _, true) if pc_line == Some(line) => (WHITE, RED),
                (_, true, _) => (BLACK, CYAN),
                (_, false, true) => (WHITE, DARK),
                _ => (if has_insn { GRAY } else { CYAN }, BLACK),
            };
            self.canvas.band(0, y, SRC_WIDTH, fg, bg);
            let mark = if self.breaks.contains(&(line as u32)) { '*' } else { ' ' };
            let arrow = if is_pc { '>' } else { ' ' };
            let shown = format!("{mark}{arrow}{line:>3} {text}");
            self.canvas.text(0, y, &shown, fg, bg, SRC_WIDTH);
            if mark == '*' {
                self.canvas.text(0, y, "*", LIGHT_RED, bg, 1);
            }
        }
        for r in 0..src_rows {
            let y = 1 + r as u16;
            self.canvas.text(SRC_WIDTH, y, "|", DARK, BLACK, 1);
        }

        // registers
        let mut rows_out: Vec<(String, String, bool)> = Vec::new();
        for i in 0..12 {
            rows_out.push((
                format!("x{i}"),
                self.fmt_value(self.vm.x[i]),
                self.vm.x[i] != self.prev_x[i],
            ));
        }
        let others: Vec<usize> = (12..29)
            .filter(|i| self.vm.x[*i] != 0 || self.vm.x[*i] != self.prev_x[*i])
            .collect();
        let summary = match others.first() {
            None => ("x12-x28".to_string(), "all 0".to_string(), false),
            Some(i) => {
                let more = if others.len() > 1 {
                    format!(" +{}", others.len() - 1)
                } else {
                    String::new()
                };
                (
                    format!("x{i}"),
                    format!("{}{more}", self.fmt_value(self.vm.x[*i])),
                    self.vm.x[*i] != self.prev_x[*i],
                )
            }
        };
        rows_out.push(summary);
        rows_out.push((
            "x29 fp".into(),
            self.fmt_value(self.vm.x[29]),
            self.vm.x[29] != self.prev_x[29],
        ));
        rows_out.push((
            "x30 lr".into(),
            self.fmt_value(self.vm.x[30]),
            self.vm.x[30] != self.prev_x[30],
        ));
        rows_out.push(("sp".into(), self.fmt_value(self.vm.sp), self.vm.sp != self.prev_sp));
        rows_out.push(("pc".into(), self.img.name_of(self.vm.pc), true));
        let flags = self.vm.flags();
        let fl = format!(
            "{}  {}",
            flags,
            ["eq", "ne", "lt", "le", "gt", "ge", "lo", "hs"]
                .iter()
                .filter(|c| self.vm.cond(insn::cond_from_name(c).unwrap()))
                .cloned()
                .collect::<Vec<_>>()
                .join(" ")
        );
        rows_out.push(("flags".into(), fl, flags != self.prev_flags));
        for (i, (name, val, changed)) in rows_out.iter().enumerate() {
            let y = 1 + i as u16;
            if y as usize > src_rows {
                break;
            }
            let fg = if *changed && name != "pc" { YELLOW } else { GRAY };
            self.canvas
                .text(reg_x, y, name, if name == "pc" { LIGHT_CYAN } else { fg }, BLACK, 8);
            self.canvas.text(reg_x + 8, y, val, fg, BLACK, reg_w.saturating_sub(8));
        }

        // memory window: two rows of 8 bytes
        let a = self.mem_addr;
        let label = self.img.name_of(a);
        self.canvas
            .text(0, mem_top, &format!("mem {label}"), LIGHT_CYAN, BLACK, 24);
        for r in 0..2u16 {
            let base = a.wrapping_add(8 * r as u64);
            let bytes: Vec<Option<u8>> = (0..8)
                .map(|i| self.vm.read(base.wrapping_add(i), 1).ok().map(|b| b[0]))
                .collect();
            let mut hex = String::new();
            let mut asc = String::new();
            for b in &bytes {
                match b {
                    Some(b) => {
                        hex.push_str(&format!("{b:02x} "));
                        asc.push(if (0x20..0x7f).contains(b) { *b as char } else { '.' });
                    }
                    None => {
                        hex.push_str("?? ");
                        asc.push(' ');
                    }
                }
            }
            let x = 25;
            self.canvas
                .text(x, mem_top + r, &format!("0x{base:x}"), DARK, BLACK, 10);
            self.canvas.text(x + 10, mem_top + r, &hex, GRAY, BLACK, 24);
            self.canvas.text(x + 35, mem_top + r, &asc, LIGHT_GREEN, BLACK, 8);
            // the 8 bytes as one number (when it is a small one), and where sp is
            if let Ok(v) = self.vm.read_u(base, 3) {
                if (v as i64).abs() < 1_000_000_000 {
                    self.canvas.text(
                        x + 44,
                        mem_top + r,
                        &format!("= {}", v as i64),
                        CYAN,
                        BLACK,
                        cols.saturating_sub(x + 44),
                    );
                }
            }
            if base == self.vm.sp {
                self.canvas.text(x + 7, mem_top + r, "sp>", YELLOW, BLACK, 3);
            }
        }

        // output
        let out = self.out.borrow();
        let text = String::from_utf8_lossy(&out);
        let mut lines: Vec<String> = Vec::new();
        for l in text.split('\n') {
            let chars: Vec<char> = l.chars().collect();
            if chars.is_empty() {
                lines.push(String::new());
                continue;
            }
            for chunk in chars.chunks(cols as usize - 8) {
                lines.push(chunk.iter().collect());
            }
        }
        if text.ends_with('\n') {
            lines.pop();
        }
        let out_rows = (status_row - out_top) as usize;
        let start = lines.len().saturating_sub(out_rows);
        self.canvas.text(0, out_top, "output", LIGHT_CYAN, BLACK, 7);
        for (i, l) in lines[start..].iter().enumerate() {
            self.canvas.text(8, out_top + i as u16, l, WHITE, BLACK, cols - 8);
        }
        if lines.is_empty() {
            self.canvas.text(8, out_top, "(nothing yet)", DARK, BLACK, 20);
        }
        drop(out);

        // status / command line
        let (fg, bg) = match (&self.cmdline, &self.state) {
            (Some(_), _) => (WHITE, BLACK),
            (None, State::Faulted(_)) => (WHITE, RED),
            (None, State::Exited(_)) => (BLACK, LIGHT_GREEN),
            _ => (BLACK, GRAY),
        };
        self.canvas.band(0, status_row, cols, fg, bg);
        let line = match &self.cmdline {
            Some(c) => format!(":{c}"),
            None => self.status.clone(),
        };
        self.canvas.text(0, status_row, &line, fg, bg, cols);
        self.canvas.flush(self.p);
        match &self.cmdline {
            Some(c) => {
                self.p.cursor((c.chars().count() as u16 + 1).min(cols - 1), status_row);
                self.p.cursor_show(true);
            }
            None => self.p.cursor_show(false),
        }
    }

    fn run(&mut self) -> CmdResult {
        self.p.print("\x1b[2J");
        loop {
            self.draw();
            let k = self.p.readkey()?;
            if let Some(mut c) = self.cmdline.take() {
                match k {
                    Key::Enter => {
                        if !self.command(&c) {
                            return Ok(0);
                        }
                    }
                    Key::Escape => {}
                    Key::Backspace => {
                        c.pop();
                        self.cmdline = Some(c);
                    }
                    Key::Char(ch) => {
                        c.push(ch);
                        self.cmdline = Some(c);
                    }
                    _ => self.cmdline = Some(c),
                }
                continue;
            }
            match k {
                Key::Char('q') => return Ok(0),
                Key::Char('s') | Key::Enter | Key::Char(' ') => self.step(),
                Key::Char('n') => self.next(),
                Key::Char('c') => self.run_until(&|_| false),
                Key::Char('b') => self.toggle_break(self.sel),
                Key::Char('r') => self.restart(),
                Key::Char(':') => self.cmdline = Some(String::new()),
                Key::Char('?') | Key::Char('h') => self.status = format!("{HELP}  :mem ADDR  :break LINE  :reg xN"),
                Key::Up | Key::Char('k') => self.sel = self.sel.saturating_sub(1).max(1),
                Key::Down | Key::Char('j') => self.sel = (self.sel + 1).min(self.src.len().max(1)),
                Key::PageUp => self.sel = self.sel.saturating_sub(10).max(1),
                Key::PageDown => self.sel = (self.sel + 10).min(self.src.len().max(1)),
                Key::Home | Key::Char('g') => self.sel = 1,
                Key::End | Key::Char('G') => self.sel = self.src.len().max(1),
                Key::Char('.') => self.follow_pc(),
                Key::Ctrl('c') => self.status = "q quits the debugger. (Ctrl-C stops a running program.)".into(),
                _ => {}
            }
        }
    }
}
