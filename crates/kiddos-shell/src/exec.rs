//! Running parsed lines: pipelines, redirects, shell builtins, the loop.

use crate::editor::{Editor, ReadOutcome};
use crate::expand::Ctx;
use crate::parser::{parse, Connector, Pipeline, Redirect};
use kiddos_kernel::{Console, Event, Input, Interrupted, Output, Pipe, Proc, Spawn, SpawnError, Topic};
use kiddos_vfs::path::tildify;

pub const SHELL_BUILTINS: [&str; 6] = ["cd", "exit", "export", "unset", "history", "help-shell"];

pub struct Shell<'a> {
    p: &'a Proc,
    login: bool,
    pub last_status: i32,
    pub positional: Vec<String>,
    pub script_name: Option<String>,
    editor: Editor,
    exit_requested: Option<i32>,
}

impl<'a> Shell<'a> {
    pub fn new(p: &'a Proc, login: bool) -> Shell<'a> {
        Shell {
            p,
            login,
            last_status: 0,
            positional: Vec::new(),
            script_name: None,
            editor: Editor::new(),
            exit_requested: None,
        }
    }

    fn history_path(&self) -> String {
        format!("{}/.ksh_history", self.p.home())
    }

    fn prompt(&self) -> String {
        let cfg = self.p.kernel().config();
        let cwd = tildify(&self.p.cwd(), &self.p.home());
        let mark = if self.p.is_root() { "#" } else { "$" };
        format!(
            "\x1b[1;32m{}@{}\x1b[0m:\x1b[1;34m{}\x1b[0m{} ",
            self.p.user, cfg.hostname, cwd, mark
        )
    }

    /// The interactive loop.
    pub fn interactive(&mut self) -> Result<i32, Interrupted> {
        let p = self.p;
        if let Ok(lines) = p.fs().read_lines(&self.history_path()) {
            self.editor.load_history(lines);
        }
        loop {
            if p.killed() {
                return Ok(self.last_status);
            }
            let prompt = self.prompt();
            let outcome = match self.editor.read_line(p, &prompt) {
                Ok(o) => o,
                Err(Interrupted) if self.login && !p.kernel().shutting_down() => continue,
                Err(e) => return Err(e),
            };
            match outcome {
                ReadOutcome::Line(line) => {
                    let line = match self.expand_history(&line) {
                        Some(l) => l,
                        None => continue,
                    };
                    if line.trim().is_empty() {
                        continue;
                    }
                    self.editor.push_history(&line);
                    let hist: String = self.editor.history().join("\n") + "\n";
                    let _ = p.fs().write(&self.history_path(), hist.as_bytes());
                    let status = self.run_line(&line)?;
                    p.kernel().emit(Event::CommandRun {
                        line: line.clone(),
                        status,
                        cwd: p.cwd(),
                    });
                    if let Some(code) = self.exit_requested {
                        return Ok(code);
                    }
                }
                ReadOutcome::Eof => {
                    if self.login {
                        p.println(&p.t("nowhere-to-exit", &[]));
                    } else {
                        return Ok(self.last_status);
                    }
                }
                ReadOutcome::Cancelled => {}
            }
        }
    }

    /// `!!` and `!n`. Returns None if the reference is bad (already reported).
    fn expand_history(&self, line: &str) -> Option<String> {
        let t = line.trim_start();
        if !t.starts_with('!') || t.starts_with("!=") {
            return Some(line.to_string());
        }
        let hist = self.editor.history();
        let (target, rest) = match t[1..].split_once(' ') {
            Some((a, b)) => (a, format!(" {b}")),
            None => (&t[1..], String::new()),
        };
        let found = if target == "!" {
            hist.last().cloned()
        } else if let Ok(n) = target.parse::<usize>() {
            if n >= 1 {
                hist.get(n - 1).cloned()
            } else {
                None
            }
        } else {
            hist.iter().rev().find(|h| h.starts_with(target)).cloned()
        };
        match found {
            Some(h) => {
                let full = format!("{h}{rest}");
                self.p.println(&full);
                Some(full)
            }
            None => {
                self.p.println(&self.p.t("history-empty", &[]));
                None
            }
        }
    }

    pub fn run_script(&mut self, src: &str) -> Result<i32, Interrupted> {
        for line in src.lines() {
            self.p.check()?;
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            self.run_line(line)?;
            if let Some(code) = self.exit_requested {
                return Ok(code);
            }
        }
        Ok(self.last_status)
    }

    pub fn run_line(&mut self, line: &str) -> Result<i32, Interrupted> {
        let list = match parse(line) {
            Ok(l) => l,
            Err(e) => {
                self.p.eprintln(&format!("ksh: {e}"));
                self.last_status = 2;
                return Ok(2);
            }
        };
        let mut prev = Connector::Always;
        let mut prev_status = 0;
        for (pipeline, conn) in list.items {
            let run = match prev {
                Connector::Always => true,
                Connector::IfOk => prev_status == 0,
                Connector::IfFailed => prev_status != 0,
            };
            if run {
                prev_status = self.run_pipeline(&pipeline)?;
                self.last_status = prev_status;
                if self.exit_requested.is_some() {
                    break;
                }
            }
            prev = conn;
        }
        Ok(self.last_status)
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx {
            proc: self.p,
            last_status: self.last_status,
            positional: &self.positional,
            script_name: self.script_name.as_deref(),
        }
    }

    fn run_pipeline(&mut self, pl: &Pipeline) -> Result<i32, Interrupted> {
        let p = self.p;
        let n = pl.cmds.len();
        // single simple command: maybe a shell builtin or an assignment
        if n == 1 {
            let argv: Vec<String> = pl.cmds[0].words.iter().flat_map(|w| self.ctx().expand(w)).collect();
            if let Some(first) = argv.first() {
                if argv.len() == 1 && is_assignment(first) {
                    let (k, v) = first.split_once('=').unwrap();
                    p.env_set(k, v);
                    return Ok(0);
                }
                if SHELL_BUILTINS.contains(&first.as_str()) {
                    return self.builtin(&argv);
                }
            }
        }
        let mut children = Vec::new();
        let mut prev_reader = None;
        let mut status = 0;
        for (i, cmd) in pl.cmds.iter().enumerate() {
            p.check()?;
            let argv: Vec<String> = cmd.words.iter().flat_map(|w| self.ctx().expand(w)).collect();
            let mut s = Spawn::child_of(p, argv.clone());
            s.stdin = match prev_reader.take() {
                Some(r) => Input::Pipe(r),
                None => Input::Tty,
            };
            let mut next_reader = None;
            if i + 1 < n {
                let (r, w) = Pipe::pair();
                next_reader = Some(r);
                s.stdout = Output::Pipe(w);
            }
            let mut redirect_failed = false;
            for r in &cmd.redirects {
                match r {
                    Redirect::Out { fd, target, append } => {
                        let path = self.ctx().expand_one(target);
                        match p.fs().open_for_write(&path, *append) {
                            Ok(abs) => {
                                let out = match abs.as_str() {
                                    "/dev/null" => Output::Null,
                                    "/dev/tty" => Output::Tty,
                                    "/dev/speaker" => Output::Speaker(Default::default()),
                                    _ => Output::File { path: abs },
                                };
                                if *fd == 2 {
                                    s.stderr = out;
                                } else {
                                    s.stdout = out;
                                }
                            }
                            Err(e) => {
                                p.eprintln(&format!("ksh: {}", p.explain(&e)));
                                redirect_failed = true;
                            }
                        }
                    }
                    Redirect::In { source } => {
                        let path = self.ctx().expand_one(source);
                        match p.fs().read(&path) {
                            Ok(data) => s.stdin = Input::bytes(data),
                            Err(e) => {
                                p.eprintln(&format!("ksh: {}", p.explain(&e)));
                                redirect_failed = true;
                            }
                        }
                    }
                }
            }
            prev_reader = next_reader;
            if redirect_failed {
                status = 1;
                continue;
            }
            if argv.is_empty() {
                // `> file` alone: the redirect did its job
                status = 0;
                continue;
            }
            match p.spawn(s) {
                Ok(child) => children.push(child),
                Err(e) => {
                    self.report_spawn_error(&argv[0], &e);
                    status = 127;
                }
            }
        }
        let mut last = None;
        for c in children {
            last = Some(c.wait());
        }
        if let Some(l) = last {
            status = l;
        }
        if status == 130 {
            p.print("^C\n");
        }
        Ok(status)
    }

    fn report_spawn_error(&self, name: &str, e: &SpawnError) {
        let p = self.p;
        match e {
            SpawnError::NotFound(n) => {
                p.eprintln(&p.t("unknown-command", &[("cmd", n)]));
                if let Some(s) = self.suggest(n) {
                    p.eprintln(&p.t("did-you-mean", &[("cmd", &s)]));
                }
            }
            SpawnError::NotExecutable(path) => {
                let shown = tildify(path, &p.home());
                p.eprintln(&p.t("not-executable", &[("path", &shown)]));
            }
            SpawnError::IsDir(path) => {
                let shown = tildify(path, &p.home());
                p.eprintln(&p.t("is-dir", &[("path", &shown)]));
            }
            SpawnError::ParentOnly(_) => p.eprintln(&p.t("parent-only", &[])),
            SpawnError::Vfs(e) => p.eprintln(&format!("{name}: {}", p.explain(e))),
        }
    }

    fn suggest(&self, typo: &str) -> Option<String> {
        if typo.chars().count() < 2 {
            return None;
        }
        let mut names: Vec<String> = self
            .p
            .kernel()
            .commands()
            .into_iter()
            .filter(|c| c.topic != Topic::Hidden && (!c.parent_only || self.p.is_root()))
            .map(|c| c.name.to_string())
            .collect();
        names.extend(SHELL_BUILTINS.iter().take(5).map(|s| s.to_string()));
        let typo_l = typo.to_lowercase();
        // rank by edit distance, then by how close the lengths are
        let mut best: Option<((usize, bool, usize), String)> = None;
        for n in names {
            let d = strsim::levenshtein(&typo_l, &n);
            let limit = if typo.len() <= 3 { 1 } else { 2 };
            let related = typo_l.starts_with(&n) || n.starts_with(&typo_l);
            let score = (d, !related, typo.len().abs_diff(n.len()));
            if d <= limit && best.as_ref().map(|(bs, _)| score < *bs).unwrap_or(true) {
                best = Some((score, n));
            }
        }
        best.map(|(_, n)| n)
    }

    fn builtin(&mut self, argv: &[String]) -> Result<i32, Interrupted> {
        let p = self.p;
        match argv[0].as_str() {
            "cd" => {
                let target = match argv.get(1).map(|s| s.as_str()) {
                    None | Some("~") => p.home(),
                    Some("-") => p.env_get("OLDPWD").unwrap_or_else(|| p.cwd()),
                    Some(t) => p.fs().path(t),
                };
                match p.fs().stat(&target) {
                    Ok(st) if st.is_dir() => {
                        p.env_set("OLDPWD", &p.cwd());
                        p.set_cwd(&target);
                        p.env_set("PWD", &target);
                        Ok(0)
                    }
                    Ok(_) => {
                        p.eprintln(&format!(
                            "cd: {}",
                            p.t("not-dir", &[("path", &tildify(&target, &p.home()))])
                        ));
                        Ok(1)
                    }
                    Err(e) => {
                        p.eprintln(&format!("cd: {}", p.explain(&e)));
                        Ok(1)
                    }
                }
            }
            "exit" => {
                if self.login {
                    p.println(&p.t("nowhere-to-exit", &[]));
                    Ok(0)
                } else {
                    let code = argv.get(1).and_then(|s| s.parse().ok()).unwrap_or(self.last_status);
                    self.exit_requested = Some(code);
                    Ok(code)
                }
            }
            "export" => {
                if argv.len() == 1 {
                    for (k, v) in p.env_all() {
                        p.println(&format!("export {k}={v}"));
                    }
                    return Ok(0);
                }
                for a in &argv[1..] {
                    match a.split_once('=') {
                        Some((k, v)) => p.env_set(k, v),
                        None => {
                            if p.env_get(a).is_none() {
                                p.env_set(a, "");
                            }
                        }
                    }
                }
                Ok(0)
            }
            "unset" => {
                for a in &argv[1..] {
                    p.env_unset(a);
                }
                Ok(0)
            }
            "history" => {
                if argv.get(1).map(|s| s == "-c").unwrap_or(false) {
                    self.editor.clear_history();
                    let _ = p.fs().write(&self.history_path(), b"");
                    return Ok(0);
                }
                let h = self.editor.history();
                if h.is_empty() {
                    p.println(&p.t("history-empty", &[]));
                }
                for (i, l) in h.iter().enumerate() {
                    p.println(&format!("{:>4}  {}", i + 1, l));
                }
                Ok(0)
            }
            _ => Ok(0),
        }
    }
}

fn is_assignment(s: &str) -> bool {
    let Some((k, _)) = s.split_once('=') else {
        return false;
    };
    !k.is_empty()
        && k.chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
        && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
