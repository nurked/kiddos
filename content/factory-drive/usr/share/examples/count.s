// count.s - count from 1 to 10, one number per line.
//
// The CPU has no idea what "10" looks like on a screen. To print a
// number you turn it into letters yourself: divide by 10 again and
// again, and each remainder is one digit. print_num does that.
//
//   as count.s && ./count
//   debug count          then press s and watch x19 go up

.text
_start:
    mov x19, #1             // x19 = the number we are on
loop:
    mov x0, x19
    bl print_num            // print it (bl = call, and remember where to come back)
    add x19, x19, #1        // next number
    cmp x19, #10
    b.le loop               // while x19 <= 10, go again

    mov x0, #0
    mov x8, #93             // exit
    svc #0

// print_num: print the number in x0 and a new line.
// Digits are built backwards (last digit first) into buf, from its end.
print_num:
    stp x29, x30, [sp, #-16]!   // save where to return to (bl changed x30)
    adr x1, bufend          // x1 walks backwards from the end of buf
    mov x2, #10
    mov x3, #'\n'
    strb w3, [x1, #-1]!     // put the new line last
digit:
    udiv x4, x0, x2         // x4 = x0 / 10
    msub x5, x4, x2, x0     // x5 = x0 - x4*10  (the remainder: one digit)
    add x5, x5, #'0'        // 7 -> '7'
    strb w5, [x1, #-1]!     // store it one byte earlier
    mov x0, x4              // keep the rest
    cbnz x0, digit          // more digits? go again

    mov x0, #1              // write(1, x1, bufend - x1)
    adr x2, bufend
    sub x2, x2, x1
    mov x8, #64
    svc #0
    ldp x29, x30, [sp], #16     // get the return address back
    ret

.data
buf: .space 24
bufend:
