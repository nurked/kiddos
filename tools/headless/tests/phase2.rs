//! Phase 2: BASIC.

use kiddos_headless::Machine;
use kiddos_kernel::Key;

#[test]
fn basic_runs_from_a_pipe_a_file_and_a_shebang() {
    let m = Machine::boot();
    m.run_script(
        "@clear
echo 'PRINT 6 * 7' | basic
@expect 42
@clear
echo 'FOR i = 1 TO 3' > loop.bas
echo 'PRINT \"line\"; i' >> loop.bas
echo 'NEXT' >> loop.bas
run loop.bas
@expect line 3
@clear
echo '#!/bin/basic' > hello.bas
echo 'PRINT \"hello from basic\"' >> hello.bas
chmod +x hello.bas
./hello.bas
@expect hello from basic
@clear
echo 'PRINT 1 +' > bad.bas
run bad.bas
@expect bad.bas:
",
    );
}

#[test]
fn basic_repl_kiddos_statements_and_exit() {
    let m = Machine::boot();
    m.run("clear");
    m.run("basic");
    assert!(m.screen().contains("KidDOS BASIC"), "{}", m.screen());
    assert!(m.screen().ends_with("\nReady"), "{}", m.screen());
    assert!(
        !m.screen().contains("[33m") && !m.screen().contains("[0m"),
        "{}",
        m.screen()
    );
    assert_eq!(m.cell(0, 4).fg, kiddos_console::colors::YELLOW, "{}", m.screen());
    m.run("PRINT 2 + 2");
    assert!(m.screen().contains("\n 4\n"), "{}", m.screen());
    m.run("CLS");
    m.run("PUT 10, 3, \"XY\", 14, 1");
    assert_eq!(m.cell(10, 3).ch, 'X', "{}", m.screen());
    assert_eq!(m.cell(11, 3).ch, 'Y');
    assert_eq!(m.cell(10, 3).fg, kiddos_console::colors::YELLOW);
    assert_eq!(m.cell(10, 3).bg, kiddos_console::colors::BLUE);
    m.run("SPEAK \"hello there\"");
    assert_eq!(m.spoken().last().unwrap(), "hello there");
    m.run("PRINT TICK > 0");
    assert!(m.screen().contains("TRUE"), "{}", m.screen());
    m.run("CLS");
    m.run("PRINT KEY$");
    m.key(Key::Up);
    assert!(m.screen().contains("\nUP\n"), "{}", m.screen());
    m.run("CLS");
    m.run("PRINT nosuch");
    assert!(
        m.screen().contains("Undefined") && m.screen().contains("spell"),
        "{}",
        m.screen()
    );
    m.run("exit");
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
}

#[test]
fn basic_save_load_and_ctrl_c() {
    let m = Machine::boot();
    m.run("clear");
    m.run("basic");
    m.run("PRINT \"saved program\"");
    // SAVE stores the *program* (what EDIT holds), so put a line in it via LOAD of a file
    m.run("exit");
    m.run("echo 'PRINT \"from disk\"' > disk.bas");
    m.run("basic");
    m.run("LOAD \"disk\"");
    m.run("RUN");
    assert!(m.screen().contains("from disk"), "{}", m.screen());
    m.run("SAVE \"copy\"");
    m.run("exit");
    m.run("cat copy.bas");
    assert!(m.screen().contains("PRINT \"from disk\""), "{}", m.screen());
    // a tight loop stops on Ctrl-C and returns to the shell prompt
    m.run("clear");
    m.run("echo 'WHILE TRUE' > spin.bas; echo 'WEND' >> spin.bas");
    m.kernel.push_text("run spin.bas");
    m.kernel.push_key(Key::Enter);
    std::thread::sleep(std::time::Duration::from_millis(150));
    assert!(m.kernel.processes().iter().any(|p| p.name == "run"), "{}", m.screen());
    m.kernel.push_key(Key::Ctrl('c'));
    m.settle();
    assert!(!m.kernel.processes().iter().any(|p| p.name == "run"));
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
}

#[test]
fn basic_cartridges_play() {
    let m = Machine::boot();
    m.run("games");
    let s = m.screen();
    for name in ["guess", "snake", "hangman", "typing"] {
        assert!(s.contains(name), "{s}");
    }
    // guess: binary search always finds it
    m.run("clear");
    m.run("play guess");
    assert!(m.screen().contains("GUESS THE NUMBER"), "{}", m.screen());
    let (mut lo, mut hi) = (1, 100);
    for _ in 0..8 {
        if m.screen().contains("YES!") {
            break;
        }
        let mid = (lo + hi) / 2;
        m.run(&mid.to_string());
        let s = m.screen();
        let last = s
            .lines()
            .rev()
            .find(|l| l.contains("Higher!") || l.contains("Lower!") || l.contains("YES!"));
        match last {
            Some(l) if l.contains("Higher!") => lo = mid + 1,
            Some(l) if l.contains("Lower!") => hi = mid - 1,
            _ => {}
        }
    }
    assert!(m.screen().contains("YES! You got it in"), "{}", m.screen());
    m.settle();
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
    m.run("badges");
    assert!(m.screen().contains("GUESSER"), "{}", m.screen());
    // snake: runs, draws the border, ESC ends it
    m.run("clear");
    m.kernel.push_text("play snake");
    m.kernel.push_key(Key::Enter);
    // the harness clock is virtual, so the snake runs into a wall at once
    std::thread::sleep(std::time::Duration::from_millis(300));
    m.settle();
    assert_eq!(m.cell(0, 1).ch, '#');
    assert!(m.screen().contains("GAME OVER"), "{}", m.screen());
    m.key(Key::Char(' '));
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
    // hangman: every letter of the alphabet wins eventually
    m.run("clear");
    m.run("play hangman");
    assert!(m.screen().contains("HANGMAN"), "{}", m.screen());
    for c in "eaiourntslcpdmhbgykvwxzjqf".chars() {
        if m.screen().contains("YOU WIN") || m.screen().contains("Oh no") {
            break;
        }
        m.key(Key::Char(c));
    }
    assert!(
        m.screen().contains("YOU WIN") || m.screen().contains("Oh no"),
        "{}",
        m.screen()
    );
    m.key(Key::Char(' '));
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
    // typing: type the ten lines
    m.run("clear");
    let beeps_before = m.host.beeps.lock().len();
    m.run("play typing");
    m.key(Key::Char(' '));
    for line in [
        "cat",
        "ls",
        "cd games",
        "pwd",
        "cat sign",
        "mkdir box",
        "echo hi",
        "cd ..",
        "man ls",
        "play snakX",
    ] {
        m.kernel.push_text(line);
        m.key(Key::Enter);
    }
    let s = m.screen();
    assert!(s.contains("Mistakes: 1"), "{s}");
    assert!(s.contains("words per minute"), "{s}");
    // good lines chime, bad lines stay silent
    let beeps: Vec<(u32, u32)> = m.host.beeps.lock()[beeps_before..].to_vec();
    assert!(!beeps.is_empty() && beeps.iter().all(|(f, _)| *f >= 660), "{beeps:?}");
    m.key(Key::Char(' '));
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
}

#[test]
fn tetris_and_sokoban_play() {
    let m = Machine::boot();
    // tetris: with a virtual clock the pieces fall at once; drop a few and quit
    m.run("clear");
    m.kernel.push_text("play tetris");
    m.kernel.push_key(Key::Enter);
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(m.cell(29, 2).ch, '|', "{}", m.screen());
    // with a virtual clock the game may already be over; either way it
    // must end cleanly and without a BASIC error
    m.kernel.push_key(Key::Escape);
    m.settle();
    let s = m.screen();
    assert!(!s.contains("tetris.bas:"), "{s}");
    if s.contains("GAME OVER") {
        m.key(Key::Char(' '));
    }
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
    // sokoban: level 1 is three pushes to the right
    m.run("clear");
    m.run("play sokoban");
    assert!(m.screen().contains("SOKOBAN   level 1 of 3"), "{}", m.screen());
    for _ in 0..3 {
        m.key(Key::Right);
    }
    assert!(m.screen().contains("LEVEL DONE in 3 moves"), "{}", m.screen());
    m.key(Key::Char(' '));
    assert!(m.screen().contains("level 2 of 3"), "{}", m.screen());
    m.key(Key::Escape);
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
}
