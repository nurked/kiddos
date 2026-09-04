//! `ksh` — the kid shell.
//!
//! A POSIX-ish subset: words, quoting, `$VAR`, `~`, globs, `;`, `&&`, `||`,
//! pipes, redirects, `#` comments, scripts with shebangs. No `$(...)`, no
//! heredocs, no functions (v1). Errors are sentences, never codes.

pub mod editor;
pub mod exec;
pub mod expand;
pub mod lexer;
pub mod parser;

use kiddos_kernel::{CmdResult, Command, Console, Kernel, Proc, Topic};

/// Register `ksh` with the kernel.
pub fn register(kernel: &Kernel) {
    kernel.register(Command::new(
        "ksh",
        ksh,
        "the kid shell (runs your scripts)",
        Topic::Programs,
    ));
}

/// `ksh [-l] [-c line] [script [args...]]`
fn ksh(p: &Proc, args: &[String]) -> CmdResult {
    let mut login = false;
    let mut script: Option<String> = None;
    let mut inline: Option<String> = None;
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-l" if script.is_none() => login = true,
            "-c" if script.is_none() => {
                i += 1;
                inline = args.get(i).cloned();
            }
            a if script.is_none() && inline.is_none() => script = Some(a.to_string()),
            a => rest.push(a.to_string()),
        }
        i += 1;
    }
    let mut sh = exec::Shell::new(p, login);
    if let Some(line) = inline {
        sh.positional = rest;
        return sh.run_line(&line);
    }
    if let Some(path) = script {
        let src = match p.fs().read_string(&path) {
            Ok(s) => s,
            Err(e) => {
                p.eprint(&format!("ksh: {}\n", p.explain(&e)));
                return Ok(1);
            }
        };
        sh.script_name = Some(path);
        sh.positional = rest;
        return sh.run_script(&src);
    }
    sh.interactive()
}
