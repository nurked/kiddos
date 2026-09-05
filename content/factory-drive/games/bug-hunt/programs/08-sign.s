// 08-sign: x0 is -3. The program should print   negative
// It prints "positive". The comparison is fine; the branch is not.
// man registers explains the flags and which b.xx is for what.

.text
_start:
    mov x0, #-3
    cmp x0, #0
    b.hi positive           // if x0 > 0

    adr x1, neg
    mov x2, #9
    b say
positive:
    adr x1, pos
    mov x2, #9
say:
    mov x0, #1
    mov x8, #64
    svc #0
    mov x0, #0
    mov x8, #93
    svc #0

.data
neg: .ascii "negative\n"
pos: .ascii "positive\n"
