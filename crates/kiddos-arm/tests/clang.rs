//! Differential test: the same source through a real clang, word for word.
//! Runs only where `clang` can target aarch64 (any macOS with Xcode's
//! command-line tools, Linux with LLVM); skips quietly elsewhere.

use std::process::Command;

const CORPUS: &str = r#"
_start:
    mov x0, #1
    mov w1, #1
    mov x2, #-1
    mov x3, #0xff
    mov x4, #0x10000
    mov x5, #0xffff0000ffff0000
    mov x6, #0x7fff
    mov w7, #-2
    mov x8, x1
    mov x9, sp
    mov sp, x9
    add x0, x0, #1
    add x0, x1, x2
    add x0, x1, x2, lsl #3
    add x0, x1, x2, lsr #1
    sub sp, sp, #16
    add sp, sp, #16
    subs x0, x1, #2
    adds w0, w1, w2
    cmp x0, #3
    cmp x0, x1
    cmp w0, #10
    cmn x0, #1
    neg x0, x1
    negs w0, w1
    and x0, x1, #0xff
    and x0, x1, x2
    orr x0, x1, x2
    orr x0, x1, #0xf0
    eor w0, w0, w0
    eor x0, x1, #1
    ands x0, x1, x2
    bic x0, x1, x2
    orn x0, x1, x2
    tst x0, #1
    tst x0, x1
    mul x0, x1, x2
    mneg x0, x1, x2
    madd x0, x1, x2, x3
    msub x0, x1, x2, x3
    udiv x2, x0, x1
    sdiv w2, w0, w1
    lsl x0, x0, #4
    lsr x0, x0, #4
    asr x0, x0, #4
    ror x0, x0, #4
    lsl w0, w0, #1
    lsr w0, w0, #31
    lsl x0, x1, x2
    lsr x0, x1, x2
    asr x0, x1, x2
    ror x0, x1, x2
    sxtb x0, w1
    sxth w0, w1
    sxtw x0, w1
    uxtb w0, w1
    uxth w0, w1
    cset x0, eq
    cset w0, lt
    csetm x0, ne
    cinc x0, x1, gt
    cneg x0, x1, le
    csel x0, x1, x2, ne
    csinc x0, x1, x2, hi
    csinv x0, x1, x2, ls
    csneg x0, x1, x2, ge
    b _start
    bl _start
    b.ne _start
    b.eq later
    b.lt _start
    b.hs _start
    b.lo later
    cbz x0, _start
    cbnz w1, later
    tbz x0, #3, _start
    tbnz w0, #31, later
    br x1
    blr x1
    ret
    ret x1
    adr x1, later
    adr x1, _start
    svc #0
    brk #1
    nop
    ldr x0, [x1]
    ldr x0, [x1, #8]
    ldr w0, [x1, #4]
    ldrb w0, [x1]
    strb w0, [x1, #1]
    ldrh w0, [x1]
    strh w0, [x1, #2]
    ldrsb x0, [x1]
    ldrsb w0, [x1]
    ldrsh x0, [x1]
    ldrsw x0, [x1]
    str x0, [sp, #-16]!
    ldr x0, [sp], #16
    str w0, [x1, #4]!
    ldr w0, [x1], #-4
    ldur x0, [x1, #-8]
    stur x0, [x1, #-8]
    ldurb w0, [x1, #-1]
    ldr x0, [x1, x2]
    ldr x0, [x1, x2, lsl #3]
    ldr w0, [x1, x2, lsl #2]
    ldrb w0, [x1, x2]
    strb w0, [x1, x2]
    ldr x0, [x1, w2, uxtw]
    ldr x0, [x1, w2, sxtw #3]
    stp x29, x30, [sp, #-16]!
    ldp x29, x30, [sp], #16
    stp x0, x1, [sp]
    ldp x0, x1, [sp, #16]
    stp w0, w1, [sp, #-8]!
    movk x0, #0x1234, lsl #16
    movz x0, #5
    movn x0, #5, lsl #48
    mvn x0, x1
    ldr x0, later
    ldr w0, later
    ldrsw x0, later
later:
    ret
"#;

fn clang_words(src: &str) -> Option<Vec<u32>> {
    let dir = std::env::temp_dir().join(format!("kiddos-arm-clang-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let s = dir.join("t.s");
    let o = dir.join("t.o");
    std::fs::write(&s, src).ok()?;
    let ok = Command::new("clang")
        .args(["-target", "aarch64-linux-gnu", "-c"])
        .arg(&s)
        .arg("-o")
        .arg(&o)
        .output()
        .ok()?;
    if !ok.status.success() {
        eprintln!("clang: {}", String::from_utf8_lossy(&ok.stderr));
        return None;
    }
    let dump = ["llvm-objdump", "objdump"].iter().find_map(|tool| {
        Command::new(tool)
            .arg("-d")
            .arg(&o)
            .output()
            .ok()
            .filter(|out| out.status.success())
    })?;
    let text = String::from_utf8_lossy(&dump.stdout);
    let mut words = Vec::new();
    for line in text.lines() {
        // "       0: d2800020     	mov	x0, #0x1"
        let t = line.trim_start();
        let Some((addr, rest)) = t.split_once(':') else {
            continue;
        };
        if addr.is_empty() || !addr.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let word = rest.split_whitespace().next().unwrap_or("");
        if word.len() == 8 {
            if let Ok(w) = u32::from_str_radix(word, 16) {
                words.push(w);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    Some(words)
}

#[test]
fn every_encoding_matches_clang() {
    let Some(theirs) = clang_words(CORPUS) else {
        eprintln!("skipping: no clang with an aarch64 target here");
        return;
    };
    let img = kiddos_arm::asm::assemble(CORPUS).unwrap_or_else(|e| panic!("{e}"));
    let ours: Vec<u32> = img
        .text
        .chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let lines: Vec<&str> = CORPUS
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.ends_with(':'))
        .collect();
    assert_eq!(theirs.len(), lines.len(), "clang words vs instruction lines");
    let mut bad = Vec::new();
    for (i, (a, b)) in ours.iter().zip(theirs.iter()).enumerate() {
        if a != b {
            bad.push(format!("{:<36} ours {a:08x}  clang {b:08x}", lines[i]));
        }
    }
    assert!(bad.is_empty(), "mismatches:\n{}", bad.join("\n"));
    assert_eq!(ours.len(), theirs.len());
}
