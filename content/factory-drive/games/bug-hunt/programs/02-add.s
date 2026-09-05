// 02-add: 2 + 3 should print   5
// It prints another number. Watch x0 in debug.

.text
_start:
    mov x1, #2
    mov x2, #3
    add x0, x1, x1          // x0 = x1 + x2
    add x0, x0, #'0'        // turn the number into its digit
    adr x1, out
    strb w0, [x1]           // put the digit into the text

    mov x0, #1
    mov x2, #2
    mov x8, #64
    svc #0

    mov x0, #0
    mov x8, #93
    svc #0

.data
out: .ascii "?\n"
