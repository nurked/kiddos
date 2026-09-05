// echo.s - read a line you type and say it back.
//
//   as echo.s && ./echo
//
// read (63) waits for a line and copies it into a buffer you give it.
// It returns how many bytes it copied (the Enter counts as one).

.text
_start:
    mov x0, #1
    adr x1, ask
    mov x2, asklen
    mov x8, #64             // write "Say something: "
    svc #0

    mov x0, #0              // 0 = the keyboard
    adr x1, line
    mov x2, #100            // room for 100 bytes
    mov x8, #63             // 63 = read
    svc #0
    mov x19, x0             // x19 = how many bytes we got

    mov x0, #1
    adr x1, you
    mov x2, youlen
    mov x8, #64             // write "You said: "
    svc #0

    mov x0, #1
    adr x1, line
    mov x2, x19             // exactly the bytes we got
    mov x8, #64
    svc #0

    mov x0, #0
    mov x8, #93
    svc #0

.data
ask: .ascii "Say something: "
asklen = . - ask
you: .ascii "You said: "
youlen = . - you

.bss
line: .space 100
