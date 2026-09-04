//! Phase 0 exit criteria, as keystroke scripts against the factory drive:
//! a kid can boot, explore, make files, read man pages, get lost, get found.

use kiddos_headless::Machine;
use kiddos_kernel::Key;

#[test]
fn boot_banner_and_motd() {
    let m = Machine::boot();
    let s = m.screen();
    assert!(s.contains("KidDOS 0.3.0"), "{s}");
    assert!(s.contains("Welcome to your computer."), "{s}");
    assert!(s.contains("Type hi and press Enter."), "{s}");
    assert!(s.ends_with("kid@kiddos:~$"), "{s}");
}

#[test]
fn navigation_get_lost_get_found() {
    let m = Machine::boot();
    m.run_script(
        "@clear
ls
@expect welcome.txt
@expect games
@expect bin
cd /games
pwd
@expect /games
cd ..
pwd
@expect kid@kiddos:/$
cd
cd nowhere
@expect I can't find ~/nowhere.
cd /etc/motd
@expect /etc/motd is not a folder.
cd
pwd
@expect /home/kid
cd -
@clear
pwd
@expect /home/kid
",
    );
}

#[test]
fn files_and_redirects() {
    let m = Machine::boot();
    m.run_script(
        "@clear
echo hello > note.txt
cat note.txt
@expect hello
echo more >> note.txt
wc -l note.txt
@expect 2 note.txt
mkdir box
rm box
@expect ~/box is a folder. To remove a folder and everything in it, use rm -r ~/box.
rm -r box
ls box
@expect I can't find ~/box.
touch a.txt b.txt c.md
echo *.txt
@expect a.txt b.txt
cp note.txt copy.txt
mv copy.txt moved.txt
cat moved.txt | head -1
@expect hello
rm
@expect rm is forever.
",
    );
}

#[test]
fn pipes_and_broken_pipe() {
    let m = Machine::boot();
    m.run_script(
        "@clear
seq 5 | sort -r | head -1
@expect 5
ls /bin | grep -c '^ls$'
@expect 1
yes | head -2
@expect y
echo hello | tr a-z A-Z
@expect HELLO
cat < welcome.txt | head -1
@expect Hi!
echo sec\"\"ret > /dev/null
@absent secret
seq 3 | wc -l > count.txt; cat count.txt
@expect 3
",
    );
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
}

#[test]
fn man_and_help() {
    let m = Machine::boot();
    m.run_script(
        "@clear
man mkdir
@expect MKDIR
@expect TRY THIS
@expect make a new folder
@clear
man nothing
@expect I have no manual page for nothing.
man -k folder
@expect mkdir
@clear
help files
@expect Working with files and folders
@expect ls
@clear
help
@expect Here is what I know.
",
    );
    // `help` is longer than a screen: the pager prompt is on the last row
    assert!(m.screen().contains("-- more --"), "{}", m.screen());
    m.key(Key::Char('q'));
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
}

#[test]
fn unknown_commands_and_suggestions() {
    let m = Machine::boot();
    m.run_script(
        "@clear
foo
@expect I don't know \"foo\". Try help, or man -k foo.
@absent Did you mean
lss
@expect Did you mean ls?
mkdri x
@expect Did you mean mkdir?
",
    );
}

#[test]
fn permissions_and_scripts() {
    let m = Machine::boot();
    m.run_script(
        "@clear
rm /etc/motd
@expect /etc/motd belongs to the machine.
echo x > /games/x
@expect /games/x belongs to the machine.
echo 'echo hi from script' > s.sh
./s.sh
@expect ~/s.sh is not runnable yet. Make it runnable with: chmod +x ~/s.sh
chmod +x s.sh
./s.sh
@expect hi from script
ls -l s.sh
@expect -rwxr-xr-x kid
echo 'echo arg: $1 of $#' > a.sh; chmod +x a.sh; ./a.sh boo
@expect arg: boo of 1
hello
@expect Hello from your first script!
",
    );
}

#[test]
fn variables_and_status() {
    let m = Machine::boot();
    m.run_script(
        "@clear
export PET=cat
echo my $PET
@expect my cat
X=5; echo $X
@expect 5
false; echo $?
@expect 1
true && echo yes
@expect yes
false || echo fallback
@expect fallback
Y=nope
@clear
false && echo $Y
@absent nope
echo \"quoted $PET\" 'literal $PET'
@expect quoted cat literal $PET
",
    );
}

#[test]
fn tab_completion_and_history() {
    let m = Machine::boot();
    m.run("clear");
    m.feed("cat wel{tab}");
    m.settle();
    assert_eq!(m.line(0), "kid@kiddos:~$ cat welcome.txt");
    m.key(Key::Enter);
    assert!(m.screen().contains("This is your computer."), "{}", m.screen());
    m.run("clear");
    m.feed("ec{tab}");
    m.settle();
    assert_eq!(m.line(0), "kid@kiddos:~$ echo");
    m.feed("one{enter}");
    m.settle();
    m.run("echo two");
    m.feed("{up}");
    m.settle();
    assert!(m.screen().ends_with("kid@kiddos:~$ echo two"), "{:?}", m.screen());
    m.feed("{up}");
    m.settle();
    assert!(m.screen().ends_with("kid@kiddos:~$ echo one"), "{}", m.screen());
    m.feed("{down}{down}");
    m.settle();
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
    m.run("clear");
    m.run("history");
    let s = m.screen();
    assert!(s.contains("echo one") && s.contains("echo two"), "{s}");
    m.run("clear");
    m.run("!hist");
    let s = m.screen();
    assert!(
        s.contains("kid@kiddos:~$ !hist\nhistory\n") && s.contains("echo one"),
        "{s}"
    );
    m.run("clear");
    m.run("echo again");
    m.run("!!");
    assert!(
        m.screen().contains("kid@kiddos:~$ !!\necho again\nagain"),
        "{:?}",
        m.screen()
    );
    // line editing: Home, insert, End
    m.run("clear");
    m.feed("cho hi{home}e{end} there{enter}");
    m.settle();
    assert!(m.screen().contains("\nhi there"), "{}", m.screen());
}

#[test]
fn ctrl_c_stops_a_program() {
    let m = Machine::boot();
    m.run("clear");
    // `hi` waits for a name: a program that is "stuck" on the keyboard
    m.run("hi");
    assert!(m.kernel.processes().iter().any(|p| p.name == "hi"));
    m.kernel.push_key(Key::Ctrl('c'));
    m.settle();
    assert!(!m.kernel.processes().iter().any(|p| p.name == "hi"));
    assert!(m.screen().contains("^C"), "{}", m.screen());
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
    // Ctrl-C on an empty prompt just gives a new prompt
    m.kernel.push_key(Key::Ctrl('c'));
    m.settle();
    assert!(m.screen().ends_with("^C\nkid@kiddos:~$"), "{}", m.screen());
}

#[test]
fn the_machine_talks() {
    let m = Machine::boot();
    m.run("echo hello there > /dev/speaker");
    assert_eq!(m.spoken(), vec!["hello there"]);
    m.run("sleep 1");
    m.run("speak good night");
    assert_eq!(m.spoken().last().unwrap(), "good night");
    m.run("hi");
    assert!(m.screen().contains("What's your name?"), "{}", m.screen());
    m.run("Sam");
    assert!(m.screen().contains("Nice to meet you, Sam."), "{}", m.screen());
    m.run("echo $NAME");
    assert!(m.screen().ends_with("Sam\nkid@kiddos:~$"), "{}", m.screen());
    assert_eq!(m.host.config.lock().as_ref().unwrap().kid_name, "Sam");
}

#[test]
fn parent_mode() {
    let m = Machine::boot();
    m.run_script(
        "@clear
shutdown
@expect Only a parent can do that. Type parent.
parent
@expect No parent password yet.
secret
secret
@expect Parent mode.
whoami
@expect root
",
    );
    assert!(m.screen().ends_with("root@kiddos:~#"), "{}", m.screen());
    m.run_script(
        "exit
@expect Bye!
parent
@expect Parent password:
wrong
@expect Wrong password.
",
    );
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
    assert!(m
        .host
        .log_lines
        .lock()
        .iter()
        .any(|l| l.contains("wrong parent password")));
}

#[test]
fn unicode_input_and_language_report() {
    let m = Machine::boot();
    m.run_script(
        "@clear
lang
@expect en — English
echo привет мир > п.txt; cat п.txt
@expect привет мир
lang ru
@expect I only speak
",
    );
}

#[test]
fn tree_find_du() {
    let m = Machine::boot();
    m.run_script(
        "@clear
mkdir -p a/b; echo 12345 > a/b/f
tree a
@expect └── f
find . -name f
@expect ./a/b/f
find / -type d -name games
@expect /games
du a
@expect 6  a
",
    );
}

#[test]
fn drive_survives_save_and_load() {
    let m = Machine::boot();
    m.run("echo keep me > keep.txt");
    let dir = std::env::temp_dir().join(format!("kiddos-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("drive.kdd");
    m.kernel.save_drive(&file).unwrap();
    let vfs = kiddos_kernel::Vfs::load(&file).unwrap();
    assert_eq!(
        vfs.read_string("/home/kid/keep.txt", &kiddos_kernel::Actor::user("kid"))
            .unwrap(),
        "keep me\n"
    );
    assert!(vfs.exists("/usr/share/man/en/ls.md"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_writes_a_file() {
    let m = Machine::boot();
    m.run("edit story.txt");
    assert!(m.line(0).contains("edit: story.txt"), "{}", m.screen());
    m.feed("Once upon a time{enter}there was a kid.{ctrl-s}");
    m.settle();
    assert!(m.screen().contains("Saved 2 lines to story.txt."), "{}", m.screen());
    m.feed("{ctrl-q}");
    m.settle();
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
    m.run("cat story.txt");
    assert!(
        m.screen().contains("Once upon a time\nthere was a kid.\n"),
        "{}",
        m.screen()
    );
    // unsaved changes ask before quitting; Ctrl-C does not kill the editor
    m.run("edit story.txt");
    m.feed("{end}!{ctrl-c}");
    m.settle();
    assert!(m.screen().contains("Save changes?"), "{}", m.screen());
    m.feed("y");
    m.settle();
    m.run("head -1 story.txt");
    assert!(m.screen().contains("Once upon a time!"), "{}", m.screen());
}
