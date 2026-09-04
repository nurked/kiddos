//! Phase 3: the WASM sandbox. Modules are written in WebAssembly text
//! here, the way `cc` output would be, so no C compiler is needed.

use kiddos_headless::Machine;
use kiddos_kernel::{Actor, Key};

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
    let host = kiddos_host::RealHost::new(paths, tx, Box::new(|| {}));
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
