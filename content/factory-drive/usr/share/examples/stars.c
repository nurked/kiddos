/* Stars: a tiny screen program. Any key ends it.   cc stars.c   ./stars.wasm */
#include "kiddos.h"

int main(void) {
    kd_clear(KD_BLACK);
    kd_cursor_show(0);
    kd_puts(2, 0, "stars - press any key", KD_YELLOW, KD_BLACK);
    while (kd_getkey() < 0) {
        int x = 1 + kd_random() % (kd_cols() - 2);
        int y = 1 + kd_random() % (kd_rows() - 2);
        int c = KD_DARKGRAY + kd_random() % 8;
        kd_put(x, y, '*', c, KD_BLACK);
        kd_sleep(30);
    }
    kd_clear(KD_BLACK);
    kd_cursor_show(1);
    return 0;
}
