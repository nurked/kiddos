//! Phase 6: ARM assembly, the debugger, bug-hunt.

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
fn hello_assembles_runs_and_disassembles() {
    let m = Machine::boot();
    m.run("cp /usr/share/examples/hello.s .");
    m.run("clear");
    m.run("as hello.s");
    assert!(m.screen().contains("OK hello (8 instructions"), "{}", m.screen());
    m.run("./hello");
    assert!(m.screen().contains("Hello from ARM!"), "{}", m.screen());
    m.run("clear");
    m.run("dis hello");
    let s = m.screen();
    assert!(s.contains("_start:"), "{s}");
    assert!(s.contains("mov x0, #1"), "{s}");
    assert!(s.contains("adr x1, msg"), "{s}");
    assert!(s.contains("svc #0"), "{s}");
    assert!(s.contains("msg:"), "{s}");
    assert!(s.contains("|Hello from ARM!.|"), "{s}");
    m.run("clear");
    m.run("hexdump -n 16 hello");
    let s = m.screen();
    assert!(s.contains("00000000  00 61 72 6d 01 00 00 00"), "{s}");
    assert!(s.contains("|.arm"), "{s}");
    m.run("ls -l hello | cut -c1-10");
    assert!(m.screen().contains("-rwxr-xr-x"), "{}", m.screen());
}

#[test]
fn count_and_echo_examples() {
    let m = Machine::boot();
    m.run("cp /usr/share/examples/count.s /usr/share/examples/echo.s .");
    m.run("as count.s && as echo.s");
    m.run("clear");
    m.run("./count");
    let s = m.screen();
    for n in 1..=10 {
        assert!(s.contains(&format!("\n{n}\n")), "missing {n} in {s}");
    }
    m.run("clear");
    m.kernel.push_text("./echo\n");
    m.settle();
    assert!(m.screen().contains("Say something:"), "{}", m.screen());
    press(&m, "banana\n");
    assert!(m.screen().contains("You said: banana"), "{}", m.screen());
}

#[test]
fn assembler_errors_name_the_line() {
    let m = Machine::boot();
    m.run("echo -e 'mov x0, #1\\nmvo x1, #2' > bad.s");
    m.run("clear");
    m.run("as bad.s");
    let s = m.screen();
    assert!(s.contains("bad.s: line 2: I don't know the instruction 'mvo'"), "{s}");
    assert!(s.contains("Did you mean m"), "{s}");
    assert!(s.contains("   2 | mvo x1, #2"), "{s}");
    m.run("echo 'b nowhere' > bad2.s; as bad2.s");
    assert!(m.screen().contains("not seen a label called nowhere"), "{}", m.screen());
}

#[test]
fn faults_are_explained_with_the_line() {
    let m = Machine::boot();
    m.run("echo -e 'mov x1, #0\\nldr x0, [x1]' > crash.s");
    m.run("as crash.s");
    m.run("clear");
    m.run("./crash");
    let s = m.screen();
    assert!(
        s.contains("crash: The program read from address 0x0 - there is nothing there"),
        "{s}"
    );
    assert!(s.contains("(line 2)"), "{s}");
    // running off the end
    m.run("echo 'mov x0, #1' > off.s; as off.s; clear; ./off");
    assert!(m.screen().contains("where there are no instructions"), "{}", m.screen());
    // an unknown system call
    m.run("echo -e 'mov x8, #4242\\nsvc #0' > sys.s; as sys.s; clear; ./sys");
    assert!(m.screen().contains("System call 4242 does not exist"), "{}", m.screen());
}

#[test]
fn ctrl_c_stops_a_loop() {
    let m = Machine::boot();
    m.run("echo 'spin: b spin' > spin.s; as spin.s");
    m.run("clear");
    m.kernel.push_text("./spin\n");
    m.settle();
    m.key(Key::Ctrl('c'));
    m.settle();
    m.run("echo back");
    assert!(m.screen().contains("\nback\n"), "{}", m.screen());
}

#[test]
fn debugger_steps_and_shows_registers() {
    let m = Machine::boot();
    m.run("cp /usr/share/examples/hello.s .; as hello.s");
    m.run("clear");
    m.kernel.push_text("debug hello\n");
    m.settle();
    let s = m.screen();
    assert!(s.contains("hello"), "{s}");
    assert!(s.contains("x0      0"), "{s}");
    assert!(s.contains("> 13     mov x0, #1"), "{s}");
    assert!(s.contains("mem msg"), "{s}");
    assert!(s.contains("Hello fr") && s.contains("om ARM!."), "{s}"); // the memory window shows the text
    press(&m, "s");
    let s = m.screen();
    assert!(s.contains("x0      1"), "{s}");
    assert!(s.contains("mov x0, #1  ->  x0 = 1"), "{s}");
    assert!(s.contains("> 14     adr x1, msg"), "{s}");
    press(&m, "s");
    assert!(m.screen().contains("x1      0x10020 msg"), "{}", m.screen());
    // run to the end: the output pane shows the program's text
    press(&m, "c");
    let s = m.screen();
    assert!(s.contains("output  Hello from ARM!"), "{s}");
    assert!(s.contains("finished with exit code 0"), "{s}");
    // a breakpoint after restart
    press(&m, "r");
    press(&m, ":break 16\n");
    assert!(m.screen().contains("Breakpoint at line 16"), "{}", m.screen());
    press(&m, "c");
    let s = m.screen();
    assert!(s.contains("Breakpoint at line 16, after 3 instructions"), "{s}");
    assert!(s.contains("x2      16"), "{s}");
    press(&m, ":mem sp\n");
    assert!(m.screen().contains("Memory window at 0x100000"), "{}", m.screen());
    press(&m, "q");
    m.run("echo back");
    assert!(m.screen().contains("\nback\n"), "{}", m.screen());
}

#[test]
fn debugger_takes_a_source_file_and_shows_a_crash() {
    let m = Machine::boot();
    m.run("echo -e 'mov x1, #0\\nldr x0, [x1]\\nret' > crash.s");
    m.run("clear");
    m.kernel.push_text("debug crash.s\n");
    m.settle();
    press(&m, "c");
    let s = m.screen();
    assert!(s.contains("read from address 0x0"), "{s}");
    assert!(s.contains("crashed at"), "{s}");
    press(&m, "q");
}

const FIXES: [(&str, &str, &str); 8] = [
    ("01-hello.s", "mov x2, #5 ", "mov x2, #7 "),
    ("02-add.s", "add x0, x1, x1", "add x0, x1, x2"),
    ("03-count.s", "b.lt loop", "b.le loop"),
    ("04-ret.s", "    svc #0\n\nbye:", "    svc #0\n    ret\n\nbye:"),
    ("05-order.s", "    ldrb w0, [x1]           // take the number out of the box\n    mov x2, #7\n    strb w2, [x1]           // put 7 into the box\n", "    mov x2, #7\n    strb w2, [x1]\n    ldrb w0, [x1]\n"),
    ("06-loop.s", "sub x19, x19, #1", "add x19, x19, #1"),
    ("07-strlen.s", "ldr x2, [x1]", "ldrb w2, [x1]"),
    ("08-sign.s", "b.hi positive", "b.gt positive"),
];

fn fix(m: &Machine, file: &str, from: &str, to: &str) {
    let path = format!("/home/kid/bug-hunt/{file}");
    let src = m.vfs_read_string(&path);
    assert!(src.contains(from), "{file} does not contain {from:?}:\n{src}");
    m.vfs_write(&path, src.replacen(from, to, 1).as_bytes());
}

#[test]
fn bug_hunt_reports_each_bug_and_rewards_all_eight() {
    let m = Machine::boot();
    m.run("clear");
    m.run("play bug-hunt");
    let s = m.screen();
    assert!(s.contains("8 programs copied in ~/bug-hunt"), "{s}");
    assert!(
        s.contains("BUG 01-hello.s   expected \"Hello!\\n\", got \"Hello\""),
        "{s}"
    );
    assert!(s.contains("BUG 02-add.s     expected \"5\\n\", got \"4\\n\""), "{s}");
    assert!(
        s.contains("BUG 03-count.s   expected \"12345\\n\", got \"1234\\n\""),
        "{s}"
    );
    assert!(
        s.contains("BUG 04-ret.s     expected \"Hi!Hi!Bye\\n\", got \"Hi!Bye\\n\""),
        "{s}"
    );
    assert!(s.contains("BUG 05-order.s   expected \"7\\n\", got \"0\\n\""), "{s}");
    assert!(s.contains("BUG 06-loop.s    never finishes"), "{s}");
    assert!(s.contains("BUG 07-strlen.s  expected \"5\\n\", got"), "{s}");
    assert!(
        s.contains("BUG 08-sign.s    expected \"negative\\n\", got \"positive\\n\""),
        "{s}"
    );
    assert!(s.contains("0 of 8 fixed."), "{s}");
    assert!(s.contains("Next: ~/bug-hunt/01-hello.s"), "{s}");
    m.run("clear");
    m.run("play bug-hunt hint");
    assert!(m.screen().contains("Hint: How many bytes"), "{}", m.screen());
    // fix them one by one: each fix turns exactly its own line green
    for (i, (file, from, to)) in FIXES.iter().enumerate() {
        fix(&m, file, from, to);
        m.run("clear");
        m.run("play bug-hunt");
        let s = m.screen();
        if i < 7 {
            assert!(s.contains(&format!("{} of 8 fixed.", i + 1)), "after {file}: {s}");
            assert!(s.contains(&format!("OK  {file}")), "after {file}: {s}");
        }
    }
    let s = m.screen();
    assert!(s.contains("All eight fixed."), "{s}");
    assert!(s.contains("BUG HUNTER"), "{s}");
    m.run("clear");
    m.run("badges");
    assert!(m.screen().contains("BUG HUNTER"), "{}", m.screen());
    // reset brings the bugs back
    m.run("clear");
    m.run("play bug-hunt reset");
    assert!(m.screen().contains("0 of 8 fixed."), "{}", m.screen());
}

#[test]
fn man_pages_and_help_know_the_new_commands() {
    let m = Machine::boot();
    for page in [
        "as",
        "debug",
        "dis",
        "hexdump",
        "syscalls",
        "registers",
        "asm",
        "bug-hunt",
    ] {
        m.run("clear");
        m.run(&format!("man {page} | cat"));
        assert!(!m.screen().contains("No manual entry"), "man {page}: {}", m.screen());
    }
    m.run("clear");
    m.run("help programs | cat");
    let s = m.screen();
    for c in ["as", "debug", "dis"] {
        assert!(
            s.contains(&format!("\n  {c}")) || s.contains(&format!(" {c} ")),
            "help: {c} missing in {s}"
        );
    }
    m.run("clear");
    m.run("games");
    assert!(m.screen().contains("bug-hunt"), "{}", m.screen());
}
