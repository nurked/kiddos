//! Variables, tilde, globs.

use crate::lexer::Word;
use kiddos_kernel::fs::has_glob;
use kiddos_kernel::Proc;

pub struct Ctx<'a> {
    pub proc: &'a Proc,
    pub last_status: i32,
    pub positional: &'a [String],
    pub script_name: Option<&'a str>,
}

impl<'a> Ctx<'a> {
    pub fn var(&self, name: &str) -> String {
        match name {
            "?" => self.last_status.to_string(),
            "#" => self.positional.len().to_string(),
            "@" | "*" => self.positional.join(" "),
            "$" => self.proc.pid.to_string(),
            "0" => self.script_name.unwrap_or("ksh").to_string(),
            n if n.chars().all(|c| c.is_ascii_digit()) => {
                let i: usize = n.parse().unwrap_or(0);
                if i == 0 {
                    String::new()
                } else {
                    self.positional.get(i - 1).cloned().unwrap_or_default()
                }
            }
            "PWD" => self.proc.cwd(),
            "NAME" => {
                let n = self.proc.kernel().kid_name.lock().clone();
                if n.is_empty() {
                    self.proc.env_get("NAME").unwrap_or_default()
                } else {
                    n
                }
            }
            "LANG" => self.proc.lang().code().to_string(),
            n => self.proc.env_get(n).unwrap_or_default(),
        }
    }

    /// Expand one word into zero or more argument strings (globs may fan
    /// out). A word that is only an unset variable still yields one empty
    /// string when quoted, nothing when unquoted (like a shell).
    pub fn expand(&self, w: &Word) -> Vec<String> {
        let mut text = String::new();
        let mut globbable = false;
        let mut any_quoted = false;
        for (i, seg) in w.segs.iter().enumerate() {
            let s = if seg.var { self.var(&seg.text) } else { seg.text.clone() };
            let s = if i == 0 && !seg.quoted && !seg.var {
                kiddos_vfs::path::expand_tilde(&s, &self.proc.home())
            } else {
                s
            };
            if !seg.quoted && has_glob(&s) {
                globbable = true;
            }
            if seg.quoted {
                any_quoted = true;
            }
            text.push_str(&s);
        }
        if text.is_empty() && !any_quoted {
            return Vec::new();
        }
        if globbable {
            let matches = self.proc.fs().glob(&text);
            if !matches.is_empty() {
                return matches;
            }
        }
        vec![text]
    }

    /// Expand a word that must be exactly one string (redirect targets).
    pub fn expand_one(&self, w: &Word) -> String {
        self.expand(w).into_iter().next().unwrap_or_default()
    }
}
