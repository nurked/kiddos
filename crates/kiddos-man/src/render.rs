//! Markdown subset → console text with ANSI colors.
//!
//! Supported: `# Title`, `## Section`, paragraphs (wrapped), `> quote`,
//! `- item` / `* item`, fenced code blocks, inline `code`, `**bold**`.

const TITLE: &str = "\x1b[1;36m";
const SECTION: &str = "\x1b[1;33m";
const CODE: &str = "\x1b[92m";
const QUOTE: &str = "\x1b[37m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Visible width of a string containing ANSI escapes.
pub fn visible_len(s: &str) -> usize {
    let mut n = 0;
    let mut in_esc = false;
    for c in s.chars() {
        if in_esc {
            if c.is_ascii_alphabetic() {
                in_esc = false;
            }
        } else if c == '\x1b' {
            in_esc = true;
        } else {
            n += 1;
        }
    }
    n
}

/// Inline markup: `code` and **bold**.
fn inline(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    let mut in_code = false;
    let mut in_bold = false;
    while let Some(c) = chars.next() {
        if c == '`' {
            in_code = !in_code;
            out.push_str(if in_code { CODE } else { RESET });
        } else if c == '*' && chars.peek() == Some(&'*') && !in_code {
            chars.next();
            in_bold = !in_bold;
            out.push_str(if in_bold { BOLD } else { RESET });
        } else {
            out.push(c);
        }
    }
    if in_code || in_bold {
        out.push_str(RESET);
    }
    out
}

/// Word-wrap `text` (may contain escapes) to `width`, with `indent` spaces on
/// every line and `first_indent` on the first.
fn wrap(text: &str, width: usize, first_indent: &str, indent: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::from(first_indent);
    let mut line_len = first_indent.len();
    let mut first_word = true;
    for word in text.split_whitespace() {
        let wl = visible_len(word);
        if !first_word && line_len + 1 + wl > width {
            lines.push(std::mem::replace(&mut line, indent.to_string()));
            line_len = indent.len();
            first_word = true;
        }
        if !first_word {
            line.push(' ');
            line_len += 1;
        }
        line.push_str(word);
        line_len += wl;
        first_word = false;
    }
    lines.push(line);
    lines
}

pub fn render(md: &str, width: usize) -> String {
    let width = width.max(20);
    let mut out: Vec<String> = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut in_code = false;

    fn flush_para(para: &mut Vec<String>, out: &mut Vec<String>, width: usize) {
        if !para.is_empty() {
            let text = inline(&para.join(" "));
            out.extend(wrap(&text, width, "  ", "  "));
            para.clear();
        }
    }

    for raw in md.lines() {
        let line = raw.trim_end();
        if line.trim_start().starts_with("```") {
            flush_para(&mut para, &mut out, width);
            in_code = !in_code;
            if !in_code {
                out.push(String::new());
            }
            continue;
        }
        if in_code {
            out.push(format!("    {CODE}{line}{RESET}"));
            continue;
        }
        let t = line.trim();
        if t.is_empty() {
            flush_para(&mut para, &mut out, width);
            if out.last().map(|l| !l.is_empty()).unwrap_or(false) {
                out.push(String::new());
            }
        } else if let Some(h) = t.strip_prefix("# ") {
            flush_para(&mut para, &mut out, width);
            out.push(format!("{TITLE}{}{RESET}", h.trim().to_uppercase()));
        } else if let Some(h) = t.strip_prefix("## ") {
            flush_para(&mut para, &mut out, width);
            out.push(format!("{SECTION}{}{RESET}", h.trim().to_uppercase()));
        } else if let Some(h) = t.strip_prefix("### ") {
            flush_para(&mut para, &mut out, width);
            out.push(format!("  {BOLD}{}{RESET}", h.trim()));
        } else if let Some(q) = t.strip_prefix("> ") {
            flush_para(&mut para, &mut out, width);
            out.extend(wrap(&format!("{QUOTE}{}{RESET}", inline(q)), width, "  ", "  "));
        } else if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush_para(&mut para, &mut out, width);
            out.extend(wrap(&inline(item), width, "   * ", "     "));
        } else {
            para.push(t.to_string());
        }
    }
    flush_para(&mut para, &mut out, width);
    while out.last().map(|l| l.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_sections_and_wraps() {
        let md = "# ls\n\n> list what is in a folder\n\n## TRY THIS\n\n- `ls` shows files\n- `ls -l` shows more\n\nA long paragraph that should wrap because it is much longer than the width we give it here.\n\n```\nls -la\n```\n";
        let r = render(md, 40);
        let mut plain = String::new();
        let mut in_esc = false;
        for c in r.chars() {
            if in_esc {
                in_esc = !c.is_ascii_alphabetic();
            } else if c == '\x1b' {
                in_esc = true;
            } else {
                plain.push(c);
            }
        }
        assert!(plain.contains("LS\n"));
        assert!(plain.contains("TRY THIS"));
        assert!(plain.contains("   * "));
        assert!(plain.contains("    ls -la"));
        for l in r.lines() {
            assert!(visible_len(l) <= 40, "{l:?}");
        }
    }

    #[test]
    fn visible_len_ignores_escapes() {
        assert_eq!(visible_len("\x1b[1;36mabc\x1b[0m"), 3);
    }
}
