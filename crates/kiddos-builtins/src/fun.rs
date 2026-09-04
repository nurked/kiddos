//! fortune cowsay figlet — every lesson ends with something funny or loud.

use crate::files::read_inputs;
use kiddos_kernel::{CmdResult, Command, Console, Kernel, Proc, Topic};

pub fn register(k: &Kernel) {
    use Topic::Text as T;
    for c in [
        Command::new("fortune", fortune, "a random saying", T),
        Command::new("cowsay", cowsay, "a cow says whatever you tell it", T),
        Command::new("figlet", figlet, "write BIG letters", T),
    ] {
        k.register(c);
    }
}

fn fortune(p: &Proc, _args: &[String]) -> CmdResult {
    let lang = p.lang().code();
    let text = p
        .fs()
        .read_string(&format!("/usr/share/fortunes/{lang}.txt"))
        .or_else(|_| p.fs().read_string("/usr/share/fortunes/en.txt"))
        .unwrap_or_else(|_| "The fortune cookie jar is empty.".into());
    let items: Vec<&str> = text.split("\n%\n").map(str::trim).filter(|s| !s.is_empty()).collect();
    if items.is_empty() {
        return Ok(1);
    }
    let seed = p.tick() ^ (p.pid as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let idx = (seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1) >> 33) as usize % items.len();
    p.println(items[idx]);
    Ok(0)
}

fn cowsay(p: &Proc, args: &[String]) -> CmdResult {
    let text = if args.is_empty() {
        if p.stdin_is_tty() {
            "Moo? Tell me what to say: cowsay hello".to_string()
        } else {
            read_inputs(p, &[])?.0
        }
    } else {
        args.join(" ")
    };
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in words {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > 38 {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    p.println(&format!(" {}", "_".repeat(w + 2)));
    let n = lines.len();
    for (i, l) in lines.iter().enumerate() {
        let (a, b) = if n == 1 {
            ('<', '>')
        } else if i == 0 {
            ('/', '\\')
        } else if i + 1 == n {
            ('\\', '/')
        } else {
            ('|', '|')
        };
        p.println(&format!("{a} {:<w$} {b}", l, w = w));
    }
    p.println(&format!(" {}", "-".repeat(w + 2)));
    p.println("        \\   ^__^");
    p.println("         \\  (oo)\\_______");
    p.println("            (__)\\       )\\/\\");
    p.println("                ||----w |");
    p.println("                ||     ||");
    Ok(0)
}

use kiddos_console::font::glyph;

fn figlet(p: &Proc, args: &[String]) -> CmdResult {
    let text = if args.is_empty() {
        if p.stdin_is_tty() {
            "KidDOS".to_string()
        } else {
            read_inputs(p, &[])?.0.lines().next().unwrap_or("").to_string()
        }
    } else {
        args.join(" ")
    };
    let (cols, _) = p.size();
    let per_line = (cols as usize / 8).max(1);
    let chars: Vec<char> = text.chars().collect();
    for chunk in chars.chunks(per_line) {
        for row in 0..8 {
            let mut line = String::new();
            for c in chunk {
                let bits = glyph(*c)[row];
                for x in 0..8 {
                    line.push(if bits & (1 << x) != 0 { '█' } else { ' ' });
                }
            }
            p.println(line.trim_end());
        }
    }
    Ok(0)
}
