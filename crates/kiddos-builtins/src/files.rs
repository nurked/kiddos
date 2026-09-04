//! Navigation and files: ls pwd cat less head tail mkdir rmdir rm cp mv
//! touch tree find du df chmod ln

use crate::util::{human_size, need_operand, short_date, tz, wants_help, Args};
use kiddos_kernel::{CmdResult, Command, Console, Kernel, Proc, Stat, Topic};
use kiddos_vfs::{basename, path::tildify, Kind};

pub fn register(k: &Kernel) {
    use Topic::Files as F;
    for c in [
        Command::new("ls", ls, "list what is in a folder", F),
        Command::new("pwd", pwd, "print where you are", F),
        Command::new("cat", cat, "show what is inside a file", F),
        Command::new("less", less, "read a long file one page at a time", F),
        Command::new("head", head, "show the first lines of a file", F),
        Command::new("tail", tail, "show the last lines of a file", F),
        Command::new("mkdir", mkdir, "make a new folder", F),
        Command::new("rmdir", rmdir, "remove an empty folder", F),
        Command::new("rm", rm, "remove a file (forever!)", F),
        Command::new("cp", cp, "copy a file", F),
        Command::new("mv", mv, "move or rename a file", F),
        Command::new("touch", touch, "make an empty file", F),
        Command::new("tree", tree, "draw folders as a tree", F),
        Command::new("find", find, "find files by name", F),
        Command::new("du", du, "how big is this folder?", F),
        Command::new("df", df, "how full is the drive?", F),
        Command::new("chmod", chmod, "change what a file allows (like +x to run it)", F),
        Command::new("ln", ln, "make a link that points to another file", F),
        Command::new("cd", |_, _| Ok(0), "go into a folder", F).in_shell(),
    ] {
        k.register(c);
    }
}

fn color_name(p: &Proc, st: &Stat, path: &str) -> String {
    let name = &st.name;
    if !p.stdout_is_tty() {
        return name.clone();
    }
    match st.kind {
        Kind::Dir => format!("\x1b[1;34m{name}\x1b[0m"),
        Kind::Symlink => {
            if p.fs().is_dir(path) {
                format!("\x1b[1;36m{name}\x1b[0m")
            } else {
                format!("\x1b[36m{name}\x1b[0m")
            }
        }
        Kind::File if st.mode & 0o111 != 0 => format!("\x1b[1;32m{name}\x1b[0m"),
        Kind::File => name.clone(),
    }
}

fn columns(p: &Proc, names: &[(String, usize)]) {
    // names: (colored text, visible width)
    let (cols, _) = p.size();
    let width = names.iter().map(|(_, w)| *w).max().unwrap_or(1) + 2;
    let per_row = ((cols as usize) / width).max(1);
    let rows = names.len().div_ceil(per_row);
    for r in 0..rows {
        let mut line = String::new();
        for c in 0..per_row {
            let i = c * rows + r;
            if let Some((text, w)) = names.get(i) {
                line.push_str(text);
                if c + 1 < per_row && i + rows < names.len() {
                    line.push_str(&" ".repeat(width - w));
                }
            }
        }
        p.println(line.trim_end());
    }
}

fn ls(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    let long = a.has("l");
    let all = a.has("a") || a.has("A");
    let dots = a.has("a");
    let human = a.has("h");
    let one = a.has("1") || !p.stdout_is_tty();
    let targets: Vec<String> = if a.positional.is_empty() {
        vec![".".into()]
    } else {
        a.positional.clone()
    };
    let mut status = 0;
    let many = targets.len() > 1;
    let tzo = tz(p);
    for (ti, t) in targets.iter().enumerate() {
        let st = match p.fs().stat(t) {
            Ok(s) => s,
            Err(e) => {
                p.complain(&e);
                status = 1;
                continue;
            }
        };
        let entries: Vec<(String, Stat)> = if st.is_dir() {
            match p.fs().readdir(t) {
                Ok(v) => {
                    let mut list: Vec<(String, Stat)> = Vec::new();
                    if dots {
                        let mut here = st.clone();
                        here.name = ".".into();
                        list.push((t.clone(), here));
                        if let Ok(mut up) = p.fs().stat(&format!("{t}/..")) {
                            up.name = "..".into();
                            list.push((format!("{t}/.."), up));
                        }
                    }
                    list.extend(
                        v.into_iter()
                            .filter(|e| all || !e.name.starts_with('.'))
                            .map(|e| (format!("{}/{}", t.trim_end_matches('/'), e.name), e)),
                    );
                    list
                }
                Err(e) => {
                    p.complain(&e);
                    status = 1;
                    continue;
                }
            }
        } else {
            let mut s = p.fs().lstat(t).unwrap_or(st);
            s.name = t.clone();
            vec![(t.clone(), s)]
        };
        if many {
            if ti > 0 {
                p.println("");
            }
            p.println(&format!("{t}:"));
        }
        if long {
            for (path, e) in &entries {
                let link = if e.is_symlink() {
                    format!(" -> {}", p.fs().readlink(path).unwrap_or_default())
                } else {
                    String::new()
                };
                let size = if e.is_dir() { e.size * 32 } else { e.size };
                let size = if human { human_size(size) } else { size.to_string() };
                p.println(&format!(
                    "{} {:<5} {:>7} {} {}{}",
                    e.mode_string(),
                    e.owner,
                    size,
                    short_date(e.mtime, tzo),
                    color_name(p, e, path),
                    link
                ));
            }
        } else if one {
            for (path, e) in &entries {
                p.println(&color_name(p, e, path));
            }
        } else {
            let names: Vec<(String, usize)> = entries
                .iter()
                .map(|(path, e)| (color_name(p, e, path), e.name.chars().count()))
                .collect();
            columns(p, &names);
        }
    }
    Ok(status)
}

fn pwd(p: &Proc, _args: &[String]) -> CmdResult {
    p.println(&p.cwd());
    Ok(0)
}

/// Read stdin or the named files, in order. Errors are reported; returns
/// the text and whether anything failed.
pub fn read_inputs(p: &Proc, files: &[String]) -> Result<(String, bool), kiddos_kernel::Interrupted> {
    let mut out = String::new();
    let mut failed = false;
    if files.is_empty() || files.iter().all(|f| f == "-") {
        let data = p.read_stdin_all()?;
        out.push_str(&String::from_utf8_lossy(&data));
        return Ok((out, false));
    }
    for f in files {
        if f == "-" {
            let data = p.read_stdin_all()?;
            out.push_str(&String::from_utf8_lossy(&data));
            continue;
        }
        match p.fs().read_string(f) {
            Ok(s) => out.push_str(&s),
            Err(e) => {
                p.complain(&e);
                failed = true;
            }
        }
    }
    Ok((out, failed))
}

fn cat(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.is_empty() && p.stdin_is_tty() {
        return Ok(need_operand(p));
    }
    let (text, failed) = read_inputs(p, &a.positional)?;
    if a.has("n") {
        for (i, l) in text.lines().enumerate() {
            p.println(&format!("{:>6}  {}", i + 1, l));
        }
    } else {
        p.print(&text);
        if !text.is_empty() && !text.ends_with('\n') {
            p.print("\n");
        }
    }
    Ok(if failed { 1 } else { 0 })
}

fn less(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.is_empty() && p.stdin_is_tty() {
        return Ok(need_operand(p));
    }
    let (text, failed) = read_inputs(p, &a.positional)?;
    kiddos_man::page(p, &text)?;
    Ok(if failed { 1 } else { 0 })
}

fn head_tail(p: &Proc, args: &[String], tail: bool) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &["n"]);
    let n = a.num("n").unwrap_or(10);
    if a.positional.is_empty() && p.stdin_is_tty() {
        return Ok(need_operand(p));
    }
    if !tail && a.positional.is_empty() {
        let mut shown = 0;
        while shown < n {
            match p.read_stdin_line()? {
                Some(l) => p.println(&l),
                None => break,
            }
            shown += 1;
        }
        return Ok(0);
    }
    let (text, failed) = read_inputs(p, &a.positional)?;
    let lines: Vec<&str> = text.lines().collect();
    let slice: &[&str] = if tail {
        &lines[lines.len().saturating_sub(n)..]
    } else {
        &lines[..n.min(lines.len())]
    };
    for l in slice {
        p.println(l);
    }
    Ok(if failed { 1 } else { 0 })
}

fn head(p: &Proc, args: &[String]) -> CmdResult {
    head_tail(p, args, false)
}

fn tail(p: &Proc, args: &[String]) -> CmdResult {
    head_tail(p, args, true)
}

fn mkdir(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.is_empty() {
        return Ok(need_operand(p));
    }
    let mut status = 0;
    for d in &a.positional {
        let r = if a.has("p") { p.fs().mkdir_p(d) } else { p.fs().mkdir(d) };
        if let Err(e) = r {
            p.complain(&e);
            status = 1;
        }
    }
    Ok(status)
}

fn rmdir(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.is_empty() {
        return Ok(need_operand(p));
    }
    let mut status = 0;
    for d in &a.positional {
        if let Err(e) = p.fs().rmdir(d) {
            p.complain(&e);
            status = 1;
        }
    }
    Ok(status)
}

fn rm(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.is_empty() {
        p.println(&p.t("rm-forever", &[]));
        return Ok(need_operand(p));
    }
    let recursive = a.has("r") || a.has("R");
    let mut status = 0;
    for t in &a.positional {
        let st = match p.fs().lstat(t) {
            Ok(s) => s,
            Err(e) => {
                if !a.has("f") {
                    p.complain(&e);
                    status = 1;
                }
                continue;
            }
        };
        let r = if st.is_dir() {
            if recursive {
                p.fs().remove_tree(t)
            } else {
                let shown = tildify(&p.fs().path(t), &p.home());
                p.eprintln(&format!("rm: {}", p.t("rm-dir-hint", &[("path", &shown)])));
                status = 1;
                continue;
            }
        } else {
            p.fs().unlink(t)
        };
        if let Err(e) = r {
            p.complain(&e);
            status = 1;
        }
    }
    Ok(status)
}

/// Destination for cp/mv: if `dest` is a dir, put `src`'s basename inside.
fn dest_path(p: &Proc, src: &str, dest: &str) -> String {
    if p.fs().is_dir(dest) {
        format!("{}/{}", dest.trim_end_matches('/'), basename(src))
    } else {
        dest.to_string()
    }
}

fn copy_tree(p: &Proc, src: &str, dest: &str) -> Result<(), kiddos_vfs::VfsError> {
    let st = p.fs().stat(src)?;
    if st.is_dir() {
        p.fs().mkdir(dest)?;
        for e in p.fs().readdir(src)? {
            copy_tree(p, &format!("{src}/{}", e.name), &format!("{dest}/{}", e.name))?;
        }
        Ok(())
    } else {
        let data = p.fs().read(src)?;
        p.fs().write(dest, &data)?;
        if st.mode & 0o111 != 0 {
            let _ = p.fs().chmod(dest, st.mode);
        }
        Ok(())
    }
}

fn cp(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.len() < 2 {
        p.println(&p.t("usage", &[("usage", "cp <from> <to>")]));
        return Ok(1);
    }
    let dest = a.positional.last().unwrap().clone();
    let srcs = &a.positional[..a.positional.len() - 1];
    if srcs.len() > 1 && !p.fs().is_dir(&dest) {
        p.eprintln(&format!("cp: {}", p.t("not-dir", &[("path", &dest)])));
        return Ok(1);
    }
    let mut status = 0;
    for s in srcs {
        let d = dest_path(p, s, &dest);
        let r = match p.fs().stat(s) {
            Ok(st) if st.is_dir() && !a.has("r") && !a.has("R") => {
                p.eprintln(&format!(
                    "cp: {} (use cp -r to copy a folder)",
                    p.t("is-dir", &[("path", s)])
                ));
                status = 1;
                continue;
            }
            Ok(_) => copy_tree(p, s, &d),
            Err(e) => Err(e),
        };
        if let Err(e) = r {
            p.complain(&e);
            status = 1;
        }
    }
    Ok(status)
}

fn mv(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.len() < 2 {
        p.println(&p.t("usage", &[("usage", "mv <from> <to>")]));
        return Ok(1);
    }
    let dest = a.positional.last().unwrap().clone();
    let srcs = &a.positional[..a.positional.len() - 1];
    if srcs.len() > 1 && !p.fs().is_dir(&dest) {
        p.eprintln(&format!("mv: {}", p.t("not-dir", &[("path", &dest)])));
        return Ok(1);
    }
    let mut status = 0;
    for s in srcs {
        let d = dest_path(p, s, &dest);
        if let Err(e) = p.fs().rename(s, &d) {
            p.complain(&e);
            status = 1;
        }
    }
    Ok(status)
}

fn touch(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.is_empty() {
        return Ok(need_operand(p));
    }
    let mut status = 0;
    for f in &a.positional {
        if let Err(e) = p.fs().touch(f) {
            p.complain(&e);
            status = 1;
        }
    }
    Ok(status)
}

fn tree(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    let root = a.positional.first().cloned().unwrap_or_else(|| ".".into());
    let all = a.has("a");
    let mut out = String::new();
    let mut dirs = 0usize;
    let mut files = 0usize;
    let abs = p.fs().path(&root);
    let st = match p.fs().stat(&abs) {
        Ok(s) => s,
        Err(e) => {
            p.complain(&e);
            return Ok(1);
        }
    };
    if !st.is_dir() {
        p.println(&root);
        return Ok(0);
    }
    out.push_str(&format!("\x1b[1;34m{}\x1b[0m\n", root));
    fn rec(p: &Proc, dir: &str, prefix: &str, all: bool, out: &mut String, dirs: &mut usize, files: &mut usize) {
        let Ok(entries) = p.fs().readdir(dir) else { return };
        let entries: Vec<Stat> = entries
            .into_iter()
            .filter(|e| all || !e.name.starts_with('.'))
            .collect();
        let n = entries.len();
        for (i, e) in entries.iter().enumerate() {
            let last = i + 1 == n;
            let branch = if last { "└── " } else { "├── " };
            let path = format!("{}/{}", dir.trim_end_matches('/'), e.name);
            let shown = match e.kind {
                Kind::Dir => format!("\x1b[1;34m{}\x1b[0m", e.name),
                Kind::Symlink => format!(
                    "\x1b[36m{}\x1b[0m -> {}",
                    e.name,
                    p.fs().readlink(&path).unwrap_or_default()
                ),
                Kind::File => e.name.clone(),
            };
            out.push_str(&format!("{prefix}{branch}{shown}\n"));
            if e.is_dir() {
                *dirs += 1;
                let next = format!("{prefix}{}", if last { "    " } else { "│   " });
                rec(p, &path, &next, all, out, dirs, files);
            } else {
                *files += 1;
            }
        }
    }
    rec(p, &abs, "", all, &mut out, &mut dirs, &mut files);
    out.push_str(&format!("\n{dirs} folders, {files} files\n"));
    kiddos_man::page(p, &out)?;
    Ok(0)
}

fn find(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    // find [dir] [-name pattern] [-type f|d]
    let mut dir = ".".to_string();
    let mut name: Option<String> = None;
    let mut kind: Option<char> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-name" | "-iname" => {
                i += 1;
                name = args.get(i).cloned();
            }
            "-type" => {
                i += 1;
                kind = args.get(i).and_then(|s| s.chars().next());
            }
            s if !s.starts_with('-') && i == 0 => dir = s.to_string(),
            s => {
                p.eprintln(&format!("find: I don't understand {s}. Try: find . -name '*.txt'"));
                return Ok(1);
            }
        }
        i += 1;
    }
    let base = p.fs().path(&dir);
    let mut lines = Vec::new();
    let r = p.fs().walk_tree(&base, &mut |path, st, _| {
        let ok_name = name
            .as_ref()
            .map(|n| kiddos_kernel::fs::glob_match(&n.to_lowercase(), &st.name.to_lowercase()))
            .unwrap_or(true);
        let ok_kind = match kind {
            Some('f') => st.is_file(),
            Some('d') => st.is_dir(),
            Some('l') => st.is_symlink(),
            _ => true,
        };
        if ok_name && ok_kind {
            // show relative to what the kid typed
            let shown = if dir == "." {
                path.strip_prefix(&format!("{}/", base.trim_end_matches('/')))
                    .map(|s| format!("./{s}"))
                    .unwrap_or_else(|| ".".into())
            } else if base == "/" {
                path.to_string()
            } else {
                path.replacen(&base, dir.trim_end_matches('/'), 1)
            };
            lines.push(shown);
        }
    });
    if let Err(e) = r {
        p.complain(&e);
        return Ok(1);
    }
    for l in lines {
        p.println(&l);
    }
    Ok(0)
}

fn du(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    let targets: Vec<String> = if a.positional.is_empty() {
        vec![".".into()]
    } else {
        a.positional.clone()
    };
    let mut status = 0;
    for t in targets {
        match p.fs().size_of(&t) {
            Ok(n) => p.println(&format!("{:>8}  {}", human_size(n), t)),
            Err(e) => {
                p.complain(&e);
                status = 1;
            }
        }
    }
    Ok(status)
}

const DRIVE_SIZE: u64 = 64 * 1024 * 1024;

fn df(p: &Proc, _args: &[String]) -> CmdResult {
    let used = p.fs().used_bytes();
    let pct = used * 100 / DRIVE_SIZE;
    p.println("Drive       Size   Used  Free  Use%");
    p.println(&format!(
        "/dev/kdd0  {:>5}  {:>5} {:>5}  {:>3}%",
        human_size(DRIVE_SIZE),
        human_size(used),
        human_size(DRIVE_SIZE.saturating_sub(used)),
        pct
    ));
    Ok(0)
}

fn parse_mode(spec: &str, current: u16) -> Option<u16> {
    if let Ok(n) = u16::from_str_radix(spec, 8) {
        return Some(n & 0o777);
    }
    // [ugoa]*[+-=][rwx]+
    let (who, rest) = match spec.find(['+', '-', '=']) {
        Some(i) => (&spec[..i], &spec[i..]),
        None => return None,
    };
    let op = rest.chars().next()?;
    let perms = &rest[1..];
    let mut bits = 0u16;
    for c in perms.chars() {
        bits |= match c {
            'r' => 4,
            'w' => 2,
            'x' => 1,
            _ => return None,
        };
    }
    let mut mask = 0u16;
    let who = if who.is_empty() { "a" } else { who };
    for c in who.chars() {
        mask |= match c {
            'u' => bits << 6,
            'g' => bits << 3,
            'o' => bits,
            'a' => (bits << 6) | (bits << 3) | bits,
            _ => return None,
        };
    }
    Some(match op {
        '+' => current | mask,
        '-' => current & !mask,
        _ => {
            let clear = match who {
                "u" => 0o700,
                "g" => 0o070,
                "o" => 0o007,
                _ => 0o777,
            };
            (current & !clear) | mask
        }
    })
}

fn chmod(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    if args.len() < 2 {
        p.println(&p.t("usage", &[("usage", "chmod +x <file>   or   chmod 755 <file>")]));
        return Ok(1);
    }
    let spec = &args[0];
    let mut status = 0;
    for f in &args[1..] {
        let st = match p.fs().stat(f) {
            Ok(s) => s,
            Err(e) => {
                p.complain(&e);
                status = 1;
                continue;
            }
        };
        let Some(mode) = parse_mode(spec, st.mode) else {
            p.eprintln(&format!(
                "chmod: I don't understand {spec}. Try +x, -w, or a number like 755."
            ));
            return Ok(1);
        };
        if let Err(e) = p.fs().chmod(f, mode) {
            p.complain(&e);
            status = 1;
        }
    }
    Ok(status)
}

fn ln(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if !a.has("s") || a.positional.len() != 2 {
        p.println(&p.t("usage", &[("usage", "ln -s <target> <link>")]));
        return Ok(1);
    }
    let link = dest_path(p, &a.positional[0], &a.positional[1]);
    if let Err(e) = p.fs().symlink(&a.positional[0], &link) {
        p.complain(&e);
        return Ok(1);
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::parse_mode;

    #[test]
    fn modes() {
        assert_eq!(parse_mode("+x", 0o644), Some(0o755));
        assert_eq!(parse_mode("u+x", 0o644), Some(0o744));
        assert_eq!(parse_mode("-w", 0o755), Some(0o555));
        assert_eq!(parse_mode("755", 0), Some(0o755));
        assert_eq!(parse_mode("o=", 0o777), Some(0o770));
        assert_eq!(parse_mode("zzz", 0), None);
    }
}
