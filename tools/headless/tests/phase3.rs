//! Phase 3: the WASM sandbox. Modules are written in WebAssembly text
//! here, the way `cc` output would be, so no C compiler is needed.

use kiddos_headless::Machine;
use kiddos_kernel::{Actor, Key};

/// The compiler tests read and write process environment variables, so
/// they must not overlap.
static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn install(m: &Machine, name: &str, wat_src: &str) {
    let bytes = wat::parse_str(wat_src).expect("valid wat");
    let path = format!("/home/kid/{name}");
    let mut vfs = m.kernel.vfs.lock();
    vfs.write(&path, &bytes, &Actor::user("kid")).unwrap();
    vfs.chmod(&path, 0o755, &Actor::user("kid")).unwrap();
}

const HELLO: &str = r#"(module
  (import "kiddos" "print" (func $print (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 8) "Hello from wasm\n")
  (func (export "main") (result i32)
    (call $print (i32.const 8) (i32.const 16))
    (i32.const 7)))"#;

#[test]
fn a_wasm_program_prints_and_exits() {
    let m = Machine::boot();
    install(&m, "hello.wasm", HELLO);
    m.run("clear");
    m.run("./hello.wasm; echo status $?");
    let s = m.screen();
    assert!(s.contains("Hello from wasm"), "{s}");
    assert!(s.contains("status 7"), "{s}");
    m.run("wasm hello.wasm");
    assert!(m.screen().matches("Hello from wasm").count() == 2, "{}", m.screen());
}

#[test]
fn keys_put_and_exit_code() {
    let m = Machine::boot();
    // waits for a key, PUTs it at (5,3) in yellow, then exit(3)
    install(
        &m,
        "key.wasm",
        r#"(module
  (import "kiddos" "readkey" (func $readkey (result i32)))
  (import "kiddos" "put" (func $put (param i32 i32 i32 i32 i32)))
  (import "kiddos" "exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "main") (result i32)
    (call $put (i32.const 5) (i32.const 3) (call $readkey) (i32.const 14) (i32.const 0))
    (call $exit (i32.const 3))
    (i32.const 0)))"#,
    );
    m.run("clear");
    m.run("./key.wasm; echo status $?");
    m.key(Key::Char('Q'));
    assert_eq!(m.cell(5, 3).ch, 'Q', "{}", m.screen());
    assert_eq!(m.cell(5, 3).fg, kiddos_console::colors::YELLOW);
    assert!(m.screen().contains("status 3"), "{}", m.screen());
}

#[test]
fn ctrl_c_stops_a_tight_loop() {
    let m = Machine::boot();
    install(
        &m,
        "spin.wasm",
        r#"(module (memory (export "memory") 1) (func (export "main") (result i32) (loop br 0) (i32.const 0)))"#,
    );
    m.run("clear");
    m.kernel.push_text("./spin.wasm");
    m.kernel.push_key(Key::Enter);
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(m.kernel.processes().iter().any(|p| p.name == "wasm"), "{}", m.screen());
    m.kernel.push_key(Key::Ctrl('c'));
    m.settle();
    assert!(!m.kernel.processes().iter().any(|p| p.name == "wasm"));
    assert!(m.screen().ends_with("kid@kiddos:~$"), "{}", m.screen());
}

#[test]
fn errors_are_sentences() {
    let m = Machine::boot();
    // memory beyond the cap: 300 pages = 19 MB
    install(
        &m,
        "big.wasm",
        r#"(module (memory (export "memory") 1) (func (export "main") (result i32) (drop (memory.grow (i32.const 300))) (i32.load (i32.const 18000000))))"#,
    );
    install(
        &m,
        "nomain.wasm",
        r#"(module (memory (export "memory") 1) (func (export "other")))"#,
    );
    install(
        &m,
        "div.wasm",
        r#"(module (memory (export "memory") 1) (func (export "main") (result i32) (i32.div_s (i32.const 1) (i32.const 0))))"#,
    );
    install(
        &m,
        "needs.wasm",
        r#"(module (import "kiddos" "teleport" (func $t)) (memory (export "memory") 1) (func (export "main") (result i32) (call $t) (i32.const 0)))"#,
    );
    m.run_script(
        "@clear
./big.wasm
@expect reached outside its memory
./nomain.wasm
@expect has no main
./div.wasm
@expect divided by zero
./needs.wasm
@expect does not have
",
    );
    m.run("clear");
    m.run("echo 'int main(void){}' > x.c; cc x.c");
    assert!(m.screen().contains("no C compiler"), "{}", m.screen());
    m.run("cc");
    assert!(m.screen().contains("cc hello.c"), "{}", m.screen());
}

#[test]
fn programs_can_read_the_drive() {
    let m = Machine::boot();
    // reads /etc/hostname into memory and prints it
    install(
        &m,
        "host.wasm",
        r#"(module
  (import "kiddos" "fs_read" (func $fs_read (param i32 i32 i32 i32) (result i32)))
  (import "kiddos" "print" (func $print (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "/etc/hostname")
  (func (export "main") (result i32)
    (call $print (i32.const 100) (call $fs_read (i32.const 0) (i32.const 13) (i32.const 100) (i32.const 50)))
    (i32.const 0)))"#,
    );
    m.run("clear");
    m.run("./host.wasm");
    assert!(m.screen().contains("\nkiddos\n"), "{}", m.screen());
    m.run("cat /usr/include/kiddos.h | grep -c KD_IMPORT");
    assert!(!m.screen().contains("\n0\n"), "{}", m.screen());
}

/// The real thing: clang on the host, the result in the sandbox. Runs only
/// where a wasm-capable clang is reachable (`KIDDOS_CC`, or Homebrew LLVM
/// with lld on a Mac); elsewhere it passes vacuously and says so.
#[test]
fn cc_compiles_the_example_and_it_runs() {
    use kiddos_kernel::HostCaps;
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    if std::env::var("KIDDOS_CC").is_err() {
        for candidate in [
            "/opt/homebrew/opt/llvm/bin/clang",
            "/usr/lib/llvm-18/bin/clang",
            "/usr/bin/clang-18",
        ] {
            if std::path::Path::new(candidate).exists() {
                std::env::set_var("KIDDOS_CC", candidate);
                break;
            }
        }
    }
    let dir = std::env::temp_dir().join(format!("kiddos-cc-test-{}", std::process::id()));
    let paths = kiddos_host::Paths::in_dir(dir.clone());
    paths.ensure().unwrap();
    let (tx, _rx) = std::sync::mpsc::channel();
    let host = kiddos_host::RealHost::new(paths.clone(), tx, Box::new(|| {}));
    // KIDDOS_TEST_KDP=<file.kdp>: exercise a real built pack instead of KIDDOS_CC
    if let Ok(kdp) = std::env::var("KIDDOS_TEST_KDP") {
        std::env::remove_var("KIDDOS_CC");
        std::fs::copy(&kdp, paths.carts.join("c.kdp")).expect("copy pack");
        let summary = host.install_pack("c.kdp").expect("install real pack");
        eprintln!("installed {summary}");
    }
    if let Err(why) = host.c_compiler_available() {
        eprintln!("skipping: {why}");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let factory = kiddos_headless::factory_dir();
    let hello = std::fs::read(factory.join("usr/share/examples/hello.c")).unwrap();
    let header = std::fs::read(factory.join("usr/include/kiddos.h")).unwrap();
    let wasm = host
        .compile_c(&[("hello.c".into(), hello), ("kiddos.h".into(), header.clone())])
        .expect("hello.c compiles");
    assert!(wasm.starts_with(b"\0asm"));
    let stars = std::fs::read(factory.join("usr/share/examples/stars.c")).unwrap();
    host.compile_c(&[("stars.c".into(), stars), ("kiddos.h".into(), header.clone())])
        .expect("stars.c compiles");
    // a broken program gets a translated error
    let bad = b"#include \"kiddos.h\"\nint main(void) { kd_print(\"hi\") return 0; }\n".to_vec();
    let err = host
        .compile_c(&[("bad.c".into(), bad), ("kiddos.h".into(), header)])
        .unwrap_err();
    let lines = kiddos_wasm::cc::humanize(&err);
    assert!(
        lines[0].contains("bad.c, line 2") && lines[0].contains("semicolon"),
        "{lines:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);

    let m = Machine::boot();
    {
        let mut vfs = m.kernel.vfs.lock();
        vfs.write("/home/kid/hello.wasm", &wasm, &Actor::user("kid")).unwrap();
        vfs.chmod("/home/kid/hello.wasm", 0o755, &Actor::user("kid")).unwrap();
    }
    m.run("clear");
    m.run("./hello.wasm");
    let s = m.screen();
    assert!(s.contains("Hello from C!"), "{s}");
    assert!(s.contains("The screen is 80 by 25 letters."), "{s}");
}

#[test]
fn rogue_the_c_cartridge_plays() {
    let m = Machine::boot();
    m.run("games");
    assert!(m.screen().contains("rogue        Rogue"), "{}", m.screen());
    m.run("clear");
    m.run("play rogue");
    let s = m.screen();
    assert!(s.starts_with("Depth 1  HP 12/12  Gold 0"), "{s}");
    assert!(s.contains("Welcome to the dungeon."), "{s}");
    assert!(s.contains('@'), "{s}");
    m.key(Key::Char('?'));
    assert!(m.screen().contains("This whole game is one C file"), "{}", m.screen());
    m.key(Key::Char(' '));
    for _ in 0..6 {
        m.key(Key::Right);
    }
    m.key(Key::Char('p'));
    // the dungeon is random: a monster may have interrupted with its own message
    let s = m.screen();
    assert!(s.contains("You have no potion.") || s.contains("bites you!"), "{s}");
    assert!(s.starts_with("Depth 1"), "{s}");
    m.key(Key::Escape);
    let s = m.screen();
    assert!(s.contains("You left the dungeon with"), "{s}");
    assert!(s.ends_with("kid@kiddos:~$"), "{s}");
    m.run("clear");
    m.run("cat /games/rogue/rogue.c | grep -c kd_");
    assert!(!m.screen().contains("\n0\n"), "{}", m.screen());
}

/// A pack is a zip in carts/ with pack.toml and bin/clang. Install one whose
/// "clang" is a script that writes a canned module, and compile through it:
/// that proves install-pack, the PATH handling and the cc plumbing without
/// needing a real compiler.
#[test]
fn install_pack_then_cc_through_it() {
    use kiddos_kernel::HostCaps;
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let saved_cc = std::env::var("KIDDOS_CC").ok();
    use std::io::Write;
    let dir = std::env::temp_dir().join(format!("kiddos-pack-test-{}", std::process::id()));
    let paths = kiddos_host::Paths::in_dir(dir.clone());
    paths.ensure().unwrap();
    let canned = wat::parse_str(HELLO).unwrap();
    // a fake clang: copies the canned module to whatever follows -o
    let script = format!(
        "#!/bin/sh\nwhile [ $# -gt 0 ]; do if [ \"$1\" = -o ]; then out=\"$2\"; fi; shift; done\nprintf '%s' '{}' | xxd -r -p > \"$out\"\n",
        canned.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default();
    zw.start_file("c/pack.toml", o).unwrap();
    zw.write_all(b"name = \"c\"\ndescription = \"a fake C compiler for the test\"\n")
        .unwrap();
    zw.start_file("c/bin/clang", o.unix_permissions(0o755)).unwrap();
    zw.write_all(script.as_bytes()).unwrap();
    let bytes = zw.finish().unwrap().into_inner();
    std::fs::write(paths.carts.join("c.kdp"), &bytes).unwrap();

    let (tx, _rx) = std::sync::mpsc::channel();
    let host = kiddos_host::RealHost::new(paths.clone(), tx, Box::new(|| {}));
    std::env::remove_var("KIDDOS_CC");
    assert!(host.c_compiler_available().is_err());
    let summary = host.install_pack("c.kdp").unwrap();
    assert!(summary.starts_with("c: 2 files"), "{summary}");
    assert_eq!(
        host.list_packs(),
        vec![("c".to_string(), "a fake C compiler for the test".to_string())]
    );
    assert!(host.c_compiler_available().is_ok());
    let wasm = host
        .compile_c(&[("x.c".into(), b"int main(void){return 0;}".to_vec())])
        .unwrap();
    assert_eq!(wasm, canned);
    host.remove_pack("c").unwrap();
    assert!(host.c_compiler_available().is_err());
    assert!(host.install_pack("nothing.kdp").is_err());
    if let Some(v) = saved_cc {
        std::env::set_var("KIDDOS_CC", v);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Go through TinyGo on the host, run in the sandbox. Runs where TinyGo is
/// reachable (`KIDDOS_TINYGO`); elsewhere it passes vacuously and says so.
#[test]
fn goc_compiles_the_example_and_it_runs() {
    use kiddos_kernel::HostCaps;
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("kiddos-go-test-{}", std::process::id()));
    let paths = kiddos_host::Paths::in_dir(dir.clone());
    paths.ensure().unwrap();
    let (tx, _rx) = std::sync::mpsc::channel();
    let host = kiddos_host::RealHost::new(paths.clone(), tx, Box::new(|| {}));
    if let Ok(kdp) = std::env::var("KIDDOS_TEST_GO_KDP") {
        std::env::remove_var("KIDDOS_TINYGO");
        std::fs::copy(&kdp, paths.carts.join("go.kdp")).expect("copy pack");
        eprintln!("installed {}", host.install_pack("go.kdp").expect("install go pack"));
    }
    if let Err(why) = host.go_compiler_available() {
        eprintln!("skipping: {why}");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let factory = kiddos_headless::factory_dir();
    let hello = std::fs::read(factory.join("usr/share/examples/hello.go")).unwrap();
    let pkg: Vec<(String, Vec<u8>)> = std::fs::read_dir(factory.join("usr/share/go/kiddos"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| {
            (
                e.file_name().to_string_lossy().to_string(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    let wasm = host
        .compile_go(&[("hello.go".into(), hello)], &pkg)
        .expect("hello.go compiles");
    assert!(wasm.starts_with(b"\0asm"));
    let bad = b"package main\n\nimport \"kiddos\"\n\nfunc main() {\n\tkiddos.print(\"x\")\n}\n".to_vec();
    let err = host.compile_go(&[("bad.go".into(), bad)], &pkg).unwrap_err();
    let lines = kiddos_wasm::goc::humanize(&err);
    assert!(
        lines[0].contains("bad.go, line 6") && lines[0].contains("case-sensitive"),
        "{lines:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);

    let m = Machine::boot();
    {
        let mut vfs = m.kernel.vfs.lock();
        vfs.write("/home/kid/hello.wasm", &wasm, &Actor::user("kid")).unwrap();
        vfs.chmod("/home/kid/hello.wasm", 0o755, &Actor::user("kid")).unwrap();
    }
    m.run("clear");
    m.run("./hello.wasm");
    let s = m.screen();
    assert!(s.contains("Hello from Go!"), "{s}");
    assert!(s.contains("The screen is 80 by 25 letters."), "{s}");
    m.run("goc nothing.go");
    assert!(m.screen().contains("no Go compiler"), "{}", m.screen());
}
