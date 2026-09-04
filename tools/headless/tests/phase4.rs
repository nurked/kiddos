//! Phase 4: vi, earned by playing.

use kiddos_headless::Machine;
use kiddos_kernel::Key;

fn press(m: &Machine, s: &str) {
    for c in s.chars() {
        match c {
            '\u{1b}' => m.kernel.push_key(Key::Escape),
            '\n' => m.kernel.push_key(Key::Enter),
            c => m.kernel.push_key(Key::Char(c)),
        }
    }
    m.settle();
}

#[test]
fn vi_is_locked_until_earned() {
    let m = Machine::boot();
    m.run("clear");
    m.run("vi note.txt");
    assert!(
        m.screen()
            .contains("vi is locked. You earn it by finishing a game: play vi-quest"),
        "{}",
        m.screen()
    );
    m.run("ls /bin | grep -c '^vi$'");
    assert!(m.screen().contains("\n0\n"), "{}", m.screen());
    m.run("man vi");
    assert!(m.screen().contains("earned in vi-quest"), "{}", m.screen());
    m.run("clear");
    m.run("games");
    let s = m.screen();
    assert!(s.contains("vi-quest") && s.contains("prison-escape"), "{s}");
}

#[test]
fn prison_escape_teaches_q_bang() {
    let m = Machine::boot();
    m.run("clear");
    m.run("play prison-escape");
    assert!(m.screen().contains("PRISON ESCAPE  Cell 1"), "{}", m.screen());
    // mash a few keys: hints arrive
    press(&m, "hjklhjkl");
    assert!(m.screen().contains("Escape gets you back to calm"), "{}", m.screen());
    press(&m, "\u{1b}:q\n");
    assert!(m.screen().contains("The door swings open."), "{}", m.screen());
    press(&m, " ");
    assert!(m.screen().contains("Cell 2"), "{}", m.screen());
    press(&m, ":q\n");
    assert!(
        m.screen()
            .contains("E37: No write since last change (add ! to override)"),
        "{}",
        m.screen()
    );
    press(&m, ":wq\n");
    assert!(m.screen().contains("E45"), "{}", m.screen());
    press(&m, ":q!\n");
    assert!(m.screen().contains("The door swings open."), "{}", m.screen());
    press(&m, " ");
    assert!(m.screen().contains("Cell 3"), "{}", m.screen());
    press(&m, ":wq\n");
    assert!(m.screen().contains("no note yet"), "{}", m.screen());
    press(&m, "ifree at last\u{1b}:q!\n");
    assert!(m.screen().contains("only opens for a saved note"), "{}", m.screen());
    press(&m, ":wq\n");
    assert!(m.screen().contains("The door swings open."), "{}", m.screen());
    press(&m, " ");
    let s = m.screen();
    assert!(s.contains("Free!") && s.contains("ESCAPED VI"), "{s}");
    assert!(s.ends_with("kid@kiddos:~$"), "{s}");
    assert_eq!(m.spoken().last().unwrap(), "You escaped from vi");
    m.run("badges");
    assert!(m.screen().contains("ESCAPED VI"), "{}", m.screen());
}

#[test]
fn vi_quest_unlocks_vi() {
    let m = Machine::boot();
    m.run("clear");
    m.run("play vi-quest");
    assert!(
        m.screen().contains("vi-quest  1/10  The land of hjkl"),
        "{}",
        m.screen()
    );
    // forbidden spell
    press(&m, "x");
    assert!(m.screen().contains("This land does not know 'x'"), "{}", m.screen());
    // walking into stone does nothing
    press(&m, "k");
    assert!(m.screen().contains("Stone."), "{}", m.screen());
    // 1: hjkl through the maze
    press(&m, "jjjjjjlllllkklllkkkkllllll");
    press(&m, "jjjjlllllllkkhhhkhklllllll");
    press(&m, "jjjjjj");
    assert!(m.screen().contains("hjkl. Your hands never leave"), "{}", m.screen());
    press(&m, " ");
    // 2: w
    assert!(m.screen().contains("2/10"), "{}", m.screen());
    press(&m, "wwwwwwwwww");
    assert!(m.screen().contains("w and b."), "{}", m.screen());
    press(&m, " ");
    // 3: 0 $
    press(&m, "jj$");
    assert!(m.screen().contains("0 and $."), "{}", m.screen());
    press(&m, " ");
    // 4: G
    press(&m, "G$");
    assert!(m.screen().contains("gg and G."), "{}", m.screen());
    press(&m, " ");
    // 5: x on boulders: go to the bottom row and clear the way
    press(&m, "jj");
    for _ in 0..40 {
        if m.screen().contains("x. The smallest spell") {
            break;
        }
        press(&m, "l");
        let (cx, cy) = m.kernel.screen.lock().cursor();
        if m.cell(cx, cy).ch == 'o' {
            press(&m, "x");
        }
    }
    assert!(m.screen().contains("x. The smallest spell"), "{}", m.screen());
    press(&m, " ");
    // 6: dd
    press(&m, "jddjdddd");
    assert!(m.screen().contains("dd. A whole line, gone."), "{}", m.screen());
    press(&m, " ");
    // 7: yy p p
    press(&m, "yypp");
    assert!(m.screen().contains("yy and p."), "{}", m.screen());
    press(&m, " ");
    // 8: search
    press(&m, "/X\n");
    assert!(m.screen().contains("/ finds anything"), "{}", m.screen());
    press(&m, " ");
    // 9: insert
    press(&m, "$amat\u{1b}");
    assert!(m.screen().contains("i, a and Esc."), "{}", m.screen());
    press(&m, " ");
    // 10: :wq
    assert!(m.screen().contains("10/10"), "{}", m.screen());
    press(&m, ":wq\n");
    assert!(m.screen().contains(":wq. You can always get out."), "{}", m.screen());
    press(&m, " ");
    let s = m.screen();
    assert!(
        s.contains("VI WIZARD") && s.contains("A new word appeared in /bin"),
        "{s}"
    );
    assert!(s.ends_with("kid@kiddos:~$"), "{s}");
    // vi works now, and stays unlocked
    m.run("clear");
    m.run("ls /bin | grep -c '^vi$'");
    assert!(m.screen().contains("\n1\n"), "{}", m.screen());
    m.run("cat ~/.unlocks");
    assert!(m.screen().contains("\nvi\n"), "{}", m.screen());
    m.run("vi note.txt");
    assert!(m.screen().contains("\"note.txt\" [New]"), "{}", m.screen());
    press(&m, "ihello from vi\nsecond line\u{1b}");
    press(&m, ":q\n");
    assert!(m.screen().contains("E37"), "{}", m.screen());
    press(&m, ":wq\n");
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
    m.run("cat note.txt");
    assert!(m.screen().contains("hello from vi\nsecond line\n"), "{}", m.screen());
    m.run("vi note.txt");
    press(&m, "ddx:q\n");
    assert!(m.screen().contains("E37"), "{}", m.screen());
    press(&m, ":q!\n");
    m.run("cat note.txt");
    assert!(m.screen().contains("hello from vi\nsecond line\n"), "{}", m.screen());
}
