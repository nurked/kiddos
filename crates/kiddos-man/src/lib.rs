//! Manual pages are Markdown files at `/usr/share/man/<lang>/<cmd>.md` (and
//! `/games/<cart>/man/<cmd>.md`). This crate renders that Markdown subset
//! into colored console text, finds pages, searches summaries, and pages
//! long output.

pub mod pager;
pub mod render;

pub use pager::page;
pub use render::render;

use kiddos_kernel::{Lang, Proc};

pub const MAN_ROOT: &str = "/usr/share/man";

/// Find the page for `name` in the current language, falling back to
/// English, then to cartridge man dirs. Returns the Markdown source.
pub fn find_page(p: &Proc, name: &str) -> Option<String> {
    let name = name.trim_matches('/');
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        return None;
    }
    let mut candidates = vec![format!("{}/{}/{}.md", MAN_ROOT, p.lang().code(), name)];
    if p.lang() != Lang::En {
        candidates.push(format!("{}/en/{}.md", MAN_ROOT, name));
    }
    if let Ok(carts) = p.fs().readdir("/games") {
        for c in carts {
            candidates.push(format!("/games/{}/man/{}.md", c.name, name));
        }
    }
    candidates.into_iter().find_map(|c| p.fs().read_string(&c).ok())
}

/// The one-line summary of a page: the first `> ...` blockquote line.
pub fn summary_of(md: &str) -> Option<String> {
    md.lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("> ").map(|s| s.trim().to_string()))
}

/// Every page name available in `lang` (plus cartridge pages).
pub fn all_pages(p: &Proc, lang: Lang) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut dirs = vec![format!("{}/{}", MAN_ROOT, lang.code())];
    if lang != Lang::En {
        dirs.push(format!("{}/en", MAN_ROOT));
    }
    if let Ok(carts) = p.fs().readdir("/games") {
        for c in carts {
            dirs.push(format!("/games/{}/man", c.name));
        }
    }
    for d in dirs {
        let Ok(entries) = p.fs().readdir(&d) else { continue };
        for e in entries {
            let Some(name) = e.name.strip_suffix(".md") else {
                continue;
            };
            if out.iter().any(|(n, _)| n == name) {
                continue;
            }
            let md = p.fs().read_string(&format!("{d}/{}", e.name)).unwrap_or_default();
            out.push((name.to_string(), summary_of(&md).unwrap_or_default()));
        }
    }
    out.sort();
    out
}

/// `man -k` / `apropos`: pages whose name or summary contains `query`.
pub fn search(p: &Proc, query: &str) -> Vec<(String, String)> {
    let q = query.to_lowercase();
    all_pages(p, p.lang())
        .into_iter()
        .filter(|(n, s)| n.to_lowercase().contains(&q) || s.to_lowercase().contains(&q))
        .collect()
}
