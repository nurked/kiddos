//! `goc hello.go`: compile Go to a `.wasm` with TinyGo, on the host, the
//! same way `cc` does. The program imports the `kiddos` package that lives
//! on the drive at `/usr/share/go/kiddos`.

use kiddos_kernel::{CmdResult, Console, Proc};

pub const PKG_DIR: &str = "/usr/share/go/kiddos";

pub fn cmd_goc(p: &Proc, args: &[String]) -> CmdResult {
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
                    "goc: I don't know the option {a}. I only need file names, and -o for the output name."
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
                "goc hello.go        (makes hello.wasm; run it with ./hello.wasm)",
            )],
        ));
        return Ok(1);
    }
    let host = p.kernel().host();
    if let Err(why) = host.go_compiler_available() {
        p.println(&format!("goc: {why}"));
        return Ok(1);
    }
    let mut files = Vec::new();
    for s in &sources {
        match p.fs().read(s) {
            Ok(data) => files.push((kiddos_vfs::basename(s).to_string(), data)),
            Err(e) => {
                p.complain(&e);
                return Ok(1);
            }
        }
    }
    let mut pkg = Vec::new();
    for e in p.fs().readdir(PKG_DIR).unwrap_or_default() {
        if let Ok(data) = p.fs().read(&format!("{PKG_DIR}/{}", e.name)) {
            pkg.push((e.name.clone(), data));
        }
    }
    let output = out.unwrap_or_else(|| {
        let first = &sources[0];
        let stem = first.strip_suffix(".go").unwrap_or(first);
        format!("{stem}.wasm")
    });
    match host.compile_go(&files, &pkg) {
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
                p.println("(goc -v shows the compiler's own words.)");
            }
            Ok(1)
        }
    }
}

/// Go's diagnostics look like `./main.go:5:2: undefined: x`.
pub fn humanize(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let l = line.trim_start_matches("./");
        let mut parts = l.splitn(4, ':');
        let (Some(file), Some(ln), Some(_col), Some(msg)) = (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if !file.ends_with(".go") || ln.trim().parse::<u32>().is_err() {
            continue;
        }
        let msg = msg.trim();
        let hint = if msg.starts_with("undefined:") {
            " Did you spell it the same way everywhere? Names are case-sensitive: kiddos.Print, not kiddos.print."
        } else if msg.contains("declared and not used") {
            " Go refuses to run if a variable is never used. Use it or remove it."
        } else if msg.contains("imported and not used") {
            " You import something you never use. Remove the import or use it."
        } else if msg.contains("expected") || msg.contains("syntax error") {
            " Something is missing or in the wrong place on that line: a brace, a bracket or a comma."
        } else if msg.contains("cannot use") || msg.contains("mismatched types") {
            " Two things of different types met. Go wants them the same: int and int, string and string."
        } else if msg.contains("missing return") {
            " A function that promises a result must return one on every path."
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
    fn explains_go_errors() {
        let raw = "./main.go:6:2: undefined: kiddos.print\n./main.go:8:6: declared and not used: x\n";
        let l = humanize(raw);
        assert_eq!(l.len(), 2);
        assert!(l[0].contains("main.go, line 6") && l[0].contains("case-sensitive"));
        assert!(l[1].contains("never used"));
    }
}
