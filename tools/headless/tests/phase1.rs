//! Phase 1: the tutor, edit, and the adventure cartridge.

use kiddos_headless::Machine;

#[test]
fn tutor_walks_through_lesson_one() {
    let m = Machine::boot();
    // silent until the kid says hi
    assert!(!m.screen().contains("\u{263A}"), "{}", m.screen());
    m.run("hi");
    m.run("Sam");
    let s = m.screen();
    assert!(s.contains("That's the machine saying hello back."), "{s}");
    assert!(s.contains("Now type help."), "{s}");
    // three misses earn a hint
    m.run("clear");
    m.run("ls");
    m.run("ls");
    m.run("ls");
    assert!(m.screen().contains("Hint: Type: help"), "{}", m.screen());
    m.run("help");
    m.feed("q");
    m.settle();
    m.run("clear");
    m.run("sleep 1"); // speech is rate limited; let the virtual clock move
    m.run("fortune");
    let s = m.screen();
    assert!(s.contains("HELLO!"), "{s}");
    assert!(s.contains("Lesson 1 done! Next: Where am I?"), "{s}");
    m.run("clear");
    m.run("progress");
    let s = m.screen();
    assert!(s.contains("\u{2713}  1. Hello"), "{s}");
    assert!(s.contains(">  2. Where am I?  (step 1 of 5)"), "{s}");
    m.run("clear");
    m.run("cat ~/.progress");
    let s = m.screen();
    assert!(
        s.contains("current = \"02-where-am-i\"") && s.contains("completed = [\"01-hello\"]"),
        "{s}"
    );
    m.run("clear");
    m.run("badges");
    assert!(m.screen().contains("> hi_"), "{}", m.screen());
    assert!(m.spoken().iter().any(|s| s == "Well done!"));
}

#[test]
fn tutor_checks_machine_state_and_can_be_silenced() {
    let m = Machine::boot();
    m.run("lesson 2");
    assert!(m.screen().contains("Lesson 2 of 12: Where am I?"), "{}", m.screen());
    m.run("pwd");
    m.run("ls");
    m.run("clear");
    // wrong folder does not count even though it is a cd
    m.run("cd /tmp");
    assert!(!m.screen().contains("Look at the prompt"), "{}", m.screen());
    m.run("cd /games");
    assert!(
        m.screen().contains("Look at the prompt: it now says /games."),
        "{}",
        m.screen()
    );
    m.run("tutor off");
    m.run("clear");
    m.run("cd ..");
    assert!(!m.screen().contains("\u{263A}"), "{}", m.screen());
    m.run("hint");
    assert!(m.screen().contains("Hint: Two dots"), "{}", m.screen());
    m.run("tutor list");
    assert!(m.screen().contains("12. Make a game"), "{}", m.screen());
}

#[test]
fn tutor_is_quiet_inside_a_game_and_stops_nagging() {
    let m = Machine::boot();
    m.run("lesson 4"); // step 1 wants: mkdir box
    m.run("clear");
    m.run("play adventure");
    m.run("cd ~/cave");
    m.run("clear");
    for _ in 0..6 {
        m.run("cat sign");
    }
    assert!(!m.screen().contains("Hint:"), "{}", m.screen());
    m.run("cd tunnel");
    m.run("ls");
    m.run("ls");
    assert!(!m.screen().contains("\u{263A}"), "{}", m.screen());
    // back outside: hints resume, but each hint is offered only once
    m.run("cd");
    m.run("clear");
    for _ in 0..3 {
        m.run("ls");
    }
    assert!(m.screen().contains("Hint: Type: mkdir box"), "{}", m.screen());
    m.run("clear");
    for _ in 0..6 {
        m.run("ls");
    }
    assert!(!m.screen().contains("Hint:"), "{}", m.screen());
    m.run("hint");
    assert!(m.screen().contains("Hint: Type: mkdir box"), "{}", m.screen());
}

#[test]
fn adventure_can_be_played_to_the_end() {
    let m = Machine::boot();
    m.run("games");
    assert!(m.screen().contains("adventure    The Drive Below"), "{}", m.screen());
    m.run("clear");
    m.run("play adventure");
    assert!(m.screen().contains("T H E   D R I V E   B E L O W"), "{}", m.screen());
    assert_eq!(m.spoken().last().unwrap(), "Welcome to the drive below");
    m.run_script(
        "@clear
cd ~/cave
cat sign
@expect mv torch tunnel
mv torch tunnel
cd tunnel
ls
@expect torch
cat pit/warning
cd pit
ls -a
@expect .note
cat .note
@expect hidden note
cd ..
cd hall
@clear
grep key book
@expect The key is under the statue
cd statue
mv key ../door
cd ../door
@clear
./open
@expect CLUNK
cd treasury
echo Sam > wall
cat wall
@expect Sam
./finish
@expect not runnable yet
chmod +x finish
@clear
./finish
@expect EXPLORER OF THE DRIVE
@expect A badge is saved in ~/badges
",
    );
    m.run("clear");
    m.run("badges");
    assert!(m.screen().contains("EXPLORER OF THE DRIVE"), "{}", m.screen());
    // locked door message when the key is not there
    m.run_script(
        "rm -r ~/cave
play adventure
cd ~/cave/tunnel/hall/door
@clear
./open
@expect The door stays shut
",
    );
}
