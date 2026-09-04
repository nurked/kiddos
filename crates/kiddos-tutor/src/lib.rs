//! The tutor lives in `/lessons/<lang>/*.toml` and in `/home/kid/.progress`.
//! It listens to every command the shell runs; when the command matches the
//! current step it talks, hands out hints after a few misses, and saves a
//! badge when a lesson ends. All of its state is a file the kid can `cat`.

use kiddos_kernel::fs::glob_match;
use kiddos_kernel::{Actor, CmdResult, Command, Event, Kernel, Proc, Topic, KID_HOME, KID_USER};
use kiddos_vfs::{normalize, path::expand_tilde};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Weak};

pub const LESSONS_DIR: &str = "/lessons";
pub const PROGRESS_FILE: &str = "/home/kid/.progress";
pub const BADGES_DIR: &str = "/home/kid/badges";
const HINT_AFTER_TRIES: u32 = 3;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Step {
    /// What the tutor says when this step begins.
    pub say: String,
    /// Glob patterns the typed line must match (any of them).
    pub expect: Vec<String>,
    /// Extra condition on the machine: `cwd P`, `exists P`, `missing P`,
    /// `contains P TEXT`, `dir P`, `file P`.
    pub check: String,
    /// Hint chain, given one at a time.
    pub hint: Vec<String>,
    /// What the tutor says when the step is done.
    pub done: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Lesson {
    pub id: String,
    pub title: String,
    pub intro: String,
    pub outro: String,
    pub badge: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Progress {
    pub current: String,
    pub step: usize,
    pub completed: Vec<String>,
    pub active: bool,
    pub hints_used: u32,
}

struct Inner {
    lessons: Vec<Lesson>,
    progress: Progress,
    tries: u32,
    hint_idx: usize,
    loaded: bool,
}

pub struct Tutor {
    kernel: Weak<Kernel>,
    inner: Mutex<Inner>,
}

fn kid() -> Actor {
    Actor::user(KID_USER)
}

impl Tutor {
    /// Wire the tutor into a kernel: commands, event listener, extension.
    /// Call before `boot`.
    pub fn install(kernel: &Arc<Kernel>) -> Arc<Tutor> {
        let tutor = Arc::new(Tutor {
            kernel: Arc::downgrade(kernel),
            inner: Mutex::new(Inner {
                lessons: Vec::new(),
                progress: Progress::default(),
                tries: 0,
                hint_idx: 0,
                loaded: false,
            }),
        });
        kernel.set_extension(tutor.clone());
        let t = tutor.clone();
        kernel.subscribe(Box::new(move |e| t.on_event(e)));
        for c in [
            Command::new(
                "tutor",
                cmd_tutor,
                "the tutor: what to do next (tutor off / on / list)",
                Topic::Learning,
            ),
            Command::new("lesson", cmd_lesson, "jump to a lesson by number", Topic::Learning),
            Command::new("hint", cmd_hint, "stuck? get a hint", Topic::Learning),
            Command::new("progress", cmd_progress, "which lessons you have done", Topic::Learning),
            Command::new("badges", cmd_badges, "show the badges you earned", Topic::Learning),
        ] {
            kernel.register(c);
        }
        tutor
    }

    fn kernel(&self) -> Option<Arc<Kernel>> {
        self.kernel.upgrade()
    }

    fn ensure_loaded(&self, k: &Kernel, inner: &mut Inner) {
        if inner.loaded {
            return;
        }
        inner.lessons = load_lessons(k);
        inner.progress = load_progress(k);
        if inner.progress.current.is_empty() || !inner.lessons.iter().any(|l| l.id == inner.progress.current) {
            if let Some(first) = inner.lessons.iter().find(|l| !inner.progress.completed.contains(&l.id)) {
                inner.progress.current = first.id.clone();
                inner.progress.step = 0;
            }
        }
        if !k.vfs.lock().exists(PROGRESS_FILE) {
            inner.progress.active = true;
            save_progress(k, &inner.progress);
        }
        inner.loaded = true;
    }

    /// Reload lessons and progress from the drive (the kid may have edited
    /// `.progress` by hand — that is allowed).
    pub fn reload(&self) {
        let mut inner = self.inner.lock();
        inner.loaded = false;
        if let Some(k) = self.kernel() {
            self.ensure_loaded(&k, &mut inner);
        }
    }

    fn on_event(&self, e: &Event) {
        let Some(k) = self.kernel() else { return };
        match e {
            Event::Boot => {
                let mut inner = self.inner.lock();
                self.ensure_loaded(&k, &mut inner);
            }
            Event::LangChanged(_) => {
                let mut inner = self.inner.lock();
                inner.loaded = false;
                self.ensure_loaded(&k, &mut inner);
            }
            Event::CommandRun { line, status, cwd } => {
                let mut inner = self.inner.lock();
                self.ensure_loaded(&k, &mut inner);
                self.on_command(&k, &mut inner, line, *status, cwd);
            }
            _ => {}
        }
    }

    fn on_command(&self, k: &Kernel, inner: &mut Inner, line: &str, status: i32, cwd: &str) {
        if !inner.progress.active {
            return;
        }
        let line = normalize_line(line);
        // typing the tutor's own commands, or starting a game, never counts as a miss
        if [
            "tutor", "hint", "progress", "badges", "lesson", "clear", "play", "games",
        ]
        .iter()
        .any(|c| line == *c || line.starts_with(&format!("{c} ")))
        {
            return;
        }
        // inside a game's world the kid is playing, not studying: stay quiet
        if in_a_game(k, cwd) {
            return;
        }
        let Some(li) = inner.lessons.iter().position(|l| l.id == inner.progress.current) else {
            return;
        };
        let step_i = inner.progress.step;
        let Some(step) = inner.lessons[li].steps.get(step_i).cloned() else {
            return;
        };
        let matched = status == 0
            && step.expect.iter().any(|pat| glob_match(&normalize_line(pat), &line))
            && check_ok(k, &step.check, cwd);
        if !matched {
            inner.tries += 1;
            if inner.tries >= HINT_AFTER_TRIES {
                inner.tries = 0;
                // offer each hint once; after that, wait to be asked (`hint`)
                if inner.hint_idx < step.hint.len().max(1) {
                    let hint = self.next_hint(inner, &step);
                    say(k, &hint);
                }
            }
            return;
        }
        inner.tries = 0;
        inner.hint_idx = 0;
        if !step.done.is_empty() {
            say(k, &step.done);
        }
        inner.progress.step += 1;
        let lesson = inner.lessons[li].clone();
        if inner.progress.step < lesson.steps.len() {
            let next = &lesson.steps[inner.progress.step];
            say(k, &next.say);
        } else {
            // lesson complete
            if !lesson.outro.is_empty() {
                say(k, &lesson.outro);
            }
            if !lesson.badge.is_empty() {
                let mut vfs = k.vfs.lock();
                let _ = vfs.mkdir_p(BADGES_DIR, &kid());
                let _ = vfs.write(
                    &format!("{BADGES_DIR}/{}.txt", lesson.id),
                    lesson.badge.as_bytes(),
                    &kid(),
                );
                drop(vfs);
                k.screen
                    .lock()
                    .write_str(&format!("\x1b[1;33m{}\x1b[0m\n", lesson.badge.trim_end()));
            }
            if !inner.progress.completed.contains(&lesson.id) {
                inner.progress.completed.push(lesson.id.clone());
            }
            k.speak("Well done!");
            k.log(&format!("lesson done: {}", lesson.id));
            match inner.lessons.get(li + 1).cloned() {
                Some(next) => {
                    inner.progress.current = next.id.clone();
                    inner.progress.step = 0;
                    say(
                        k,
                        &format!(
                            "Lesson {} done! Next: {}. Type tutor when you are ready.",
                            li + 1,
                            next.title
                        ),
                    );
                }
                None => {
                    inner.progress.step = lesson.steps.len();
                    say(k, "That was the last lesson I have. You know the machine now. Go play.");
                }
            }
        }
        save_progress(k, &inner.progress);
    }

    fn next_hint(&self, inner: &mut Inner, step: &Step) -> String {
        inner.progress.hints_used += 1;
        if step.hint.is_empty() {
            inner.hint_idx = 1;
            return format!("Reminder: {}", step.say);
        }
        let h = step.hint[inner.hint_idx.min(step.hint.len() - 1)].clone();
        inner.hint_idx += 1;
        format!("Hint: {h}")
    }
}

/// Is `cwd` inside a folder some cartridge declared as its world?
fn in_a_game(k: &Kernel, cwd: &str) -> bool {
    let vfs = k.vfs.lock();
    kiddos_cart::all_worlds(&vfs, KID_HOME)
        .iter()
        .any(|w| cwd == w || cwd.starts_with(&format!("{}/", w.trim_end_matches('/'))))
}

fn normalize_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Evaluate a step's `check` against the machine.
fn check_ok(k: &Kernel, check: &str, cwd: &str) -> bool {
    let check = check.trim();
    if check.is_empty() {
        return true;
    }
    let mut parts = check.splitn(3, ' ');
    let op = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("");
    let abs = normalize(cwd, &expand_tilde(path, KID_HOME));
    let vfs = k.vfs.lock();
    match op {
        "cwd" => normalize("/", &expand_tilde(path, KID_HOME)) == cwd,
        "exists" => vfs.exists(&abs),
        "missing" => !vfs.exists(&abs),
        "dir" => vfs.is_dir(&abs),
        "file" => vfs.stat(&abs).map(|s| s.is_file()).unwrap_or(false),
        "contains" => vfs
            .read_string(&abs, &Actor::root())
            .map(|s| s.to_lowercase().contains(&rest.to_lowercase()))
            .unwrap_or(false),
        _ => true,
    }
}

/// The tutor's voice on screen.
fn say(k: &Kernel, text: &str) {
    let mut out = String::new();
    for (i, line) in text.trim_end().lines().enumerate() {
        if i == 0 {
            out.push_str(&format!("\x1b[1;35m\u{263A} \x1b[0;35m{line}\x1b[0m\n"));
        } else {
            out.push_str(&format!("  \x1b[35m{line}\x1b[0m\n"));
        }
    }
    k.screen.lock().write_str(&out);
}

fn lessons_dir(k: &Kernel) -> String {
    format!("{LESSONS_DIR}/{}", k.lang().code())
}

pub fn load_lessons(k: &Kernel) -> Vec<Lesson> {
    let vfs = k.vfs.lock();
    let dir = lessons_dir(k);
    let Ok(entries) = vfs.readdir(&dir, &Actor::root()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries {
        if !e.name.ends_with(".toml") {
            continue;
        }
        let Ok(text) = vfs.read_string(&format!("{dir}/{}", e.name), &Actor::root()) else {
            continue;
        };
        match toml::from_str::<Lesson>(&text) {
            Ok(mut l) => {
                if l.id.is_empty() {
                    l.id = e.name.trim_end_matches(".toml").to_string();
                }
                out.push(l);
            }
            Err(err) => k.log(&format!("bad lesson {}: {err}", e.name)),
        }
    }
    out
}

pub fn load_progress(k: &Kernel) -> Progress {
    k.vfs
        .lock()
        .read_string(PROGRESS_FILE, &Actor::root())
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_progress(k: &Kernel, p: &Progress) {
    let text = format!(
        "# The tutor keeps its notes here. You can read it (cat ~/.progress).\n# You can even change it. That is allowed.\n{}",
        toml::to_string(p).unwrap_or_default()
    );
    let _ = k.vfs.lock().write(PROGRESS_FILE, text.as_bytes(), &kid());
}

// ---- commands ----------------------------------------------------------

fn tutor_of(p: &Proc) -> Option<Arc<Tutor>> {
    p.kernel().extension::<Tutor>()
}

fn cmd_tutor(p: &Proc, args: &[String]) -> CmdResult {
    let Some(t) = tutor_of(p) else { return Ok(1) };
    t.reload();
    let k = p.kernel();
    let mut inner = t.inner.lock();
    match args.first().map(|s| s.as_str()) {
        Some("off") => {
            inner.progress.active = false;
            save_progress(k, &inner.progress);
            p.println("OK, I'll be quiet. Type tutor on when you want me back.");
            return Ok(0);
        }
        Some("on") => {
            inner.progress.active = true;
            save_progress(k, &inner.progress);
        }
        Some("list") => {
            for (i, l) in inner.lessons.iter().enumerate() {
                p.println(&format!("{:>2}. {}", i + 1, l.title));
            }
            return Ok(0);
        }
        Some("restart") => {
            inner.progress = Progress {
                current: inner.lessons.first().map(|l| l.id.clone()).unwrap_or_default(),
                active: true,
                ..Default::default()
            };
            inner.tries = 0;
            inner.hint_idx = 0;
            save_progress(k, &inner.progress);
            p.println("Starting over from lesson 1.");
        }
        _ => {}
    }
    if inner.lessons.is_empty() {
        p.println("I have no lessons on this drive.");
        return Ok(1);
    }
    let Some(li) = inner.lessons.iter().position(|l| l.id == inner.progress.current) else {
        p.println("All lessons are done. Type progress to see them.");
        return Ok(0);
    };
    let lesson = &inner.lessons[li];
    p.println(&format!(
        "\x1b[1mLesson {} of {}: {}\x1b[0m",
        li + 1,
        inner.lessons.len(),
        lesson.title
    ));
    if !inner.progress.active {
        p.println("(The tutor is off. Type tutor on to turn it back on.)");
    }
    match lesson.steps.get(inner.progress.step) {
        Some(step) => {
            if inner.progress.step == 0 && !lesson.intro.is_empty() {
                say(k, &lesson.intro);
            }
            say(k, &step.say);
        }
        None => p.println("This lesson is done. Type progress."),
    }
    Ok(0)
}

fn cmd_lesson(p: &Proc, args: &[String]) -> CmdResult {
    let Some(t) = tutor_of(p) else { return Ok(1) };
    t.reload();
    let n: usize = match args.first().and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None => {
            p.println(&p.t("usage", &[("usage", "lesson <number>   (see them with tutor list)")]));
            return Ok(1);
        }
    };
    {
        let k = p.kernel();
        let mut inner = t.inner.lock();
        let Some(l) = inner.lessons.get(n.wrapping_sub(1)).cloned() else {
            p.println(&format!("I have lessons 1 to {}.", inner.lessons.len()));
            return Ok(1);
        };
        inner.progress.current = l.id;
        inner.progress.step = 0;
        inner.progress.active = true;
        inner.tries = 0;
        inner.hint_idx = 0;
        save_progress(k, &inner.progress);
    }
    cmd_tutor(p, &[])
}

fn cmd_hint(p: &Proc, _args: &[String]) -> CmdResult {
    let Some(t) = tutor_of(p) else { return Ok(1) };
    let k = p.kernel();
    let mut inner = t.inner.lock();
    let step = inner
        .lessons
        .iter()
        .find(|l| l.id == inner.progress.current)
        .and_then(|l| l.steps.get(inner.progress.step).cloned());
    match step {
        Some(step) => {
            let h = t.next_hint(&mut inner, &step);
            say(k, &h);
            save_progress(k, &inner.progress);
        }
        None => p.println("No lesson is running. Type tutor."),
    }
    Ok(0)
}

fn cmd_progress(p: &Proc, _args: &[String]) -> CmdResult {
    let Some(t) = tutor_of(p) else { return Ok(1) };
    t.reload();
    let inner = t.inner.lock();
    for (i, l) in inner.lessons.iter().enumerate() {
        let mark = if inner.progress.completed.contains(&l.id) {
            "\x1b[1;32m\u{2713}\x1b[0m"
        } else if l.id == inner.progress.current {
            "\x1b[1;33m>\x1b[0m"
        } else {
            " "
        };
        let steps = if l.id == inner.progress.current && !inner.progress.completed.contains(&l.id) {
            format!("  (step {} of {})", inner.progress.step + 1, l.steps.len())
        } else {
            String::new()
        };
        p.println(&format!(" {mark} {:>2}. {}{}", i + 1, l.title, steps));
    }
    p.println(&format!(
        "{} of {} done, {} hints used. Notes in ~/.progress, badges in ~/badges.",
        inner.progress.completed.len(),
        inner.lessons.len(),
        inner.progress.hints_used
    ));
    Ok(0)
}

fn cmd_badges(p: &Proc, _args: &[String]) -> CmdResult {
    let entries = p.fs().readdir(BADGES_DIR).unwrap_or_default();
    if entries.is_empty() {
        p.println("No badges yet. Finish a lesson (type tutor) or a game.");
        return Ok(0);
    }
    for e in entries {
        if let Ok(text) = p.fs().read_string(&format!("{BADGES_DIR}/{}", e.name)) {
            p.println(&format!("\x1b[1;33m{}\x1b[0m", text.trim_end()));
            p.println("");
        }
    }
    Ok(0)
}
