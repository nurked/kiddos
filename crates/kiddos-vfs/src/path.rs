//! Pure path helpers. No filesystem access.

/// Join `path` onto `cwd` (both Unix-style) and collapse `.` and `..`
/// lexically. Result is always absolute. `..` above `/` stays at `/`.
pub fn normalize(cwd: &str, path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let joined;
    let full: &str = if path.starts_with('/') {
        path
    } else {
        joined = format!("{}/{}", cwd, path);
        &joined
    };
    for c in full.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            c => parts.push(c),
        }
    }
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

/// Expand a leading `~` or `~/` to `home`.
pub fn expand_tilde(path: &str, home: &str) -> String {
    if path == "~" {
        home.to_string()
    } else if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{}", home.trim_end_matches('/'), rest)
    } else {
        path.to_string()
    }
}

pub fn basename(path: &str) -> &str {
    let p = path.trim_end_matches('/');
    if p.is_empty() {
        return "/";
    }
    p.rsplit('/').next().unwrap_or(p)
}

pub fn dirname(path: &str) -> String {
    let p = path.trim_end_matches('/');
    if p.is_empty() {
        return "/".to_string();
    }
    match p.rfind('/') {
        Some(0) | None if p.starts_with('/') => "/".to_string(),
        Some(i) => p[..i].to_string(),
        None => ".".to_string(),
    }
}

/// Show `/home/kid/x` as `~/x`.
pub fn tildify(path: &str, home: &str) -> String {
    if path == home {
        "~".to_string()
    } else if let Some(rest) = path.strip_prefix(&format!("{}/", home)) {
        format!("~/{}", rest)
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        assert_eq!(normalize("/home/kid", "a/b"), "/home/kid/a/b");
        assert_eq!(normalize("/home/kid", ".."), "/home");
        assert_eq!(normalize("/home/kid", "../../../.."), "/");
        assert_eq!(normalize("/home/kid", "/x//y/./z/"), "/x/y/z");
        assert_eq!(normalize("/", "."), "/");
    }

    #[test]
    fn names() {
        assert_eq!(basename("/a/b/c"), "c");
        assert_eq!(basename("/"), "/");
        assert_eq!(dirname("/a/b/c"), "/a/b");
        assert_eq!(dirname("/a"), "/");
        assert_eq!(dirname("/"), "/");
        assert_eq!(tildify("/home/kid/x", "/home/kid"), "~/x");
        assert_eq!(tildify("/home/kid", "/home/kid"), "~");
        assert_eq!(expand_tilde("~/a", "/home/kid"), "/home/kid/a");
    }
}
