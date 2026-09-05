/* kiddos.h - the whole machine, as seen from C.
 *
 * A KidDOS C program has no libc. It has this file. Every function here
 * talks to the console, the keyboard, the clock or the drive, and nothing
 * else exists. Compile with:   cc hello.c        run with:   ./hello.wasm
 *
 *     #include "kiddos.h"
 *     int main(void) {
 *         kd_print("Hello!\n");
 *         return 0;
 *     }
 */
#ifndef KIDDOS_H
#define KIDDOS_H

#define KD_IMPORT(n) __attribute__((import_module("kiddos"), import_name(n)))

/* --- the raw imports (string = pointer + length) ------------------------ */
KD_IMPORT("print")       void kd_print_n(const char *s, int len);
KD_IMPORT("eprint")      void kd_eprint_n(const char *s, int len);
KD_IMPORT("put")         void kd_put(int x, int y, int ch, int fg, int bg);
KD_IMPORT("cursor")      void kd_cursor(int x, int y);
KD_IMPORT("cursor_show") void kd_cursor_show(int visible);
KD_IMPORT("clear")       void kd_clear(int bg);
KD_IMPORT("color")       void kd_color(int fg, int bg);
KD_IMPORT("size")        int  kd_size_raw(void);          /* cols << 16 | rows */
KD_IMPORT("getkey")      int  kd_getkey(void);            /* -1 if no key waiting */
KD_IMPORT("readkey")     int  kd_readkey(void);           /* waits */
KD_IMPORT("readline")    int  kd_readline_n(char *buf, int cap); /* -1 at end of input */
KD_IMPORT("sleep")       void kd_sleep(int ms);
KD_IMPORT("tick")        long long kd_tick(void);         /* ms since boot */
KD_IMPORT("beep")        void kd_beep(int freq, int ms);
KD_IMPORT("speak")       int  kd_speak_n(const char *s, int len);
KD_IMPORT("random")      int  kd_random(void);            /* 0 .. 2^31-1 */
KD_IMPORT("exit")        void kd_exit(int code);
KD_IMPORT("fs_read")     int  kd_fs_read_n(const char *path, int plen, char *buf, int cap);
KD_IMPORT("fs_write")    int  kd_fs_write_n(const char *path, int plen, const char *data, int dlen, int append);

/* --- pixel mode: 320 x 200, 256 colors, double-buffered ----------------- */
/* kd_gfx_mode(1) switches the screen to pixels (the text stays underneath  */
/* and comes back with kd_gfx_mode(0) or when the program ends). Drawing    */
/* goes to a hidden buffer; kd_gfx_flip() shows it. Colors are palette      */
/* numbers: 0-15 the usual colors, 16-31 grays, 32-247 a 6x6x6 color cube  */
/* (KD_RGB below), and you can change any entry with kd_gfx_palette.        */
KD_IMPORT("gfx_mode")    void kd_gfx_mode(int on);
KD_IMPORT("gfx_clear")   void kd_gfx_clear(int color);
KD_IMPORT("gfx_pixel")   void kd_gfx_pixel(int x, int y, int color);
KD_IMPORT("gfx_get")     int  kd_gfx_get(int x, int y);
KD_IMPORT("gfx_line")    void kd_gfx_line(int x1, int y1, int x2, int y2, int color);
KD_IMPORT("gfx_rect")    void kd_gfx_rect(int x, int y, int w, int h, int color);   /* outline */
KD_IMPORT("gfx_fill")    void kd_gfx_fill(int x, int y, int w, int h, int color);   /* filled  */
KD_IMPORT("gfx_circle")  void kd_gfx_circle(int x, int y, int r, int color, int filled);
KD_IMPORT("gfx_blit")    void kd_gfx_blit(int x, int y, int w, int h, const unsigned char *pixels, int transparent); /* transparent: a color, or -1 */
KD_IMPORT("gfx_read")    int  kd_gfx_read(int x, int y, int w, int h, unsigned char *out);
KD_IMPORT("gfx_palette") void kd_gfx_palette(int index, int r, int g, int b);
KD_IMPORT("gfx_text")    int  kd_gfx_text_n(int x, int y, const char *s, int len, int fg, int bg); /* bg -1 = see-through */
KD_IMPORT("gfx_flip")    void kd_gfx_flip(void);
KD_IMPORT("key_down")    int  kd_key_down(int key);          /* 1 while the key is held */
KD_IMPORT("key_event")   int  kd_key_event(void);            /* next key down/up, or -1 */

#define KD_GFX_W 320
#define KD_GFX_H 200
/* a palette number from red, green, blue levels 0..5 */
#define KD_RGB(r, g, b) (32 + 36 * (r) + 6 * (g) + (b))
/* a palette number from a gray level 0..15 */
#define KD_GRAY(v) (16 + (v))
/* kd_key_event() sets this bit when the key went up rather than down */
#define KD_KEY_RELEASED 0x1000000

/* --- keys, as returned by kd_getkey / kd_readkey ------------------------ */
/* Letters and symbols come back as themselves: 'a', ' ', '7'. Others:    */
#define KD_KEY_ENTER   0x110001
#define KD_KEY_BS      0x110002
#define KD_KEY_TAB     0x110003
#define KD_KEY_ESC     0x110004
#define KD_KEY_UP      0x110005
#define KD_KEY_DOWN    0x110006
#define KD_KEY_LEFT    0x110007
#define KD_KEY_RIGHT   0x110008
#define KD_KEY_HOME    0x110009
#define KD_KEY_END     0x11000A
#define KD_KEY_PGUP    0x11000B
#define KD_KEY_PGDN    0x11000C
#define KD_KEY_INS     0x11000D
#define KD_KEY_DEL     0x11000E
#define KD_KEY_F(n)    (0x110014 + (n))
#define KD_KEY_CTRL(c) (0x120000 + (c))   /* KD_KEY_CTRL('c') */

/* --- colors, same numbers as BASIC's COLOR ------------------------------ */
enum { KD_BLACK, KD_BLUE, KD_GREEN, KD_CYAN, KD_RED, KD_MAGENTA, KD_BROWN, KD_GRAY,
       KD_DARKGRAY, KD_LIGHTBLUE, KD_LIGHTGREEN, KD_LIGHTCYAN, KD_LIGHTRED, KD_LIGHTMAGENTA,
       KD_YELLOW, KD_WHITE };

/* --- small helpers so you do not need a libc ----------------------------- */
static inline int kd_strlen(const char *s) { int n = 0; while (s[n]) n++; return n; }
static inline void kd_print(const char *s) { kd_print_n(s, kd_strlen(s)); }
static inline void kd_eprint(const char *s) { kd_eprint_n(s, kd_strlen(s)); }
static inline int  kd_speak(const char *s) { return kd_speak_n(s, kd_strlen(s)); }
static inline int  kd_cols(void) { return kd_size_raw() >> 16; }
static inline int  kd_rows(void) { return kd_size_raw() & 0xFFFF; }
static inline void kd_puts(int x, int y, const char *s, int fg, int bg) {
    for (int i = 0; s[i]; i++) kd_put(x + i, y, s[i], fg, bg);
}
static inline void kd_print_int(int v) {
    char b[12]; int i = 11; b[i] = 0;
    unsigned u = v < 0 ? -(unsigned)v : (unsigned)v;
    do { b[--i] = '0' + u % 10; u /= 10; } while (u);
    if (v < 0) b[--i] = '-';
    kd_print(b + i);
}
/* Read a line into buf (up to cap-1 chars, always 0-terminated). 0 at end of input. */
static inline int kd_readline(char *buf, int cap) {
    int n = kd_readline_n(buf, cap - 1);
    if (n < 0) { buf[0] = 0; return 0; }
    buf[n] = 0; return 1;
}
static inline int kd_gfx_text(int x, int y, const char *s, int fg, int bg) {
    return kd_gfx_text_n(x, y, s, kd_strlen(s), fg, bg);
}
static inline int kd_fs_read(const char *path, char *buf, int cap) {
    return kd_fs_read_n(path, kd_strlen(path), buf, cap);
}
static inline int kd_fs_write(const char *path, const char *data) {
    return kd_fs_write_n(path, kd_strlen(path), data, kd_strlen(data), 0);
}
static inline int kd_fs_append(const char *path, const char *data) {
    return kd_fs_write_n(path, kd_strlen(path), data, kd_strlen(data), 1);
}

#endif
