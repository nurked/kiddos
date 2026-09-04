//! Prison Escape: you wake up inside vi. The only way out is the one every
//! programmer had to learn the hard way.

use crate::engine::{Event, Mode, Vi};
use crate::render;
use kiddos_kernel::{CmdResult, Console, Interrupted, Key, Proc};

const STORY_ROWS: u16 = 7;

struct Cell {
    title: &'static str,
    story: [&'static str; 4],
    walls: &'static str,
    /// (after N keys, say this)
    hints: [(usize, &'static str); 4],
    start_dirty: bool,
}

const CELLS: [Cell; 3] = [
    Cell {
        title: "Cell 1",
        story: [
            "You wake up in a cell. The walls are made of text.",
            "A sign says: THIS IS VI. Every key does something in here.",
            "Somewhere there is a door. Nobody tells you the word for it.",
            "",
        ],
        walls: "################################\n#                              #\n#   You are the blinking box.  #\n#   The door has no handle.    #\n#                              #\n#   Someone scratched here:    #\n#   'press Escape first'       #\n#                              #\n################################",
        hints: [
            (6, "A guard mutters: ...they always mash keys. Escape gets you back to calm."),
            (12, "The guard again: ...then the colon. The little ':' key. It opens the door's ear."),
            (20, "The guard, bored: ...and q. q for quit. Then Enter."),
            (30, "Chalk on the wall, plain as day:   :q   then Enter"),
        ],
        start_dirty: false,
    },
    Cell {
        title: "Cell 2",
        story: [
            "Another cell. This time you scratched your name on the wall",
            "before you woke up. The door remembers changes.",
            "It will not open for a plain :q now. It wants you to mean it.",
            "",
        ],
        walls: "################################\n#                              #\n#   KID WAS HERE  (scratched)  #\n#                              #\n#   The door hums:             #\n#   'no write since last       #\n#    change'                   #\n#                              #\n################################",
        hints: [
            (6, "Try the old word. See what the door says."),
            (12, "The guard: ...E37 means the door noticed your scratch. Saving is forbidden here."),
            (20, "The guard: ...there is a way to say 'I don't care, let me out': add a bang."),
            (30, "Chalk on the wall:   :q!   (the ! means: yes, throw it away)"),
        ],
        start_dirty: true,
    },
    Cell {
        title: "Cell 3",
        story: [
            "The last cell. Here the door opens only for prisoners who",
            "leave a note for the next one. Write something on the wall,",
            "then leave AND save. One command does both.",
            "",
        ],
        walls: "################################\n#                              #\n#   (write your note here)     #\n#                              #\n#   Others wrote:              #\n#   'i to write, Esc to stop'  #\n#   'w means write'            #\n#                              #\n################################",
        hints: [
            (6, "First write: press i, type, press Escape."),
            (12, "The guard: ...q leaves. w writes. Together they are :wq"),
            (20, "The guard: ...a note first! The door checks for one."),
            (30, "Chalk on the wall:   i  your note  Esc  then  :wq"),
        ],
        start_dirty: false,
    },
];

fn story_band(p: &Proc, n: usize, cell: &Cell, hint: &str) {
    let mut out = String::from("\x1b[H");
    out.push_str(&format!(
        "\x1b[1;36m PRISON ESCAPE  {}  ({} of 3)\x1b[0m\x1b[K\n",
        cell.title,
        n + 1
    ));
    for line in cell.story.iter() {
        out.push_str(&format!(" {line}\x1b[K\n"));
    }
    out.push_str(&format!(" \x1b[33m{hint}\x1b[0m\x1b[K\n"));
    out.push_str("\x1b[K");
    p.print(&out);
}

pub fn cmd_prison(p: &Proc, _args: &[String]) -> CmdResult {
    if !p.stdout_is_tty() {
        p.eprintln("prison-escape: I need the screen for this.");
        return Ok(1);
    }
    let (_, rows) = p.size();
    p.print("\x1b[2J");
    let mut days = 0usize;
    for (n, cell) in CELLS.iter().enumerate() {
        match play_cell(p, n, cell, rows)? {
            Some(d) => days += d,
            None => {
                p.print("\x1b[0m\x1b[2J\x1b[H");
                p.println("You stay in prison for now. (play prison-escape to try again)");
                return Ok(1);
            }
        }
    }
    p.print("\x1b[0m\x1b[2J\x1b[H");
    let badge = " .-------------.\n |  ESCAPED VI |\n |    :q!      |\n '-------------'\n";
    p.println(&format!(
        "\x1b[1;33mFree! It took you {days} key presses. Every programmer remembers their first time.\x1b[0m"
    ));
    p.print(&format!("\x1b[1;33m{badge}\x1b[0m"));
    p.println("You now know how to leave vi. vi-quest teaches you how to move around in it.");
    let _ = p.fs().mkdir_p("~/badges");
    let _ = p.fs().write("~/badges/prison-escape.txt", badge.as_bytes());
    p.speak("You escaped from vi");
    Ok(0)
}

/// Returns Some(keys pressed) when escaped, None when the kid gave up (Ctrl-C).
fn play_cell(p: &Proc, n: usize, cell: &Cell, rows: u16) -> Result<Option<usize>, Interrupted> {
    let mut vi = Vi::new(cell.walls);
    vi.cx = 4;
    vi.cy = 2;
    vi.dirty = cell.start_dirty;
    let band = rows - STORY_ROWS - 1;
    let mut hint = String::new();
    p.print("\x1b[2J");
    loop {
        story_band(p, n, cell, &hint);
        p.print(&format!("\x1b[{};1H\x1b[90m Ctrl-C gives up\x1b[0m\x1b[K", rows));
        render::draw(p, &mut vi, STORY_ROWS, band, false);
        let k = p.readkey()?;
        if k == Key::Ctrl('c') && vi.mode == Mode::Normal {
            return Ok(None);
        }
        let ev = vi.key(k);
        for (after, text) in cell.hints.iter() {
            if vi.keys_seen == *after {
                hint = text.to_string();
            }
        }
        let escaped = match (n, ev) {
            (0, Event::Quit { .. }) => true,
            (0, Event::WriteQuit) => true,
            (1, Event::Quit { force: false }) => {
                vi.message = "E37: No write since last change (add ! to override)".into();
                false
            }
            (1, Event::WriteQuit) | (1, Event::Write) => {
                vi.message = "E45: 'readonly' option is set. The guard forbids saving here.".into();
                false
            }
            (1, Event::Quit { force: true }) => true,
            (2, Event::WriteQuit) => {
                if vi.dirty {
                    true
                } else {
                    vi.message = "The door checks the wall: no note yet. (i, type, Esc, then :wq)".into();
                    false
                }
            }
            (2, Event::Quit { force: true }) => {
                vi.message = "You could leave, but the door only opens for a saved note. Try :wq".into();
                false
            }
            (2, Event::Quit { force: false }) => {
                vi.message = if vi.dirty {
                    "E37: No write since last change (add ! to override)".into()
                } else {
                    "Write a note first.".into()
                };
                false
            }
            (_, Event::Unknown(c)) => {
                vi.message = format!("E492: Not an editor command: {c}");
                false
            }
            _ => false,
        };
        if escaped {
            p.beep(660, 60);
            p.beep(990, 120);
            render::draw(p, &mut vi, STORY_ROWS, band, false);
            p.print(&format!(
                "\x1b[{};1H\x1b[1;32m The door swings open.  Press any key.\x1b[0m\x1b[K",
                rows
            ));
            let _ = p.readkey()?;
            return Ok(Some(vi.keys_seen));
        }
    }
}
