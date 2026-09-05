//! `bug-hunt`: eight tiny programs, one bug each. The command copies them
//! to `~/bug-hunt`, runs the kid's copies in the emulator, and reports
//! which print the right thing. Finishing all eight earns a badge.

use crate::sys::{Io, ProcSys};
use crate::vm::{Fault, Step};
use crate::{asm, machine_for};
use kiddos_kernel::{CmdResult, Console, Proc};
use std::cell::RefCell;
use std::rc::Rc;

pub const DIR: &str = "~/bug-hunt";
const STEP_LIMIT: u64 = 200_000;

pub struct Puzzle {
    pub file: &'static str,
    pub expect: &'static str,
    pub hint: &'static str,
}

pub const PUZZLES: [Puzzle; 8] = [
    Puzzle {
        file: "01-hello.s",
        expect: "Hello!\n",
        hint: "How many bytes is \"Hello!\" plus the new line? Count them, or let the assembler: len = . - msg",
    },
    Puzzle {
        file: "02-add.s",
        expect: "5\n",
        hint: "Step to the add and read the line again slowly. Which two registers should it add?",
    },
    Puzzle {
        file: "03-count.s",
        expect: "12345\n",
        hint: "The loop stops when x19 reaches 5, before printing it. b.lt is 'less than'; is there a 'less or equal'?",
    },
    Puzzle {
        file: "04-ret.s",
        expect: "Hi!Hi!Bye\n",
        hint: "bl shout jumps to shout and remembers where to come back (x30). What instruction goes back there?",
    },
    Puzzle {
        file: "05-order.s",
        expect: "7\n",
        hint: "Watch the box with :mem box. The program takes the number out before it puts 7 in.",
    },
    Puzzle {
        file: "06-loop.s",
        expect: "*****\n",
        hint: "x19 should count up to 5. Which way does sub make it go?",
    },
    Puzzle {
        file: "07-strlen.s",
        expect: "5\n",
        hint: "A letter is one byte. ldr reads eight at once, so it never sees a lone zero. Which load reads one byte?",
    },
    Puzzle {
        file: "08-sign.s",
        expect: "negative\n",
        hint: "b.hi compares numbers as if they could never be negative, so -3 looks huge. b.gt knows about minus.",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Wrong(String),
    NoFile,
    AsmError(String),
    Crash(String),
    Forever,
}

struct Quiet {
    out: Rc<RefCell<Vec<u8>>>,
}

impl Io for Quiet {
    fn write(&mut self, _p: &Proc, _fd: u64, bytes: &[u8]) {
        self.out.borrow_mut().extend_from_slice(bytes);
    }
    fn read_line(&mut self, _p: &Proc) -> Result<Option<String>, Fault> {
        Ok(None)
    }
}

/// Assemble and run one program with no screen, and judge its output.
pub fn check(p: &Proc, path: &str, expect: &str) -> Outcome {
    let Ok(src) = p.fs().read_string(path) else {
        return Outcome::NoFile;
    };
    let img = match asm::assemble(&src) {
        Ok(i) => i,
        Err(e) => return Outcome::AsmError(e.to_string()),
    };
    let mut vm = machine_for(p, &img);
    let out = Rc::new(RefCell::new(Vec::new()));
    let mut sys = ProcSys::new(p, Box::new(Quiet { out: out.clone() }));
    let mut steps = 0u64;
    let result = loop {
        match vm.step(&mut sys) {
            Ok(Step::Ran) => {}
            Ok(Step::Exit(_)) => break Ok(()),
            Ok(Step::Brk(_)) => break Err("the program stopped at a brk".to_string()),
            Err(f) => {
                let line = img.line_of(vm.pc).map(|l| format!(" (line {l})")).unwrap_or_default();
                break Err(format!("{}{line}", f.explain(&|a| img.name_of(a))));
            }
        }
        steps += 1;
        if steps > STEP_LIMIT {
            return Outcome::Forever;
        }
        if steps % 4096 == 0 && p.killed() {
            return Outcome::Crash("stopped".into());
        }
    };
    let got = String::from_utf8_lossy(&out.borrow()).into_owned();
    match result {
        Err(e) => Outcome::Crash(e),
        Ok(()) if got == expect => Outcome::Pass,
        Ok(()) => Outcome::Wrong(got),
    }
}

fn show(s: &str) -> String {
    let s = s.replace('\n', "\\n");
    if s.chars().count() > 30 {
        format!("{}...", s.chars().take(30).collect::<String>())
    } else {
        s
    }
}

pub fn cmd_bug_hunt(p: &Proc, args: &[String]) -> CmdResult {
    let reset = args.iter().any(|a| a == "reset");
    let src_dir = p
        .env_get("CART")
        .map(|c| format!("{c}/programs"))
        .unwrap_or_else(|| "/games/bug-hunt/programs".into());
    if let Err(e) = p.fs().mkdir_p(DIR) {
        p.complain(&e);
        return Ok(1);
    }
    let mut copied = 0;
    for pz in &PUZZLES {
        let dst = format!("{DIR}/{}", pz.file);
        if reset || !p.fs().exists(&dst) {
            match p.fs().read(&format!("{src_dir}/{}", pz.file)) {
                Ok(data) => {
                    if p.fs().write(&dst, &data).is_ok() {
                        copied += 1;
                    }
                }
                Err(e) => {
                    p.complain(&e);
                    return Ok(1);
                }
            }
        }
    }
    p.println("\x1b[1;33mBUG HUNT\x1b[0m  eight programs, one bug each");
    if copied > 0 {
        p.println(&format!(
            "{} program{} {} in {DIR}. Read them, fix them, then play bug-hunt again.",
            copied,
            if copied == 1 { "" } else { "s" },
            if reset { "put back" } else { "copied" }
        ));
    }
    p.println("");
    let mut passed = 0;
    let mut first_hint: Option<(&Puzzle, Outcome)> = None;
    for pz in &PUZZLES {
        let path = format!("{DIR}/{}", pz.file);
        let outcome = check(p, &path, pz.expect);
        let (mark, note) = match &outcome {
            Outcome::Pass => ("\x1b[1;32mOK \x1b[0m", "fixed!".to_string()),
            Outcome::Wrong(got) => (
                "\x1b[1;31mBUG\x1b[0m",
                format!("expected \"{}\", got \"{}\"", show(pz.expect), show(got)),
            ),
            Outcome::NoFile => ("\x1b[1;31m???\x1b[0m", "the file is gone (play bug-hunt reset)".into()),
            Outcome::AsmError(e) => ("\x1b[1;31mERR\x1b[0m", format!("does not assemble: {e}")),
            Outcome::Crash(e) => ("\x1b[1;31mBUG\x1b[0m", format!("crashes: {e}")),
            Outcome::Forever => (
                "\x1b[1;31mBUG\x1b[0m",
                "never finishes (it was stopped after a while)".into(),
            ),
        };
        p.println(&format!("  {mark} {:<12} {note}", pz.file));
        if outcome == Outcome::Pass {
            passed += 1;
        } else if first_hint.is_none() {
            first_hint = Some((pz, outcome));
        }
    }
    p.println("");
    if passed == PUZZLES.len() {
        let badge = " .-------------.\n |  BUG HUNTER |\n |  8 / 8      |\n |  debug  s c |\n '-------------'\n";
        p.println("All eight fixed. You can read a program the way the CPU does now.");
        p.print(&format!("\x1b[1;33m{badge}\x1b[0m"));
        let _ = p.fs().mkdir_p("~/badges");
        let _ = p.fs().write("~/badges/bug-hunt.txt", badge.as_bytes());
        p.beep(880, 80);
        p.beep(1320, 120);
        return Ok(0);
    }
    p.println(&format!("{passed} of {} fixed.", PUZZLES.len()));
    if let Some((pz, _)) = first_hint {
        p.println(&format!(
            "Next: \x1b[1m{DIR}/{}\x1b[0m   Look:  debug {DIR}/{}   Fix:  edit {DIR}/{}",
            pz.file, pz.file, pz.file
        ));
        if args.iter().any(|a| a == "hint") {
            p.println(&format!("Hint: {}", pz.hint));
        } else {
            p.println("Stuck? play bug-hunt hint");
        }
    }
    Ok(0)
}
