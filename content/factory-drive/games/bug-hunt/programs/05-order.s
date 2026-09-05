// 05-order: should print   7
// The number is stored in a box in memory, then printed.
// It prints 0. Use   :mem box   in debug and watch when the box changes.

.text
_start:
    adr x1, box
    ldrb w0, [x1]           // take the number out of the box
    mov x2, #7
    strb w2, [x1]           // put 7 into the box

    add x0, x0, #'0'
    adr x1, out
    strb w0, [x1]
    mov x0, #1
    mov x2, #2
    mov x8, #64
    svc #0

    mov x0, #0
    mov x8, #93
    svc #0

.data
box: .byte 0
out: .ascii "?\n"
