//! BASIC. EndBASIC's interpreter runs inside a KidDOS process: its console is
//! the process's console, its drive is a folder on the virtual drive, and a
//! handful of KidDOS statements (SPEAK, BEEP, KEY$, TICK, PUT) map 1:1 onto
//! the console API so BASIC games are portable to the other bindings.

mod console;
mod drive;
mod ext;

use console::KidConsole;
use drive::KidDriveFactory;
use endbasic_core::exec::{Machine, StopReason};
use endbasic_std::console::Console as EbConsole;
use endbasic_std::program::Program;
use endbasic_std::MachineBuilder;
use futures_lite::future::FutureExt;
use kiddos_kernel::{CmdResult, Command, Console, Kernel, Proc, Topic, KID_HOME};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

pub const WELCOME: &str = "\x1b[1;36mKidDOS BASIC\x1b[0m (EndBASIC 0.12)\n\
Type a line and press Enter to run it. EDIT writes a program, RUN runs it,\n\
SAVE \"name\" keeps it in your home folder, HELP explains everything, EXIT leaves.\n";

pub fn register(k: &Kernel) {
    k.register(
        Command::new(
            "basic",
            cmd_basic,
            "BASIC: write real programs (basic, or basic file.bas)",
            Topic::Programs,
        )
        .keep_alive(),
    );
    k.register(Command::new("run", cmd_run, "run a program file: run game.bas", Topic::Programs).keep_alive());
}

struct Built {
    machine: Machine,
    console: Rc<RefCell<dyn EbConsole>>,
    program: Option<Rc<RefCell<dyn Program>>>,
}

fn build(p: &Arc<Proc>, interactive: bool) -> Result<Built, String> {
    let (tx, rx) = async_channel::unbounded();
    let console: Rc<RefCell<dyn EbConsole>> = Rc::new(RefCell::new(KidConsole::new(p.clone(), tx.clone())));
    let sleep_p = p.clone();
    let sleep_fn: endbasic_std::exec::SleepFn = Box::new(move |d, pos| {
        let p = sleep_p.clone();
        async move {
            p.sleep(d.as_millis() as u64).map_err(|_| {
                endbasic_core::exec::Error::IoError(
                    pos,
                    std::io::Error::new(std::io::ErrorKind::Interrupted, "stopped"),
                )
            })
        }
        .boxed_local()
    });
    // Called by the interpreter between instructions: notice Ctrl-C (or a
    // kill) even when the program never reads the keyboard.
    let yield_p = p.clone();
    let yield_tx = tx.clone();
    let yield_fn: endbasic_core::exec::YieldNowFn = Box::new(move || {
        let stop = yield_p.killed() || yield_p.take_key_if(|k| *k == kiddos_kernel::Key::Ctrl('c')).is_some();
        if stop {
            let _ = yield_tx.try_send(endbasic_core::exec::Signal::Break);
        }
        Box::pin(async {})
    });
    let builder = MachineBuilder::default()
        .with_console(console.clone())
        .with_signals_chan((tx, rx))
        .with_yield_now_fn(yield_fn)
        .with_sleep_fn(sleep_fn);
    let (mut machine, program) = if interactive {
        let mut ib = builder.make_interactive();
        {
            let storage = ib.get_storage();
            let mut st = storage.borrow_mut();
            st.register_scheme("kiddos", Box::new(KidDriveFactory { proc: p.clone() }));
            let home = if p.is_root() {
                "/root".to_string()
            } else {
                KID_HOME.to_string()
            };
            st.mount("HOME", &format!("kiddos://{home}"))
                .map_err(|e| e.to_string())?;
            st.cd("HOME:").map_err(|e| e.to_string())?;
            let _ = st.unmount("MEMORY");
        }
        let program: Rc<RefCell<dyn Program>> = Rc::new(RefCell::new(endbasic_repl::editor::Editor::default()));
        let ib = ib.with_program(program.clone());
        (ib.build().map_err(|e| e.to_string())?, Some(program))
    } else {
        (builder.build().map_err(|e| e.to_string())?, None)
    };
    ext::add_all(&mut machine, p.clone(), console.clone());
    Ok(Built {
        machine,
        console,
        program,
    })
}

/// Run BASIC source (a file's contents) to completion. Returns the exit code.
pub fn run_source(p: &Proc, name: &str, src: &str) -> i32 {
    let arc = p.arc();
    let mut built = match build(&arc, false) {
        Ok(b) => b,
        Err(e) => {
            p.eprintln(&format!("basic: {e}"));
            return 1;
        }
    };
    // a shebang line is for the shell, not for BASIC
    let body = if src.starts_with("#!") {
        src.split_once('\n').map(|(_, rest)| rest).unwrap_or("")
    } else {
        src
    };
    let result = futures_lite::future::block_on(built.machine.exec(&mut body.as_bytes()));
    finish_gfx(p, matches!(result, Ok(StopReason::Eof) | Ok(StopReason::Exited(_))));
    p.set_color(kiddos_console::colors::DEFAULT_FG, kiddos_console::colors::DEFAULT_BG);
    p.cursor_show(true);
    if p.cursor_pos().0 != 0 {
        p.print("\n");
    }
    match result {
        Ok(StopReason::Eof) => 0,
        Ok(StopReason::Exited(code)) => code as i32,
        Ok(StopReason::Break) => {
            p.println("");
            130
        }
        Err(e) => {
            p.println("");
            p.eprintln(&format!("\x1b[1;31m{name}: {}\x1b[0m", ext::humanize(&e.to_string())));
            1
        }
    }
}

/// A program that ends in pixel mode leaves its picture up until a key is
/// pressed, so a kid's first `GFX_CIRCLE` does not vanish at once. After a
/// Break or an error the text comes straight back so the message is seen.
fn finish_gfx(p: &Proc, wait: bool) {
    if !p.gfx_on() {
        return;
    }
    if wait {
        let _ = p.readkey();
    }
    p.gfx_mode(false);
}

fn cmd_basic(p: &Proc, args: &[String]) -> CmdResult {
    if let Some(file) = args.first().filter(|a| !a.starts_with('-')) {
        return match p.fs().read_string(file) {
            Ok(src) => Ok(run_source(p, file, &src)),
            Err(e) => {
                p.complain(&e);
                Ok(1)
            }
        };
    }
    if !p.stdin_is_tty() {
        // `echo 'PRINT 1' | basic`
        let src = String::from_utf8_lossy(&p.read_stdin_all()?).to_string();
        return Ok(run_source(p, "basic", &src));
    }
    let arc = p.arc();
    let mut built = match build(&arc, true) {
        Ok(b) => b,
        Err(e) => {
            p.eprintln(&format!("basic: {e}"));
            return Ok(1);
        }
    };
    p.print(WELCOME);
    let _program = built.program.take().expect("interactive");
    let code = futures_lite::future::block_on(repl(p, &mut built.machine, built.console.clone()));
    p.set_color(kiddos_console::colors::DEFAULT_FG, kiddos_console::colors::DEFAULT_BG);
    p.cursor_show(true);
    if p.killed() {
        return Err(kiddos_kernel::Interrupted);
    }
    Ok(code)
}

/// The interactive loop. Like EndBASIC's own, plus: EXIT/QUIT/BYE leave,
/// END at the prompt does not, and errors get a hint.
#[allow(clippy::await_holding_refcell_ref)] // single-threaded; nothing else touches the console meanwhile
async fn repl(p: &Proc, machine: &mut Machine, console: Rc<RefCell<dyn EbConsole>>) -> i32 {
    let mut history: Vec<String> = Vec::new();
    loop {
        if p.killed() {
            return 130;
        }
        // our own printer understands colors; EndBASIC's strips escapes
        p.println("\x1b[1;33mReady\x1b[0m");
        let line = {
            let mut c = console.borrow_mut();
            endbasic_std::console::read_line(&mut *c, "", "", Some(&mut history)).await
        };
        machine.drain_signals();
        match line {
            Ok(line) => {
                let word = line.trim().to_ascii_uppercase();
                if matches!(word.as_str(), "EXIT" | "QUIT" | "BYE" | "SYSTEM") {
                    p.println(&p.t("bye", &[]));
                    return 0;
                }
                if word.is_empty() {
                    continue;
                }
                let result = machine.exec(&mut line.as_bytes()).await;
                finish_gfx(p, matches!(result, Ok(StopReason::Eof) | Ok(StopReason::Exited(_))));
                match result {
                    Ok(StopReason::Break) => p.println(&format!("\x1b[33m{}\x1b[0m", p.t("program-stopped", &[]))),
                    Ok(_) => {}
                    Err(e) => p.println(&format!("\x1b[1;31m{}\x1b[0m", ext::humanize(&e.to_string()))),
                }
                p.cursor_show(true);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                if p.killed() {
                    return 130;
                }
                p.println("^C");
            }
            Err(_) => return 0,
        }
    }
}

fn cmd_run(p: &Proc, args: &[String]) -> CmdResult {
    let Some(file) = args.first() else {
        p.println(&p.t("usage", &[("usage", "run <program.bas>   or   run <script>")]));
        return Ok(1);
    };
    if file.ends_with(".bas") || file.ends_with(".BAS") {
        return cmd_basic(p, args);
    }
    match p.run_and_wait(args.to_vec()) {
        Ok(code) => Ok(code),
        Err(e) => {
            p.eprintln(&format!("run: {e}"));
            Ok(127)
        }
    }
}
