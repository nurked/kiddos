//! `cc hello.c`: compile C to a `.wasm` the kid can run.
//!
//! The compiler is a real clang with a wasm32 target, on the host, found by
//! the host layer (a `packs/c` folder next to the drive, or `KIDDOS_CC`).
//! The kernel never sees it: sources go out and one `.wasm` comes back
//! through a single `HostCaps` method. The program links against nothing
//! but `/usr/include/kiddos.h`, which is on the drive for the kid to read.

use kiddos_kernel::{CmdResult, Console, Proc};

pub const HEADER_PATH: &str = "/usr/include/kiddos.h";

pub fn cmd_cc(p: &Proc, args: &[String]) -> CmdResult {
    let verbose = args.iter().any(|a| a == "-v");
    let mut out: Option<String> = None;
    let mut sources: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                out = args.get(i).cloned();
            }
            "-v" => {}
            a if a.starts_with('-') => {
                p.println(&format!(
                    "cc: I don't know the option {a}. I only need file names, and -o for the output name."
                ));
                return Ok(1);
            }
            a => sources.push(a.to_string()),
        }
        i += 1;
    }
    if sources.is_empty() {
        p.println(&p.t(
            "usage",
            &[(
                "usage",
                "cc hello.c        (makes hello.wasm; run it with ./hello.wasm)",
            )],
        ));
        return Ok(1);
    }
    let host = p.kernel().host();
    if let Err(why) = host.c_compiler_available() {
        p.println(&format!("cc: {why}"));
        return Ok(1);
    }
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for s in &sources {
        match p.fs().read(s) {
            Ok(data) => files.push((kiddos_vfs::basename(s).to_string(), data)),
            Err(e) => {
                p.complain(&e);
                return Ok(1);
            }
        }
    }
    let header = p.fs().read(HEADER_PATH).unwrap_or_default();
    files.push(("kiddos.h".into(), header));
    let output = out.unwrap_or_else(|| {
        let first = &sources[0];
        let stem = first.strip_suffix(".c").unwrap_or(first);
        format!("{stem}.wasm")
    });
    match host.compile_c(&files) {
        Ok(wasm) => {
            if let Err(e) = p.fs().write(&output, &wasm) {
                p.complain(&e);
                return Ok(1);
            }
            let _ = p.fs().chmod(&output, 0o755);
            p.println(&format!(
                "\x1b[1;32mOK\x1b[0m {} ({} bytes). Run it: ./{}",
                output,
                wasm.len(),
                kiddos_vfs::basename(&output)
            ));
            Ok(0)
        }
        Err(raw) => {
            if verbose {
                p.print(&raw);
                if !raw.ends_with('\n') {
                    p.print("\n");
                }
            } else {
                for line in humanize(&raw) {
                    p.println(&line);
                }
                p.println("(cc -v shows the compiler's own words.)");
            }
            Ok(1)
        }
    }
}

/// Turn clang's diagnostics into sentences with the line number first.
pub fn humanize(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in raw.lines() {
        // file.c:3:5: error: expected ';' after expression
        let mut parts = line.splitn(4, ':');
        let (Some(file), Some(ln), Some(_col), Some(rest)) = (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if ln.trim().parse::<u32>().is_err() {
            continue;
        }
        let rest = rest.trim();
        let (kind, msg) = match rest.split_once(':') {
            Some((k, m)) => (k.trim(), m.trim()),
            None => continue,
        };
        if kind != "error" && kind != "warning" {
            continue;
        }
        let hint = if msg.contains("expected ';'") {
            " Every statement ends with a semicolon."
        } else if msg.starts_with("use of undeclared identifier") || msg.contains("undeclared") {
            " Did you spell it the same way everywhere, and say what it is (int x;) before using it?"
        } else if msg.contains("expected ')'") || msg.contains("expected '('") {
            " Count the brackets: every ( needs a )."
        } else if msg.contains("expected '}'") || msg.contains("expected '{'") {
            " Count the braces: every { needs a }."
        } else if msg.contains("implicit declaration of function") || msg.contains("call to undeclared function") {
            " Is that function in kiddos.h? Type: cat /usr/include/kiddos.h"
        } else if msg.contains("unterminated") || msg.contains("missing terminating") {
            " A quote mark opened and never closed."
        } else if msg.contains("undefined symbol") {
            " Something is used but never written. Check function names."
        } else {
            ""
        };
        let file = file.rsplit('/').next().unwrap_or(file);
        out.push(format!("\x1b[1;31m{file}, line {}:\x1b[0m {msg}.{hint}", ln.trim()));
    }
    if out.is_empty() {
        out.push(format!(
            "The compiler said no: {}",
            raw.lines().find(|l| !l.trim().is_empty()).unwrap_or("(no details)")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::humanize;

    #[test]
    fn explains_clang_errors() {
        let raw = "hello.c:3:18: error: expected ';' after expression\n    kd_print(\"hi\")\n                 ^\n                 ;\nhello.c:5:5: error: use of undeclared identifier 'x'\n2 errors generated.\n";
        let lines = humanize(raw);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("hello.c, line 3:") && lines[0].contains("semicolon"));
        assert!(lines[1].contains("line 5") && lines[1].contains("spell"));
        assert!(humanize("wasm-ld: error: cannot open x")[0].contains("The compiler said no"));
    }
}
