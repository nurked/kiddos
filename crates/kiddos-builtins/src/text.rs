//! Text tools: echo grep wc sort uniq rev tr cut seq yes

use crate::files::read_inputs;
use crate::util::{need_operand, wants_help, Args};
use kiddos_console::color;
use kiddos_kernel::{CmdResult, Command, Console, Kernel, Proc, Topic};

pub fn register(k: &Kernel) {
    use Topic::Text as T;
    for c in [
        Command::new("echo", echo, "say something back (or put it in a file with >)", T),
        Command::new("grep", grep, "find lines that contain a word", T),
        Command::new("wc", wc, "count lines, words and letters", T),
        Command::new("sort", sort, "put lines in order", T),
        Command::new("uniq", uniq, "drop repeated lines (sort first!)", T),
        Command::new("rev", rev, "flip every line backwards", T),
        Command::new("tr", tr, "swap letters, like a-z A-Z for SHOUTING", T),
        Command::new("cut", cut, "keep only part of each line", T),
        Command::new("seq", seq, "count from one number to another", T),
        Command::new("yes", yes, "say yes forever (press Ctrl-C to stop)", T),
    ] {
        k.register(c);
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('e') => out.push('\x1b'),
                Some('a') => out.push('\x07'),
                Some('\\') => out.push('\\'),
                Some(o) => {
                    out.push('\\');
                    out.push(o);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn echo(p: &Proc, args: &[String]) -> CmdResult {
    let mut rest = args;
    let mut newline = true;
    let mut escapes = false;
    let mut color_name: Option<&str> = None;
    loop {
        match rest.first().map(|s| s.as_str()) {
            Some("-n") => newline = false,
            Some("-e") => escapes = true,
            Some("-c") if rest.len() > 1 => {
                color_name = Some(rest[1].as_str());
                rest = &rest[1..];
            }
            _ => break,
        }
        rest = &rest[1..];
    }
    let mut text = rest.join(" ");
    if escapes {
        text = unescape(&text);
    }
    if let Some(cn) = color_name {
        match color::by_name(cn) {
            Some(idx) => {
                let code = if idx < 8 {
                    30 + ansi_of(idx)
                } else {
                    90 + ansi_of(idx - 8)
                };
                text = format!("\x1b[{code}m{text}\x1b[0m");
            }
            None => {
                p.eprintln(&format!(
                    "echo: I don't know the color {cn}. I know: {}",
                    color::NAMES.join(" ")
                ));
                return Ok(1);
            }
        }
    }
    p.print(&text);
    if newline {
        p.print("\n");
    }
    Ok(0)
}

/// CGA index → ANSI color number (0..7).
fn ansi_of(cga: u8) -> u8 {
    (cga & 2) | ((cga & 1) << 2) | ((cga & 4) >> 2)
}

fn grep(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.is_empty() {
        p.println(&p.t(
            "usage",
            &[("usage", "grep <word> [file...]   or   something | grep <word>")],
        ));
        return Ok(2);
    }
    let icase = a.has("i");
    let invert = a.has("v");
    let number = a.has("n");
    let count = a.has("c");
    let pat = if icase {
        a.positional[0].to_lowercase()
    } else {
        a.positional[0].clone()
    };
    let files = &a.positional[1..];
    if files.is_empty() && p.stdin_is_tty() {
        p.println(&p.t(
            "usage",
            &[("usage", "grep <word> <file>   or   something | grep <word>")],
        ));
        return Ok(2);
    }
    let anchored_start = pat.starts_with('^');
    let anchored_end = pat.ends_with('$') && pat.len() > 1;
    let core = pat.trim_start_matches('^').trim_end_matches('$');
    let matches = |line: &str| -> bool {
        let l = if icase { line.to_lowercase() } else { line.to_string() };
        let hit = match (anchored_start, anchored_end) {
            (true, true) => l == core,
            (true, false) => l.starts_with(core),
            (false, true) => l.ends_with(core),
            (false, false) => l.contains(core),
        };
        hit != invert
    };
    let many = files.len() > 1;
    let mut any = false;
    let mut status = 0;
    let inputs: Vec<(String, String)> = if files.is_empty() {
        let data = p.read_stdin_all()?;
        vec![("(stdin)".into(), String::from_utf8_lossy(&data).to_string())]
    } else {
        let mut v = Vec::new();
        for f in files {
            match p.fs().read_string(f) {
                Ok(s) => v.push((f.clone(), s)),
                Err(e) => {
                    p.complain(&e);
                    status = 2;
                }
            }
        }
        v
    };
    for (name, text) in inputs {
        let mut n = 0;
        for (i, line) in text.lines().enumerate() {
            if matches(line) {
                any = true;
                n += 1;
                if count {
                    continue;
                }
                let shown = if p.stdout_is_tty() && !invert && !core.is_empty() {
                    highlight(line, core, icase)
                } else {
                    line.to_string()
                };
                let mut out = String::new();
                if many {
                    out.push_str(&format!("\x1b[35m{name}\x1b[0m:"));
                }
                if number {
                    out.push_str(&format!("\x1b[32m{}\x1b[0m:", i + 1));
                }
                out.push_str(&shown);
                p.println(&out);
            }
        }
        if count {
            if many {
                p.println(&format!("{name}:{n}"));
            } else {
                p.println(&n.to_string());
            }
        }
    }
    Ok(if status != 0 {
        status
    } else if any {
        0
    } else {
        1
    })
}

fn highlight(line: &str, pat: &str, icase: bool) -> String {
    let hay = if icase { line.to_lowercase() } else { line.to_string() };
    let mut out = String::new();
    let mut last = 0;
    let mut start = 0;
    while let Some(i) = hay[start..].find(pat) {
        let i = start + i;
        // hay and line share byte offsets only if lowercasing kept lengths; be safe
        if hay.len() != line.len() {
            return line.to_string();
        }
        out.push_str(&line[last..i]);
        out.push_str(&format!("\x1b[1;31m{}\x1b[0m", &line[i..i + pat.len()]));
        last = i + pat.len();
        start = last;
        if pat.is_empty() {
            break;
        }
    }
    out.push_str(&line[last..]);
    out
}

fn wc(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.is_empty() && p.stdin_is_tty() {
        return Ok(need_operand(p));
    }
    let (text, failed) = read_inputs(p, &a.positional)?;
    let lines = text.lines().count();
    let words = text.split_whitespace().count();
    let chars = text.chars().count();
    let only = a.has("l") || a.has("w") || a.has("c") || a.has("m");
    let mut parts = Vec::new();
    if !only || a.has("l") {
        parts.push(format!("{lines:>6}"));
    }
    if !only || a.has("w") {
        parts.push(format!("{words:>6}"));
    }
    if !only || a.has("c") || a.has("m") {
        parts.push(format!("{chars:>6}"));
    }
    let name = if a.positional.len() == 1 {
        format!(" {}", a.positional[0])
    } else {
        String::new()
    };
    p.println(&format!("{}{}", parts.join(""), name));
    Ok(if failed { 1 } else { 0 })
}

fn sort(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.is_empty() && p.stdin_is_tty() {
        return Ok(need_operand(p));
    }
    let (text, failed) = read_inputs(p, &a.positional)?;
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    if a.has("n") {
        lines.sort_by(|x, y| {
            let nx: f64 = x.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let ny: f64 = y.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
            nx.partial_cmp(&ny)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| x.cmp(y))
        });
    } else if a.has("f") {
        lines.sort_by_key(|l| l.to_lowercase());
    } else {
        lines.sort();
    }
    if a.has("r") {
        lines.reverse();
    }
    if a.has("u") {
        lines.dedup();
    }
    for l in lines {
        p.println(&l);
    }
    Ok(if failed { 1 } else { 0 })
}

fn uniq(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    if a.positional.is_empty() && p.stdin_is_tty() {
        return Ok(need_operand(p));
    }
    let (text, failed) = read_inputs(p, &a.positional)?;
    let mut prev: Option<String> = None;
    let mut count = 0usize;
    let flush = |prev: &Option<String>, count: usize| {
        if let Some(l) = prev {
            if a.has("c") {
                p.println(&format!("{count:>7} {l}"));
            } else if !(a.has("d") && count < 2) {
                p.println(l);
            }
        }
    };
    for line in text.lines() {
        if prev.as_deref() == Some(line) {
            count += 1;
        } else {
            flush(&prev, count);
            prev = Some(line.to_string());
            count = 1;
        }
    }
    flush(&prev, count);
    Ok(if failed { 1 } else { 0 })
}

fn rev(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let (text, failed) = read_inputs(p, args)?;
    for l in text.lines() {
        p.println(&l.chars().rev().collect::<String>());
    }
    Ok(if failed { 1 } else { 0 })
}

fn expand_set(s: &str) -> Vec<char> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if i + 2 < chars.len() && chars[i + 1] == '-' && chars[i] <= chars[i + 2] {
            let (a, b) = (chars[i] as u32, chars[i + 2] as u32);
            for c in a..=b {
                if let Some(ch) = char::from_u32(c) {
                    out.push(ch);
                }
            }
            i += 3;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn tr(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &[]);
    let delete = a.has("d");
    if a.positional.is_empty() || (!delete && a.positional.len() < 2) {
        p.println(&p.t("usage", &[("usage", "something | tr a-z A-Z   or   tr -d <letters>")]));
        return Ok(1);
    }
    let from = expand_set(&a.positional[0]);
    let to = if delete {
        Vec::new()
    } else {
        expand_set(&a.positional[1])
    };
    let data = p.read_stdin_all()?;
    let text = String::from_utf8_lossy(&data);
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match from.iter().position(|f| *f == c) {
            Some(i) if delete => {
                let _ = i;
            }
            Some(i) => out.push(*to.get(i).or(to.last()).unwrap_or(&c)),
            None => out.push(c),
        }
    }
    p.print(&out);
    Ok(0)
}

fn cut(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let a = Args::parse(args, &["d", "f", "c"]);
    let delim = a.value("d").and_then(|d| d.chars().next()).unwrap_or('\t');
    let fields: Option<Vec<usize>> = a
        .value("f")
        .map(|f| f.split(',').filter_map(|s| s.parse().ok()).collect());
    let chars: Option<(usize, usize)> = a.value("c").map(|c| {
        let (x, y) = c.split_once('-').unwrap_or((c, c));
        (x.parse().unwrap_or(1), y.parse().unwrap_or(usize::MAX))
    });
    if fields.is_none() && chars.is_none() {
        p.println(&p.t("usage", &[("usage", "cut -d ' ' -f 2 <file>   or   cut -c 1-5 <file>")]));
        return Ok(1);
    }
    if a.positional.is_empty() && p.stdin_is_tty() {
        return Ok(need_operand(p));
    }
    let (text, failed) = read_inputs(p, &a.positional)?;
    for line in text.lines() {
        if let Some(fs) = &fields {
            let parts: Vec<&str> = line.split(delim).collect();
            let picked: Vec<&str> = fs
                .iter()
                .filter_map(|i| parts.get(i.wrapping_sub(1)).copied())
                .collect();
            p.println(&picked.join(&delim.to_string()));
        } else if let Some((from, to)) = chars {
            let s: String = line
                .chars()
                .skip(from.saturating_sub(1))
                .take(to.saturating_sub(from) + 1)
                .collect();
            p.println(&s);
        }
    }
    Ok(if failed { 1 } else { 0 })
}

fn seq(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let nums: Vec<i64> = args.iter().filter_map(|s| s.parse().ok()).collect();
    let (from, step, to) = match nums.as_slice() {
        [to] => (1, 1, *to),
        [from, to] => (*from, 1, *to),
        [from, step, to] => (*from, *step, *to),
        _ => {
            p.println(&p.t("usage", &[("usage", "seq 10   or   seq 1 10   or   seq 10 -2 1")]));
            return Ok(1);
        }
    };
    if step == 0 || to.abs() > 1_000_000 || from.abs() > 1_000_000 {
        return Ok(1);
    }
    let mut i = from;
    let mut count = 0;
    while (step > 0 && i <= to) || (step < 0 && i >= to) {
        p.println(&i.to_string());
        i += step;
        count += 1;
        if count % 200 == 0 {
            p.check()?;
        }
    }
    Ok(0)
}

fn yes(p: &Proc, args: &[String]) -> CmdResult {
    let word = if args.is_empty() {
        "y".to_string()
    } else {
        args.join(" ")
    };
    let mut n = 0u64;
    loop {
        p.println(&word);
        n += 1;
        if n % 100 == 0 {
            p.check()?;
            if p.stdout_is_tty() {
                p.sleep(1)?;
            }
        }
        if p.interrupted() {
            return Err(kiddos_kernel::Interrupted);
        }
    }
}
