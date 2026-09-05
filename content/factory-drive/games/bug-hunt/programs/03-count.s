// 03-count: should print   12345
// One digit is missing. Which way does the loop end?

.text
_start:
    mov x19, #1
loop:
    add x0, x19, #'0'
    adr x1, out
    strb w0, [x1]
    mov x0, #1
    mov x2, #1
    mov x8, #64
    svc #0                  // print one digit

    add x19, x19, #1
    cmp x19, #5
    b.lt loop               // go again while x19 < 5

    mov x0, #1
    adr x1, nl
    mov x2, #1
    mov x8, #64
    svc #0

    mov x0, #0
    mov x8, #93
    svc #0

.data
out: .byte 0
nl: .ascii "\n"
