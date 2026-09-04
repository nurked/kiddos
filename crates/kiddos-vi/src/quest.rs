//! vi-quest: a grid world edited with vi. The cursor is the hero, `#` is
//! stone, `X` is the way out, and each land knows only a few spells.
//! Finishing the last land unlocks /bin/vi for good.

use crate::engine::{Event, Mode, Vi};
use crate::render;
use kiddos_kernel::{CmdResult, Console, Interrupted, Key, Proc};
use serde::Deserialize;

pub const LEVELS_DIR: &str = "/games/vi-quest/levels";
const STORY_ROWS: u16 = 6;
const HINT_AFTER: usize = 25;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Level {
    pub title: String,
    pub story: String,
    /// Keys allowed in normal mode: "h", "dd" (its first key), "Esc", "Enter", "any".
    pub keys: Vec<String>,
    /// "reach" (stand on X) or "text" (the buffer must equal `target`).
    pub goal: String,
    pub target: String,
    pub map: String,
    pub hint: String,
    pub done: String,
}

pub fn load_levels(p: &Proc) -> Vec<Level> {
    let mut names: Vec<String> = p
        .fs()
        .readdir(LEVELS_DIR)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.name.ends_with(".toml"))
        .map(|e| e.name)
        .collect();
    names.sort();
    names
        .iter()
        .filter_map(|n| p.fs().read_string(&format!("{LEVELS_DIR}/{n}")).ok())
        .filter_map(|t| toml::from_str::<Level>(&t).ok())
        .collect()
}

fn key_allowed(level: &Level, k: Key, vi: &Vi) -> bool {
    if vi.mode != Mode::Normal {
        return true; // typing inside insert/command/search is always fine
    }
    if k == Key::Ctrl('c') {
        return true;
    }
    if level.keys.iter().any(|s| s == "any") {
        return true;
    }
    let name = match k {
        Key::Char(c) => c.to_string(),
        Key::Escape => "Esc".into(),
        Key::Enter => "Enter".into(),
        _ => return false,
    };
    level
        .keys
        .iter()
        .any(|s| s == &name || s.starts_with(&name) && s.len() == 2 && name.len() == 1)
}

fn allowed_list(level: &Level) -> String {
    level.keys.join(" ")
}

fn story_band(p: &Proc, n: usize, total: usize, level: &Level, extra: &str) {
    let mut out = String::from("\x1b[H");
    out.push_str(&format!(
        "\x1b[1;36m vi-quest  {}/{}  {}\x1b[0m\x1b[K\n",
        n + 1,
        total,
        level.title
    ));
    let lines: Vec<&str> = level.story.lines().collect();
    for i in 0..(STORY_ROWS as usize - 2) {
        out.push_str(&format!(" {}\x1b[K\n", lines.get(i).unwrap_or(&"")));
    }
    out.push_str(&format!(
        " \x1b[33mspells: {}\x1b[0m   \x1b[90m{}\x1b[0m\x1b[K",
        allowed_list(level),
        extra
    ));
    p.print(&out);
}

fn wait_key(p: &Proc) -> Result<Key, Interrupted> {
    p.readkey()
}

pub fn cmd_vi_quest(p: &Proc, _args: &[String]) -> CmdResult {
    if !p.stdout_is_tty() {
        p.eprintln("vi-quest: I need the screen for this.");
        return Ok(1);
    }
    let levels = load_levels(p);
    if levels.is_empty() {
        p.eprintln("vi-quest: no levels found in /games/vi-quest/levels.");
        return Ok(1);
    }
    let (_, rows) = p.size();
    p.print("\x1b[2J");
    let total = levels.len();
    for (n, level) in levels.iter().enumerate() {
        if !play_level(p, n, total, level, rows)? {
            p.print("\x1b[0m\x1b[2J\x1b[H");
            p.println("You leave the quest for now. Come back with: play vi-quest");
            return Ok(1);
        }
    }
    p.print("\x1b[0m\x1b[2J\x1b[H");
    let badge = " .-------------.\n |  VI WIZARD  |\n |  h j k l    |\n |  :wq        |\n '-------------'\n";
    p.println("\x1b[1;33mYou know the spells. The editor is yours.\x1b[0m");
    p.print(&format!("\x1b[1;33m{badge}\x1b[0m"));
    let _ = p.fs().mkdir_p("~/badges");
    let _ = p.fs().write("~/badges/vi-quest.txt", badge.as_bytes());
    if p.kernel().unlock("vi") {
        p.println("A new word appeared in /bin:  \x1b[1;32mvi\x1b[0m.  Try:  vi story.txt");
    } else {
        p.println("vi was already yours. Try:  vi story.txt");
    }
    p.speak("You are a vi wizard");
    Ok(0)
}

/// Returns Ok(false) if the kid quit.
fn play_level(p: &Proc, n: usize, total: usize, level: &Level, rows: u16) -> Result<bool, Interrupted> {
    let mut map = level.map.clone();
    let (mut sx, mut sy) = (0usize, 0usize);
    for (y, line) in level.map.lines().enumerate() {
        if let Some(x) = line.find('@') {
            sx = x;
            sy = y;
        }
    }
    map = map.replacen('@', " ", 1);
    let mut vi = Vi::new(&map);
    vi.cx = sx;
    vi.cy = sy;
    let band = rows - STORY_ROWS - 1;
    let mut extra = String::new();
    p.print("\x1b[2J");
    loop {
        story_band(p, n, total, level, &extra);
        p.print(&format!(
            "\x1b[{};1H\x1b[90m Ctrl-C leaves the quest\x1b[0m\x1b[K",
            rows
        ));
        render::draw(p, &mut vi, STORY_ROWS, band, true);
        let k = wait_key(p)?;
        if k == Key::Ctrl('c') {
            return Ok(false);
        }
        if !key_allowed(level, k, &vi) {
            let name = match k {
                Key::Char(c) => c.to_string(),
                other => format!("{other:?}"),
            };
            vi.message = format!("This land does not know '{name}'. Its spells: {}", allowed_list(level));
            p.beep(200, 40);
            continue;
        }
        let before = vi.clone();
        let ev = vi.key(k);
        if vi.mode == Mode::Normal && vi.current() == Some('#') {
            vi = before;
            vi.message = "Stone. You cannot go there.".into();
            p.beep(150, 30);
            continue;
        }
        if vi.keys_seen == HINT_AFTER && !level.hint.is_empty() {
            extra = format!("hint: {}", level.hint);
        }
        let won = match level.goal.as_str() {
            // a text goal counts only once the kid is back in normal mode (Esc is the lesson)
            "text" => vi.mode == Mode::Normal && vi.text().trim_end() == level.target.trim_end(),
            "quit" => ev == Event::WriteQuit,
            _ => vi.current() == Some('X'),
        };
        if let Event::Quit { .. } = ev {
            if level.goal != "quit" {
                vi.message = "Not yet. Leaving is the last spell.".into();
            }
        }
        if won {
            p.beep(880, 60);
            p.beep(1320, 90);
            render::draw(p, &mut vi, STORY_ROWS, band, true);
            p.print(&format!(
                "\x1b[{};1H\x1b[1;32m {}  Press any key.\x1b[0m\x1b[K",
                rows,
                if level.done.is_empty() { "Done!" } else { &level.done }
            ));
            let _ = wait_key(p)?;
            return Ok(true);
        }
    }
}
