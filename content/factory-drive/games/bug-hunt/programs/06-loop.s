// 06-loop: should print   *****   (five stars)
// It never finishes. Press c in debug, then Ctrl-C, and look at x19.

.text
_start:
    mov x19, #0
stars:
    mov x0, #1
    adr x1, star
    mov x2, #1
    mov x8, #64
    svc #0                  // one star

    sub x19, x19, #1        // count it
    cmp x19, #5
    b.lt stars              // five stars in total

    mov x0, #1
    adr x1, nl
    mov x2, #1
    mov x8, #64
    svc #0
    mov x0, #0
    mov x8, #93
    svc #0

.data
star: .ascii "*"
nl: .ascii "\n"
