// hello.s - the smallest ARM program: say hello, then stop.
//
//   as hello.s        makes a program called hello
//   ./hello           runs it
//   debug hello       runs it one instruction at a time
//
// The machine does only two things for you: it runs instructions, and it
// does "system calls" when you ask with svc (man syscalls). Everything
// else - the text, its length - you set up in registers yourself.

.text
_start:
    mov x0, #1          // x0 = 1  (1 means "the screen")
    adr x1, msg         // x1 = the address where the text lives
    mov x2, len         // x2 = how many bytes to write
    mov x8, #64         // 64 = write  (man syscalls)
    svc #0              // do it

    mov x0, #0          // 0 = "everything went fine"
    mov x8, #93         // 93 = exit
    svc #0

.data
msg: .ascii "Hello from ARM!\n"
len = . - msg           // "here" minus "where msg starts" = its length
