//! Phase 5: pixel mode. 320x200, 256 colors, from BASIC and from WASM.

use kiddos_headless::Machine;
use kiddos_kernel::{Actor, Key};

fn install(m: &Machine, name: &str, wat_src: &str) {
    let bytes = wat::parse_str(wat_src).expect("valid wat");
    let path = format!("/home/kid/{name}");
    let mut vfs = m.kernel.vfs.lock();
    vfs.write(&path, &bytes, &Actor::user("kid")).unwrap();
    vfs.chmod(&path, 0o755, &Actor::user("kid")).unwrap();
}

fn write(m: &Machine, name: &str, text: &str) {
    let path = format!("/home/kid/{name}");
    m.kernel
        .vfs
        .lock()
        .write(&path, text.as_bytes(), &Actor::user("kid"))
        .unwrap();
}

fn front(m: &Machine, x: usize, y: usize) -> u8 {
    m.pixels().expect("pixel mode").front()[y * 320 + x]
}

#[test]
fn basic_draws_and_the_picture_waits_for_a_key() {
    let m = Machine::boot();
    m.run("clear");
    write(
        &m,
        "circle.bas",
        "COLOR 14\nGFX_CIRCLEF 160, 100, 20\nCOLOR 4\nGFX_LINE 0, 0, 319, 0\nGFX_RECTF 10, 10, 12, 12\n",
    );
    m.run("basic circle.bas");
    // the program ended in pixel mode: the picture is up, waiting for a key
    assert!(m.kernel.screen.lock().pixel_mode());
    assert_eq!(front(&m, 160, 100), 14);
    assert_eq!(front(&m, 100, 0), 4);
    assert_eq!(front(&m, 11, 11), 4);
    assert_eq!(front(&m, 13, 13), 0);
    assert_eq!(front(&m, 200, 150), 0);
    m.key(Key::Char(' '));
    assert!(!m.kernel.screen.lock().pixel_mode());
    assert!(m.pixels().is_none());
    // and the shell is back with its prompt intact
    assert!(m.screen().contains("basic circle.bas"), "{}", m.screen());
}

#[test]
fn basic_screen_zero_returns_to_text_without_waiting() {
    let m = Machine::boot();
    m.run("clear");
    write(
        &m,
        "flash.bas",
        "SCREEN 13\nCOLOR 2\nGFX_PIXEL 5, 5\nSCREEN 0\nPRINT \"done\"\n",
    );
    m.run("basic flash.bas");
    assert!(!m.kernel.screen.lock().pixel_mode());
    assert!(m.screen().contains("done"), "{}", m.screen());
}

#[test]
fn basic_sync_off_buffers_until_flip() {
    let m = Machine::boot();
    m.run("clear");
    write(
        &m,
        "buf.bas",
        "GFX_SYNC FALSE\nCOLOR 9\nGFX_RECTF 0, 0, 319, 199\nPRINT GFX_GET(50, 50)\nK$ = KEY\nGFX_FLIP\nK$ = KEY\n",
    );
    m.run("basic buf.bas");
    // drawn but not flipped: the visible buffer is still black, GFX_GET saw it
    assert_eq!(front(&m, 50, 50), 0);
    m.key(Key::Char('a'));
    assert_eq!(front(&m, 50, 50), 9);
    m.key(Key::Char('a')); // second KEY
    m.key(Key::Char('a')); // the end-of-program wait
    assert!(!m.kernel.screen.lock().pixel_mode());
    assert!(
        m.screen().contains("\n9\n") || m.screen().contains("9\n"),
        "{}",
        m.screen()
    );
}

#[test]
fn basic_palette_text_and_keydown() {
    let m = Machine::boot();
    m.run("clear");
    write(
        &m,
        "pal.bas",
        "PALETTE 100, 255, 0, 255\nCOLOR 100\nGFX_TEXT 0, 0, \"HI\"\nWHILE NOT KEYDOWN(\"RIGHT\")\nSLEEP 0.01\nWEND\nPRINT \"held\"\nSCREEN 0\n",
    );
    m.kernel.push_text("basic pal.bas\n");
    // the program spins until the right arrow is held
    std::thread::sleep(std::time::Duration::from_millis(200));
    let px = m.pixels().expect("pixel mode");
    assert_eq!(px.palette()[100], [255, 0, 255]);
    let lit = (0..16)
        .flat_map(|x| (0..8).map(move |y| (x, y)))
        .filter(|&(x, y)| px.front()[y * 320 + x] == 100)
        .count();
    assert!(lit > 10, "text drawn in color 100: {lit} pixels");
    m.key_down(Key::Right);
    m.settle();
    assert!(!m.kernel.screen.lock().pixel_mode());
    assert!(m.screen().contains("held"), "{}", m.screen());
    m.key_up(Key::Right);
    assert!(!m.kernel.key_held(Key::Right));
}

#[test]
fn basic_break_in_pixel_mode_returns_to_text_at_once() {
    let m = Machine::boot();
    m.run("clear");
    write(&m, "spin.bas", "GFX_PIXEL 1, 1\nWHILE TRUE\nWEND\n");
    m.kernel.push_text("basic spin.bas\n");
    std::thread::sleep(std::time::Duration::from_millis(150));
    assert!(m.kernel.screen.lock().pixel_mode());
    m.key(Key::Ctrl('c'));
    m.settle();
    assert!(!m.kernel.screen.lock().pixel_mode());
    assert!(m.screen().contains("kid@"), "{}", m.screen());
}

/// Fills the screen with color 200, writes a pixel, then waits for one
/// key and reports through `key_down` whether Left is held at that moment.
const DRAW: &str = r#"(module
  (import "kiddos" "gfx_mode" (func $mode (param i32)))
  (import "kiddos" "gfx_fill" (func $fill (param i32 i32 i32 i32 i32)))
  (import "kiddos" "gfx_pixel" (func $pixel (param i32 i32 i32)))
  (import "kiddos" "gfx_text" (func $text (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "kiddos" "gfx_blit" (func $blit (param i32 i32 i32 i32 i32 i32)))
  (import "kiddos" "gfx_get" (func $get (param i32 i32) (result i32)))
  (import "kiddos" "gfx_flip" (func $flip))
  (import "kiddos" "readkey" (func $readkey (result i32)))
  (import "kiddos" "key_down" (func $key_down (param i32) (result i32)))
  (import "kiddos" "print" (func $print (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 8) "Hi")
  (data (i32.const 16) "\01\02\03\04")
  (data (i32.const 32) "held\n")
  (data (i32.const 48) "free\n")
  (func (export "main") (result i32)
    (call $mode (i32.const 1))
    (call $fill (i32.const 0) (i32.const 0) (i32.const 320) (i32.const 200) (i32.const 200))
    (call $pixel (i32.const 3) (i32.const 3) (i32.const 15))
    (drop (call $text (i32.const 100) (i32.const 100) (i32.const 8) (i32.const 2) (i32.const 15) (i32.const -1)))
    ;; a 2x2 sprite with color 1 transparent, at (10, 10)
    (call $blit (i32.const 10) (i32.const 10) (i32.const 2) (i32.const 2) (i32.const 16) (i32.const 1))
    (call $flip)
    (drop (call $readkey))
    (if (call $key_down (i32.const 0x110007))
      (then (call $print (i32.const 32) (i32.const 5)))
      (else (call $print (i32.const 48) (i32.const 5))))
    (call $get (i32.const 3) (i32.const 3))))"#;

#[test]
fn wasm_draws_blits_and_sees_held_keys() {
    let m = Machine::boot();
    install(&m, "draw.wasm", DRAW);
    m.run("clear");
    m.run("./draw.wasm; echo status $?");
    assert!(m.kernel.screen.lock().pixel_mode());
    assert_eq!(front(&m, 0, 0), 200);
    assert_eq!(front(&m, 3, 3), 15);
    assert_eq!(front(&m, 10, 10), 200, "transparent pixel skipped");
    assert_eq!(front(&m, 11, 10), 2);
    assert_eq!(front(&m, 10, 11), 3);
    assert_eq!(front(&m, 11, 11), 4);
    let white = (0..16)
        .flat_map(|x| (0..8).map(move |y| (x, y)))
        .filter(|&(x, y)| front(&m, 100 + x, 100 + y) == 15)
        .count();
    assert!(white > 10, "{white}");
    // hold Left, then press Enter: the program sees Left held and exits
    // with the pixel's color; pixel mode ends with the process
    m.kernel.push_key_event(Key::Left, true);
    m.key(Key::Enter);
    assert!(!m.kernel.screen.lock().pixel_mode());
    let s = m.screen();
    assert!(s.contains("held") && s.contains("status 15"), "{s}");
    m.kernel.push_key_event(Key::Left, false);
}

const EVENTS: &str = r#"(module
  (import "kiddos" "key_event" (func $ev (result i32)))
  (import "kiddos" "gfx_mode" (func $mode (param i32)))
  (import "kiddos" "readkey" (func $readkey (result i32)))
  (memory (export "memory") 1)
  (func (export "main") (result i32)
    (local $a i32) (local $b i32)
    (call $mode (i32.const 1))
    (drop (call $readkey))
    (local.set $a (call $ev))
    (local.set $b (call $ev))
    ;; down event is the plain code, up event has bit 24 set; none is -1
    (if (i32.ne (local.get $a) (i32.const 0x61)) (then (return (i32.const 1))))
    (if (i32.ne (local.get $b) (i32.const 0x1000061)) (then (return (i32.const 2))))
    (if (i32.ne (call $ev) (i32.const -1)) (then (return (i32.const 3))))
    (i32.const 0)))"#;

#[test]
fn wasm_key_events_come_in_order() {
    let m = Machine::boot();
    install(&m, "ev.wasm", EVENTS);
    m.run("clear");
    // an event from before pixel mode began is not seen: entering clears
    // the queue so a game does not replay the shell's typing
    m.kernel.push_key_event(Key::Char('z'), true);
    m.kernel.push_key_event(Key::Char('z'), false);
    m.kernel.push_text("./ev.wasm; echo status $?\n");
    m.settle();
    m.kernel.push_key_event(Key::Char('a'), true);
    m.kernel.push_key_event(Key::Char('a'), false);
    m.key(Key::Enter);
    assert!(m.screen().contains("status 0"), "{}", m.screen());
}

#[test]
fn ctrl_c_ends_a_wasm_program_in_pixel_mode() {
    let m = Machine::boot();
    install(
        &m,
        "loop.wasm",
        r#"(module
  (import "kiddos" "gfx_mode" (func $mode (param i32)))
  (memory (export "memory") 1)
  (func (export "main") (result i32) (call $mode (i32.const 1)) (loop br 0) (i32.const 0)))"#,
    );
    m.run("clear");
    m.kernel.push_text("./loop.wasm\n");
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(m.kernel.screen.lock().pixel_mode());
    m.kernel.push_key(Key::Ctrl('c'));
    m.settle();
    assert!(!m.kernel.screen.lock().pixel_mode());
    assert!(m.screen().contains("kid@"), "{}", m.screen());
}

#[test]
fn pixel_mode_hides_text_and_restores_it() {
    let m = Machine::boot();
    m.run("clear");
    m.run("echo remember me");
    install(
        &m,
        "flash.wasm",
        r#"(module
  (import "kiddos" "gfx_mode" (func $mode (param i32)))
  (import "kiddos" "readkey" (func $readkey (result i32)))
  (memory (export "memory") 1)
  (func (export "main") (result i32)
    (call $mode (i32.const 1)) (drop (call $readkey)) (call $mode (i32.const 0)) (i32.const 0)))"#,
    );
    m.run("./flash.wasm");
    assert!(m.kernel.screen.lock().pixel_mode());
    // text is still there underneath
    assert!(m.screen().contains("remember me"));
    m.key(Key::Enter);
    assert!(!m.kernel.screen.lock().pixel_mode());
    assert!(m.screen().contains("remember me"));
}

#[test]
fn basic_reads_and_writes_files() {
    let m = Machine::boot();
    m.run("clear");
    write(
        &m,
        "files.bas",
        "WRITEFILE \"note.txt\", \"one\" + CHR(10)\nAPPENDFILE \"note.txt\", \"two\" + CHR(10)\nPRINT LEN(READFILE(\"note.txt\"))\nPRINT LEN(READFILE(\"nothing.txt\"))\n",
    );
    m.run("basic files.bas");
    m.run("cat note.txt");
    let s = m.screen();
    assert!(s.contains(" 8\n 0\n"), "{s}");
    assert!(s.contains("one\ntwo"), "{s}");
}

#[test]
fn paint_paints_saves_and_loads() {
    let m = Machine::boot();
    m.run("clear");
    m.run("play paint");
    assert!(m.kernel.screen.lock().pixel_mode(), "{}", m.screen());
    // cursor starts at dot (32, 18); paint it yellow (the default color)
    m.key(Key::Char(' '));
    assert_eq!(front(&m, 32 * 5 + 2, 18 * 5 + 2), 14);
    // move right, pick red (4), paint; erase is X
    m.key(Key::Right);
    m.key(Key::Char('4'));
    m.key(Key::Char(' '));
    assert_eq!(front(&m, 33 * 5 + 2, 18 * 5 + 2), 4);
    m.key(Key::Char('x'));
    m.key(Key::Left);
    assert_eq!(front(&m, 33 * 5 + 2, 18 * 5 + 2), 0);
    // the color bar and help line are there
    assert_eq!(front(&m, 4 * 12 + 5, 186), 4);
    m.key(Key::Char('s'));
    m.key(Key::Char('n'));
    assert_eq!(front(&m, 32 * 5 + 2, 18 * 5 + 2), 0, "N clears");
    m.key(Key::Char('l'));
    assert_eq!(front(&m, 32 * 5 + 2, 18 * 5 + 2), 14, "L brings it back");
    m.key(Key::Escape);
    assert!(!m.kernel.screen.lock().pixel_mode());
    let text = m
        .kernel
        .vfs
        .lock()
        .read("/home/kid/picture.txt", &Actor::user("kid"))
        .unwrap();
    let text = String::from_utf8(text).unwrap();
    let rows: Vec<&str> = text.lines().collect();
    assert_eq!(rows.len(), 36);
    assert!(rows.iter().all(|r| r.len() == 64));
    assert_eq!(&rows[18][30..36], "..N...", "yellow dot is N, the red one was erased");
}
