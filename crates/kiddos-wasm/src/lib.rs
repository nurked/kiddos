//! Compiled programs run here. Any language that targets wasm32 becomes a
//! KidDOS language: the module imports the console API from a module named
//! `kiddos` (see `/usr/include/kiddos.h`) and exports `main`. A program
//! built with a real libc may also use the small WASI subset in `wasi.rs`,
//! which maps stdio and files onto the machine. Either way a program cannot
//! see the host, only the screen, the keys, the clock and the virtual
//! drive, exactly like a BASIC program.
//!
//! Safety: memory is capped, execution is epoch-interrupted every few
//! milliseconds so Ctrl-C (or `kill`) stops any loop, and traps become one
//! sentence on the screen.

pub mod cc;
pub mod goc;
mod host;
mod wasi;

use host::{Exit, State};
use kiddos_kernel::{CmdResult, Command, Console, Kernel, Proc, Topic};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const MEMORY_LIMIT: usize = 16 * 1024 * 1024;
/// A cartridge may ask for more through `memory_mb` in its manifest, which
/// `play` passes down as this environment variable. Capped at 256 MB.
pub const MEMORY_ENV: &str = "KIDDOS_MEMORY_MB";
pub const MEMORY_MAX_MB: u64 = 256;
pub const EPOCH_MS: u64 = 10;
/// The import module name programs use: `(import "kiddos" "print" ...)`.
pub const MODULE: &str = "kiddos";

pub fn register(k: &Kernel) {
    k.register(Command::new("wasm", cmd_wasm, "run a compiled program (.wasm)", Topic::Programs).keep_alive());
    k.register(Command::new(
        "cc",
        cc::cmd_cc,
        "compile a C program: cc hello.c",
        Topic::Programs,
    ));
    k.register(Command::new(
        "goc",
        goc::cmd_goc,
        "compile a Go program: goc hello.go",
        Topic::Programs,
    ));
}

fn engine() -> anyhow::Result<wasmtime::Engine> {
    let mut cfg = wasmtime::Config::new();
    cfg.epoch_interruption(true);
    wasmtime::Engine::new(&cfg)
}

/// Run a module's `main` (or `_start`) for process `p`. Returns the exit
/// code. Errors are explained on the screen in one sentence.
pub fn run_module(p: &Proc, name: &str, bytes: &[u8]) -> i32 {
    match run_inner(p, bytes) {
        Ok(code) => code,
        Err(e) => {
            if p.killed() {
                return 130;
            }
            p.set_color(kiddos_console::colors::DEFAULT_FG, kiddos_console::colors::DEFAULT_BG);
            if p.cursor_pos().0 != 0 {
                p.print("\n");
            }
            p.eprintln(&format!("\x1b[1;31m{name}: {}\x1b[0m", explain(&e)));
            1
        }
    }
}

fn explain(e: &anyhow::Error) -> String {
    if let Some(trap) = e.downcast_ref::<wasmtime::Trap>() {
        return match trap {
            wasmtime::Trap::Interrupt => "Stopped.".into(),
            wasmtime::Trap::MemoryOutOfBounds | wasmtime::Trap::TableOutOfBounds => {
                "The program reached outside its memory. (In C that is usually an array index or a pointer that went too far.)".into()
            }
            wasmtime::Trap::IntegerDivisionByZero => "The program divided by zero.".into(),
            wasmtime::Trap::StackOverflow => "The program called itself too many times (stack overflow).".into(),
            wasmtime::Trap::UnreachableCodeReached => "The program hit an 'unreachable' point and gave up (an abort or an assert).".into(),
            other => format!("The program crashed: {other}"),
        };
    }
    let text = e.to_string();
    if text.contains("unknown import") {
        return format!(
            "The program asks for something this machine does not have: {}",
            text.lines().next().unwrap_or("")
        );
    }
    if text.contains("memory") && text.contains("limit") {
        return "The program wanted more memory than a program may have here.".into();
    }
    text.lines().next().unwrap_or("The program could not run.").to_string()
}

fn run_inner(p: &Proc, bytes: &[u8]) -> anyhow::Result<i32> {
    let engine = engine()?;
    let module = wasmtime::Module::new(&engine, bytes)?;
    let mut linker = wasmtime::Linker::new(&engine);
    host::link(&mut linker)?;
    wasi::link(&mut linker)?;
    let mut store = wasmtime::Store::new(&engine, State::new(p.arc(), memory_limit(p)));
    store.limiter(|s| &mut s.limits);
    // Ctrl-C and kill are noticed between epochs
    store.set_epoch_deadline(1);
    store.epoch_deadline_callback(|ctx| {
        let s = ctx.data();
        let stop = s.proc.killed() || s.proc.take_key_if(|k| *k == kiddos_kernel::Key::Ctrl('c')).is_some();
        if stop {
            Err(wasmtime::Trap::Interrupt.into())
        } else {
            Ok(wasmtime::UpdateDeadline::Continue(1))
        }
    });
    p.handle_ctrl_c(true);
    let done = Arc::new(AtomicBool::new(false));
    let ticker = {
        let engine = engine.clone();
        let done = done.clone();
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(EPOCH_MS));
                engine.increment_epoch();
            }
        })
    };
    let result = (|| -> anyhow::Result<i32> {
        let instance = linker.instantiate(&mut store, &module)?;
        // reactor-style modules (TinyGo) set up their runtime in _initialize
        if let Ok(init) = instance.get_typed_func::<(), ()>(&mut store, "_initialize") {
            init.call(&mut store, ())?;
        }
        // clang's wasm target calls a no-argument main `__main_void`
        for name in ["main", "__main_void"] {
            if let Ok(main) = instance.get_typed_func::<(), i32>(&mut store, name) {
                return main.call(&mut store, ());
            }
        }
        // goc adds an exported `kiddos_main` (no result) that calls Go's main
        if let Ok(main) = instance.get_typed_func::<(), ()>(&mut store, "kiddos_main") {
            main.call(&mut store, ())?;
            return Ok(0);
        }
        if let Ok(start) = instance.get_typed_func::<(), ()>(&mut store, "_start") {
            start.call(&mut store, ())?;
            return Ok(0);
        }
        anyhow::bail!("The program has no main. A KidDOS program exports a function called main.")
    })();
    done.store(true, Ordering::Relaxed);
    let _ = ticker.join();
    p.handle_ctrl_c(false);
    {
        let proc = p.arc();
        store.data_mut().files.flush_all(&proc);
    }
    p.cursor_show(true);
    match result {
        Ok(code) => Ok(code),
        Err(e) => match e.downcast_ref::<Exit>() {
            Some(Exit(code)) => Ok(*code),
            None => Err(e),
        },
    }
}

/// The memory cap for this process: the default, or what a cartridge asked
/// for in its manifest.
fn memory_limit(p: &Proc) -> usize {
    p.env_get(MEMORY_ENV)
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|mb| mb.clamp(1, MEMORY_MAX_MB) as usize * 1024 * 1024)
        .unwrap_or(MEMORY_LIMIT)
}

/// `wasm <file> [args]`: the interpreter the kernel picks for `\0asm` files.
fn cmd_wasm(p: &Proc, args: &[String]) -> CmdResult {
    let Some(file) = args.first() else {
        p.println(&p.t("usage", &[("usage", "wasm <program.wasm>   (or just ./program)")]));
        return Ok(1);
    };
    match p.fs().read(file) {
        Ok(bytes) => Ok(run_module(p, kiddos_vfs::basename(file), &bytes)),
        Err(e) => {
            p.complain(&e);
            Ok(1)
        }
    }
}

/// Imports and exports of a module, one per line (for diagnostics).
pub fn describe(bytes: &[u8]) -> anyhow::Result<String> {
    let engine = engine()?;
    let module = wasmtime::Module::new(&engine, bytes)?;
    let mut out = String::new();
    for i in module.imports() {
        out.push_str(&format!("import {}.{}\n", i.module(), i.name()));
    }
    for e in module.exports() {
        out.push_str(&format!("export {}\n", e.name()));
    }
    Ok(out)
}
