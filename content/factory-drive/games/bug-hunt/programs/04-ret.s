// 04-ret: should print   Hi!Hi!Bye
// It only says Hi! once. Step through it with n and s: where does the
// program go after the first Hi!?

.text
_start:
    bl shout
    bl shout
    b bye

shout:                      // print "Hi!" and come back
    mov x0, #1
    adr x1, hi
    mov x2, #3
    mov x8, #64
    svc #0

bye:
    mov x0, #1
    adr x1, byemsg
    mov x2, #4
    mov x8, #64
    svc #0
    mov x0, #0
    mov x8, #93
    svc #0

.data
hi: .ascii "Hi!"
byemsg: .ascii "Bye\n"
