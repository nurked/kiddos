// 01-hello: should print   Hello!
// It prints something shorter. Why?

.text
_start:
    mov x0, #1
    adr x1, msg
    mov x2, #5              // how many bytes to write
    mov x8, #64
    svc #0

    mov x0, #0
    mov x8, #93
    svc #0

.data
msg: .ascii "Hello!\n"
