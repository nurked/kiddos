/* Bounce: a ball in pixel mode. Hold the arrows to push it, ESC ends.
 *     cc bounce.c     ./bounce.wasm                                       */
#include "kiddos.h"

int main(void) {
    int x = 160, y = 100, dx = 2, dy = 1;
    kd_gfx_mode(1);
    while (!kd_key_down(KD_KEY_ESC)) {
        if (kd_key_down(KD_KEY_LEFT))  dx--;
        if (kd_key_down(KD_KEY_RIGHT)) dx++;
        if (kd_key_down(KD_KEY_UP))    dy--;
        if (kd_key_down(KD_KEY_DOWN))  dy++;
        x += dx; y += dy;
        if (x < 10 || x > 309) { dx = -dx; kd_beep(440, 20); }
        if (y < 10 || y > 189) { dy = -dy; kd_beep(660, 20); }
        kd_gfx_fill(0, 0, KD_GFX_W, KD_GFX_H, KD_RGB(0, 0, 2));   /* night sky */
        kd_gfx_circle(x, y, 10, KD_YELLOW, 1);
        kd_gfx_text(8, 8, "arrows push, ESC ends", KD_WHITE, -1);
        kd_gfx_flip();
        kd_sleep(16);
    }
    return 0;
}
