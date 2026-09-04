//! UI strings. Bundles are Fluent-syntax `.ftl` files (a subset: `key = value`,
//! indented continuation lines, `{ $var }` placeholders, `#` comments). The
//! subset keeps the files compatible with the real `fluent` crate should we
//! need plurals later, while costing zero dependencies today.
//!
//! Every user-visible string the machine says lives here, not in code, so
//! that the machine's *voice* can be edited by non-programmers, and so that
//! other languages can be added later by dropping in another `.ftl` file.
//! Today there is one language: English.

use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Lang {
    #[default]
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
        }
    }
    pub fn from_code(code: &str) -> Option<Lang> {
        match code.trim().to_ascii_lowercase().as_str() {
            "en" | "english" => Some(Lang::En),
            _ => None,
        }
    }
    pub fn all() -> [Lang; 1] {
        [Lang::En]
    }
    /// Human name in its own language.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
        }
    }
}

const EN: &str = include_str!("../locales/en.ftl");

pub struct Bundle {
    lang: Lang,
    strings: HashMap<String, String>,
}

impl Bundle {
    pub fn parse(lang: Lang, src: &str) -> Bundle {
        let mut strings = HashMap::new();
        let mut key: Option<String> = None;
        let mut value = String::new();
        let flush = |key: &mut Option<String>, value: &mut String, strings: &mut HashMap<String, String>| {
            if let Some(k) = key.take() {
                strings.insert(k, std::mem::take(value).trim_end().to_string());
            }
        };
        for line in src.lines() {
            if line.trim_start().starts_with('#') {
                continue;
            }
            if (line.starts_with(' ') || line.starts_with('\t')) && key.is_some() {
                let cont = line.trim_start();
                if !value.is_empty() {
                    value.push('\n');
                }
                value.push_str(cont);
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                    flush(&mut key, &mut value, &mut strings);
                    key = Some(k.to_string());
                    value = v.trim().to_string();
                    continue;
                }
            }
            if line.trim().is_empty() {
                flush(&mut key, &mut value, &mut strings);
            }
        }
        flush(&mut key, &mut value, &mut strings);
        Bundle { lang, strings }
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    pub fn has(&self, key: &str) -> bool {
        self.strings.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.strings.keys().map(|s| s.as_str())
    }

    /// Look up `key` and substitute `{ $name }` placeholders.
    pub fn get(&self, key: &str, args: &[(&str, &str)]) -> Option<String> {
        let raw = self.strings.get(key)?;
        Some(substitute(raw, args))
    }
}

fn substitute(raw: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) => {
                let inner = after[..end].trim();
                if let Some(name) = inner.strip_prefix('$') {
                    match args.iter().find(|(k, _)| *k == name) {
                        Some((_, v)) => out.push_str(v),
                        None => {
                            out.push('{');
                            out.push_str(&after[..end]);
                            out.push('}');
                        }
                    }
                } else {
                    // Fluent string literal { "..." } or unknown: emit literal text
                    out.push_str(inner.trim_matches('"'));
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn bundles() -> &'static [Bundle; 1] {
    static B: OnceLock<[Bundle; 1]> = OnceLock::new();
    B.get_or_init(|| [Bundle::parse(Lang::En, EN)])
}

pub fn bundle(lang: Lang) -> &'static Bundle {
    match lang {
        Lang::En => &bundles()[0],
    }
}

/// Translate `key` in `lang`, falling back to English, then to the key
/// itself (so a missing string is visible, never a crash).
pub fn t(lang: Lang, key: &str, args: &[(&str, &str)]) -> String {
    bundle(lang)
        .get(key, args)
        .or_else(|| bundle(Lang::En).get(key, args))
        .unwrap_or_else(|| key.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_subset() {
        let b = Bundle::parse(
            Lang::En,
            "# comment\nhello = Hi { $name }!\nmulti = line one\n    line two\n\nplain = x = y\n",
        );
        assert_eq!(b.get("hello", &[("name", "Ivan")]).unwrap(), "Hi Ivan!");
        assert_eq!(b.get("multi", &[]).unwrap(), "line one\nline two");
        assert_eq!(b.get("plain", &[]).unwrap(), "x = y");
        assert_eq!(b.get("missing", &[]), None);
    }

    #[test]
    fn english_bundle_loads() {
        let en = bundle(Lang::En);
        assert!(en.keys().count() > 10);
        assert!(en.has("unknown-command"));
    }

    #[test]
    fn falls_back() {
        assert_eq!(t(Lang::En, "no-such-key", &[]), "no-such-key");
        assert!(t(Lang::En, "unknown-command", &[("cmd", "foo")]).contains("foo"));
    }
}
