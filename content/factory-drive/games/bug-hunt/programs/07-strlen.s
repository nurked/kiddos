// 07-strlen: should print   5   (the length of "Hello")
// The counting loop walks the text one letter at a time until it finds
// the 0 at the end. One instruction reads too much. man as: sizes.

.text
_start:
    adr x1, word
    mov x19, #0             // x19 = letters counted so far
count:
    ldr x2, [x1]            // read one letter
    cbz x2, done            // 0 = the end of the text
    add x19, x19, #1
    add x1, x1, #1          // next letter
    b count
done:
    add x0, x19, #'0'
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
word: .asciz "Hello"
.align 3
out: .ascii "?\n"
