/* doomgeneric on KidDOS.
 *
 * Doom draws into an 8-bit, 320x200 buffer with its own 256-color palette,
 * which is exactly the machine's pixel mode: each frame is one blit and one
 * flip, and the palette is uploaded when Doom changes it (the red flash when
 * you are hit, the yellow one when you pick something up).
 *
 * Keys: arrows turn and walk, A/D strafe, X fires, SPACE opens doors, Esc is
 * the menu, Enter picks, Tab is the map, 1-7 choose weapons. Run is always on.
 */
#include "doomkeys.h"
#include "m_argv.h"
#include "doomgeneric.h"
#include "i_video.h"
#include "kiddos.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>

/* Doom only calls system() to pop up a desktop error box; there is none. */
int system(const char *cmd) { (void)cmd; return -1; }

#define KEYQUEUE_SIZE 32
static unsigned short s_KeyQueue[KEYQUEUE_SIZE];
static unsigned int s_KeyQueueWriteIndex = 0;
static unsigned int s_KeyQueueReadIndex = 0;

static struct color s_LastPalette[256];
static int s_PaletteSent = 0;

static void addKeyToQueue(int pressed, unsigned char key)
{
    s_KeyQueue[s_KeyQueueWriteIndex] = (unsigned short)((pressed << 8) | key);
    s_KeyQueueWriteIndex = (s_KeyQueueWriteIndex + 1) % KEYQUEUE_SIZE;
}

static unsigned char convertKey(int code)
{
    switch (code) {
    case KD_KEY_ENTER: return KEY_ENTER;
    case KD_KEY_ESC:   return KEY_ESCAPE;
    case KD_KEY_LEFT:  return KEY_LEFTARROW;
    case KD_KEY_RIGHT: return KEY_RIGHTARROW;
    case KD_KEY_UP:    return KEY_UPARROW;
    case KD_KEY_DOWN:  return KEY_DOWNARROW;
    case KD_KEY_TAB:   return KEY_TAB;
    case KD_KEY_BS:    return KEY_BACKSPACE;
    case KD_KEY_HOME:  return KEY_HOME;
    case KD_KEY_END:   return KEY_END;
    case KD_KEY_PGUP:  return KEY_PGUP;
    case KD_KEY_PGDN:  return KEY_PGDN;
    case KD_KEY_INS:   return KEY_INS;
    case KD_KEY_DEL:   return KEY_DEL;
    case ' ':          return KEY_USE;
    case 'x': case 'X': return KEY_FIRE;
    case 'a': case 'A': return KEY_STRAFE_L;
    case 'd': case 'D': return KEY_STRAFE_R;
    case '-':          return KEY_MINUS;
    case '=': case '+': return KEY_EQUALS;
    default: break;
    }
    if (code >= KD_KEY_F(1) && code <= KD_KEY_F(12))
        return (unsigned char)(KEY_F1 + (code - KD_KEY_F(1)));
    if (code >= 'A' && code <= 'Z')
        return (unsigned char)(code - 'A' + 'a');
    if (code > 0 && code < 128)
        return (unsigned char)code;
    return 0;
}

void DG_Init(void)
{
    kd_gfx_mode(1);
    /* always run: Doom thinks Shift is held */
    addKeyToQueue(1, KEY_RSHIFT);
}

void DG_DrawFrame(void)
{
    if (!s_PaletteSent || memcmp(s_LastPalette, colors, sizeof(colors)) != 0) {
        for (int i = 0; i < 256; i++)
            kd_gfx_palette(i, colors[i].r, colors[i].g, colors[i].b);
        memcpy(s_LastPalette, colors, sizeof(colors));
        s_PaletteSent = 1;
    }
    kd_gfx_blit(0, 0, DOOMGENERIC_RESX, DOOMGENERIC_RESY, (const unsigned char *)DG_ScreenBuffer, -1);
    kd_gfx_flip();
    /* drain the keyboard once per frame */
    for (;;) {
        int ev = kd_key_event();
        if (ev < 0) break;
        int pressed = !(ev & KD_KEY_RELEASED);
        unsigned char key = convertKey(ev & ~KD_KEY_RELEASED);
        if (key) addKeyToQueue(pressed, key);
    }
}

void DG_SleepMs(uint32_t ms)
{
    kd_sleep((int)ms);
}

uint32_t DG_GetTicksMs(void)
{
    return (uint32_t)kd_tick();
}

int DG_GetKey(int *pressed, unsigned char *doomKey)
{
    if (s_KeyQueueReadIndex == s_KeyQueueWriteIndex)
        return 0;
    unsigned short keyData = s_KeyQueue[s_KeyQueueReadIndex];
    s_KeyQueueReadIndex = (s_KeyQueueReadIndex + 1) % KEYQUEUE_SIZE;
    *pressed = keyData >> 8;
    *doomKey = keyData & 0xFF;
    return 1;
}

void DG_SetWindowTitle(const char *title)
{
    (void)title;
}

int main(int argc, char **argv)
{
    /* saves and the config file live in ~/.doom on the virtual drive */
    const char *home = getenv("HOME");
    if (!home) home = "/home/kid";
    char dir[256];
    snprintf(dir, sizeof dir, "%s/.doom", home);
    mkdir(dir, 0755);
    chdir(dir);

    const char *cart = getenv("CART");
    if (!cart) cart = "/games/doom";
    char wad[256];
    snprintf(wad, sizeof wad, "%s/freedoom1.wad", cart);

    int has_iwad = 0;
    for (int i = 1; i < argc; i++)
        if (strcmp(argv[i], "-iwad") == 0) has_iwad = 1;

    char *args[32];
    int n = 0;
    args[n++] = "doom";
    if (!has_iwad) {
        args[n++] = "-iwad";
        args[n++] = wad;
    }
    for (int i = 1; i < argc && n < 30; i++) args[n++] = argv[i];
    args[n] = NULL;

    doomgeneric_Create(n, args);
    for (;;) doomgeneric_Tick();
    return 0;
}
