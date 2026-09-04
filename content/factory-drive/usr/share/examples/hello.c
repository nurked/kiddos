/* Your first C program. Copy it home, then:   cc hello.c   and   ./hello.wasm */
#include "kiddos.h"

int main(void) {
    kd_print("Hello from C!\n");
    kd_print("The screen is ");
    kd_print_int(kd_cols());
    kd_print(" by ");
    kd_print_int(kd_rows());
    kd_print(" letters.\n");
    return 0;
}
