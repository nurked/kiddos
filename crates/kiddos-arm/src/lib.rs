//! A small AArch64 for kids: assemble it, run it, step through it.
//!
//! `as hello.s` writes `hello`, a `\0arm` image the kernel runs through
//! the `arm` command (the way `\0asm` goes to `wasm`). `dis` reads one
//! back, `debug` steps through one on a full screen, and `bug-hunt` is
//! the game that teaches `debug`.

pub mod asm;
pub mod bughunt;
pub mod debug;
pub mod dis;
pub mod image;
pub mod insn;
pub mod sys;
pub mod vm;

use image::Image;
use kiddos_kernel::{CmdResult, Command, Console, Kernel, Proc, Topic};
use vm::{Fault, Step, Vm};

/// A cartridge may ask for more memory through `memory_mb` in its
/// manifest, which `play` passes down as this variable (shared with wasm).
pub const MEMORY_ENV: &str = "KIDDOS_MEMORY_MB";

pub fn register(k: &Kernel) {
    k.register(Command::new(
        "as",
        cmd_as,
        "assemble an ARM program: as hello.s",
        Topic::Programs,
    ));
    k.register(
        Command::new(
            "arm",
            cmd_arm,
            "run an assembled program (or just ./prog)",
            Topic::Programs,
        )
        .keep_alive(),
    );
    k.register(Command::new(
        "dis",
        cmd_dis,
        "show a program's instructions: dis hello",
        Topic::Programs,
    ));
    k.register(
        Command::new(
            "debug",
            debug::cmd_debug,
            "step through a program one instruction at a time",
            Topic::Programs,
        )
        .keep_alive(),
    );
    k.register(
        Command::new(
            "bug-hunt",
            bughunt::cmd_bug_hunt,
            "eight tiny programs, one bug each; find them with debug",
            Topic::Hidden,
        )
        .keep_alive(),
    );
}

fn memory_for(p: &Proc) -> usize {
    p.env_get(MEMORY_ENV)
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|mb| (mb * 1024 * 1024).clamp(vm::DEFAULT_MEMORY, vm::MAX_MEMORY))
        .unwrap_or(vm::DEFAULT_MEMORY)
}

/// Load an image (a `.s` source is assembled on the spot, an assembled
/// file is read back) for `arm`, `dis` and `debug`.
pub fn load_program(p: &Proc, path: &str) -> Result<Image, String> {
    let bytes = p.fs().read(path).map_err(|e| p.explain(&e))?;
    if Image::is_image(&bytes) {
        return Image::from_bytes(&bytes);
    }
    if path.ends_with(".s") || path.ends_with(".S") || path.ends_with(".asm") {
        let src = String::from_utf8_lossy(&bytes);
        return asm::assemble(&src).map_err(|e| e.to_string());
    }
    Err(format!(
        "{path} is not an assembled program. Make one with: as {}.s",
        kiddos_vfs::basename(path).trim_end_matches(".s")
    ))
}

/// A machine with `img` loaded and the process's memory cap.
pub fn machine_for(p: &Proc, img: &Image) -> Vm {
    let mut vm = Vm::new(memory_for(p));
    vm.load(&img.text, &img.data, img.bss as usize, img.entry);
    vm
}

/// Run an image to the end. Faults are explained on the screen.
pub fn run_image(p: &Proc, name: &str, img: &Image) -> i32 {
    let mut vm = machine_for(p, img);
    let mut sys = sys::ProcSys::new(p, Box::new(sys::Direct));
    p.handle_ctrl_c(true);
    let result = run_until_stop(p, &mut vm, &mut sys);
    p.handle_ctrl_c(false);
    match result {
        Ok(code) => code,
        Err(Fault::Interrupted) => 130,
        Err(f) => {
            p.set_color(kiddos_console::colors::DEFAULT_FG, kiddos_console::colors::DEFAULT_BG);
            if p.cursor_pos().0 != 0 {
                p.print("\n");
            }
            let where_ = match img.line_of(vm.pc) {
                Some(l) => format!(" (line {l})"),
                None => String::new(),
            };
            p.eprintln(&format!(
                "\x1b[1;31m{name}: {}{where_}\x1b[0m",
                f.explain(&|a| img.name_of(a))
            ));
            p.eprintln(&format!("Watch it happen: debug {name}"));
            1
        }
    }
}

/// Run until exit or fault, checking for Ctrl-C between bursts.
fn run_until_stop(p: &Proc, vm: &mut Vm, sys: &mut dyn vm::Sys) -> Result<i32, Fault> {
    loop {
        for _ in 0..4096 {
            match vm.step(sys)? {
                Step::Ran => {}
                Step::Exit(code) => return Ok(code),
                Step::Brk(n) => {
                    return Err(Fault::Sys(format!(
                        "The program hit a breakpoint (brk #{n}). Run it in debug to stop there and look around."
                    )))
                }
            }
        }
        if p.killed() || p.take_key_if(|k| *k == kiddos_kernel::Key::Ctrl('c')).is_some() {
            return Err(Fault::Interrupted);
        }
        std::thread::yield_now();
    }
}

/// `arm <file>`: what the kernel runs for `\0arm` files.
fn cmd_arm(p: &Proc, args: &[String]) -> CmdResult {
    let Some(file) = args.first() else {
        p.println(&p.t("usage", &[("usage", "arm <program>   (or just ./program)")]));
        return Ok(1);
    };
    match load_program(p, file) {
        Ok(img) => Ok(run_image(p, kiddos_vfs::basename(file), &img)),
        Err(e) => {
            p.eprintln(&format!("{}: {e}", kiddos_vfs::basename(file)));
            Ok(1)
        }
    }
}

/// `as hello.s [-o hello]`
fn cmd_as(p: &Proc, args: &[String]) -> CmdResult {
    let mut out: Option<String> = None;
    let mut src: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                out = args.get(i).cloned();
            }
            a if a.starts_with('-') => {
                p.println(&format!(
                    "as: I don't know the option {a}. I need a .s file, and -o for the output name."
                ));
                return Ok(1);
            }
            a => {
                if src.is_some() {
                    p.println("as: one program at a time, please.");
                    return Ok(1);
                }
                src = Some(a.to_string());
            }
        }
        i += 1;
    }
    let Some(src) = src else {
        p.println(&p.t(
            "usage",
            &[("usage", "as hello.s        (makes hello; run it with ./hello)")],
        ));
        return Ok(1);
    };
    let text = match p.fs().read_string(&src) {
        Ok(t) => t,
        Err(e) => {
            p.complain(&e);
            return Ok(1);
        }
    };
    let output = out.unwrap_or_else(|| {
        let stem = src
            .strip_suffix(".s")
            .or_else(|| src.strip_suffix(".S"))
            .unwrap_or(&src);
        if stem == src {
            format!("{src}.out")
        } else {
            stem.to_string()
        }
    });
    match asm::assemble(&text) {
        Ok(img) => {
            let bytes = img.to_bytes();
            if let Err(e) = p.fs().write(&output, &bytes) {
                p.complain(&e);
                return Ok(1);
            }
            let _ = p.fs().chmod(&output, 0o755);
            p.println(&format!(
                "\x1b[1;32mOK\x1b[0m {} ({} instructions, {} bytes of data). Run it: ./{}",
                output,
                img.lines.len(),
                img.data.len() + img.bss as usize,
                kiddos_vfs::basename(&output)
            ));
            Ok(0)
        }
        Err(e) => {
            p.eprintln(&format!("\x1b[1;31m{}: {e}\x1b[0m", kiddos_vfs::basename(&src)));
            if e.line > 0 {
                if let Some(l) = text.lines().nth(e.line - 1) {
                    p.eprintln(&format!("  {:>4} | {}", e.line, l.trim_end()));
                }
            }
            Ok(1)
        }
    }
}

/// `dis prog`: the instructions as text, with labels and source lines.
fn cmd_dis(p: &Proc, args: &[String]) -> CmdResult {
    let Some(file) = args.first() else {
        p.println(&p.t(
            "usage",
            &[("usage", "dis <program>     (an assembled program, or a .s file)")],
        ));
        return Ok(1);
    };
    let img = match load_program(p, file) {
        Ok(i) => i,
        Err(e) => {
            p.eprintln(&format!("dis: {e}"));
            return Ok(1);
        }
    };
    p.print(&listing(&img));
    Ok(0)
}

/// The text `dis` prints.
pub fn listing(img: &Image) -> String {
    let mut out = String::new();
    let src: Vec<&str> = img.source.lines().collect();
    let name_of = |a: u64| match img.symbols.iter().find(|(_, x)| *x == a) {
        Some((n, _)) => n.clone(),
        None => format!("0x{a:x}"),
    };
    let text_end = vm::TEXT_BASE + img.text.len() as u64;
    let mut addr = vm::TEXT_BASE;
    let mut last_line = 0u32;
    while addr + 4 <= text_end {
        for (name, a) in &img.symbols {
            if *a == addr {
                out.push_str(&format!("\x1b[1;33m{name}:\x1b[0m\n"));
            }
        }
        let o = (addr - vm::TEXT_BASE) as usize;
        let w = u32::from_le_bytes([img.text[o], img.text[o + 1], img.text[o + 2], img.text[o + 3]]);
        let line = img.line_of(addr);
        let text = match line {
            Some(_) => dis::format(w, addr, &name_of),
            None => format!(".quad 0x{:x}", {
                // literal pool entries are 8 bytes
                let hi = if o + 8 <= img.text.len() {
                    u32::from_le_bytes([img.text[o + 4], img.text[o + 5], img.text[o + 6], img.text[o + 7]])
                } else {
                    0
                };
                ((hi as u64) << 32) | w as u64
            }),
        };
        let comment = match line {
            Some(l) if l != last_line => {
                last_line = l;
                src.get(l as usize - 1)
                    .map(|s| format!("\x1b[36m// {l}: {}\x1b[0m", s.trim()))
                    .unwrap_or_default()
            }
            _ => String::new(),
        };
        out.push_str(&format!("0x{addr:x}  {w:08x}  {text:<32} {comment}\n"));
        if line.is_none() {
            addr += 8;
        } else {
            addr += 4;
        }
    }
    if !img.data.is_empty() || img.bss > 0 {
        let base = (text_end + 15) & !15;
        out.push_str(&format!(
            "\x1b[1;33m.data\x1b[0m (0x{base:x}, {} bytes)\n",
            img.data.len()
        ));
        for (i, chunk) in img.data.chunks(16).enumerate() {
            let a = base + 16 * i as u64;
            for (name, x) in &img.symbols {
                if *x >= a && *x < a + 16 && *x >= base && (*x as usize - base as usize) < img.data.len() {
                    out.push_str(&format!("\x1b[1;33m{name}:\x1b[0m (0x{x:x})\n"));
                }
            }
            out.push_str(&format!("0x{a:x}  {}\n", hex_row(chunk)));
        }
        if img.bss > 0 {
            out.push_str(&format!(
                "\x1b[1;33m.bss\x1b[0m (0x{:x}, {} bytes of zeros)\n",
                base + img.data.len() as u64,
                img.bss
            ));
        }
    }
    out
}

/// `48 65 6c 6c 6f  |Hello|`, the way hexdump shows a row.
pub fn hex_row(chunk: &[u8]) -> String {
    let mut hex = String::new();
    for (i, b) in chunk.iter().enumerate() {
        if i == 8 {
            hex.push(' ');
        }
        hex.push_str(&format!("{b:02x} "));
    }
    let ascii: String = chunk
        .iter()
        .map(|b| if (0x20..0x7f).contains(b) { *b as char } else { '.' })
        .collect();
    format!("{hex:<49} |{ascii}|")
}
