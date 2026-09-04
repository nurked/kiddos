/* ROGUE - a small dungeon crawler in C, written against kiddos.h only.
 *
 * You are @. Arrows move. Walk into a monster to fight it. ! is a potion,
 * $ is gold, > is the stairs down. Find the stairs on each floor; every
 * floor is deeper and nastier. ESC quits, ? shows help.
 *
 * Build it yourself:   cp /games/rogue/rogue.c .   cc rogue.c   ./rogue.wasm
 * The whole game is this one file: rooms, corridors, monsters, and a loop
 * that waits for a key, moves everything once, and redraws.
 */
#include "kiddos.h"

#define W 78
#define H 21
#define MAXROOMS 9
#define MAXMON 24

static char map[H][W];      /* '#' wall, '.' floor, '+' door, '>' stairs, '!' potion, '$' gold */
static char seen[H][W];     /* 1 once the kid has been near it */
static int px, py, hp, maxhp, gold, depth, kills, potions;
static int monx[MAXMON], mony[MAXMON], monhp[MAXMON], monkind[MAXMON];
static int nmon;
static char msg[80];

static int rnd(int n) { return n <= 0 ? 0 : kd_random() % n; }
static int iabs(int v) { return v < 0 ? -v : v; }

static void say(const char *s) {
    int i = 0;
    for (; s[i] && i < 78; i++) msg[i] = s[i];
    msg[i] = 0;
}

/* --- building a floor -------------------------------------------------- */
static int rx[MAXROOMS], ry[MAXROOMS], rw[MAXROOMS], rh[MAXROOMS], nrooms;

static void carve_room(int x, int y, int w, int h) {
    for (int j = y; j < y + h; j++)
        for (int i = x; i < x + w; i++) map[j][i] = '.';
}

static void carve_corridor(int x1, int y1, int x2, int y2) {
    int x = x1, y = y1;
    while (x != x2) { map[y][x] = '.'; x += x2 > x ? 1 : -1; }
    while (y != y2) { map[y][x] = '.'; y += y2 > y ? 1 : -1; }
    map[y][x] = '.';
}

static int monster_for_depth(void) {
    int r = rnd(10);
    if (depth <= 1) return r < 7 ? 0 : 1;          /* rats and bats */
    if (depth <= 3) return r < 4 ? 1 : (r < 8 ? 2 : 3);
    return r < 3 ? 2 : (r < 7 ? 3 : 4);
}

static const char monchar[5] = {'r', 'b', 'g', 'o', 'D'};
static const char *monname[5] = {"a rat", "a bat", "a goblin", "an ogre", "a DRAGON"};
static const int monmaxhp[5] = {2, 3, 5, 9, 18};
static const int mondmg[5] = {1, 1, 2, 3, 5};
static const int moncolor[5] = {KD_GRAY, KD_LIGHTMAGENTA, KD_LIGHTGREEN, KD_LIGHTRED, KD_RED};

static void new_floor(void) {
    for (int j = 0; j < H; j++)
        for (int i = 0; i < W; i++) { map[j][i] = '#'; seen[j][i] = 0; }
    nrooms = 0;
    for (int tries = 0; tries < 60 && nrooms < MAXROOMS; tries++) {
        int w = 5 + rnd(10), h = 3 + rnd(4);
        int x = 1 + rnd(W - w - 2), y = 1 + rnd(H - h - 2);
        int ok = 1;
        for (int r = 0; r < nrooms; r++)
            if (x < rx[r] + rw[r] + 1 && x + w + 1 > rx[r] && y < ry[r] + rh[r] + 1 && y + h + 1 > ry[r]) ok = 0;
        if (!ok) continue;
        carve_room(x, y, w, h);
        if (nrooms > 0) {
            int cx = rx[nrooms - 1] + rw[nrooms - 1] / 2, cy = ry[nrooms - 1] + rh[nrooms - 1] / 2;
            carve_corridor(cx, cy, x + w / 2, y + h / 2);
        }
        rx[nrooms] = x; ry[nrooms] = y; rw[nrooms] = w; rh[nrooms] = h;
        nrooms++;
    }
    px = rx[0] + rw[0] / 2; py = ry[0] + rh[0] / 2;
    int last = nrooms - 1;
    map[ry[last] + rh[last] / 2][rx[last] + rw[last] / 2] = '>';
    for (int n = 0; n < 3 + depth; n++) {
        int r = rnd(nrooms);
        map[ry[r] + rnd(rh[r])][rx[r] + rnd(rw[r])] = '$';
    }
    for (int n = 0; n < 2; n++) {
        int r = rnd(nrooms);
        map[ry[r] + rnd(rh[r])][rx[r] + rnd(rw[r])] = '!';
    }
    nmon = 0;
    for (int r = 1; r < nrooms && nmon < MAXMON; r++) {
        int count = 1 + rnd(1 + depth / 2);
        for (int c = 0; c < count && nmon < MAXMON; c++) {
            monx[nmon] = rx[r] + rnd(rw[r]);
            mony[nmon] = ry[r] + rnd(rh[r]);
            monkind[nmon] = monster_for_depth();
            monhp[nmon] = monmaxhp[monkind[nmon]];
            nmon++;
        }
    }
    map[py][px] = '.';
}

/* --- drawing ------------------------------------------------------------ */
static void draw(void) {
    for (int j = 0; j < H; j++)
        for (int i = 0; i < W; i++)
            if (iabs(i - px) <= 8 && iabs(j - py) <= 5) seen[j][i] = 1;
    for (int j = 0; j < H; j++) {
        for (int i = 0; i < W; i++) {
            int y = j + 1;
            if (!seen[j][i]) { kd_put(i, y, ' ', KD_BLACK, KD_BLACK); continue; }
            int lit = iabs(i - px) <= 8 && iabs(j - py) <= 5;
            char c = map[j][i];
            int fg = lit ? KD_GRAY : KD_DARKGRAY;
            if (c == '#') fg = lit ? KD_BROWN : KD_DARKGRAY;
            if (c == '>') fg = KD_WHITE;
            if (c == '!') fg = KD_LIGHTCYAN;
            if (c == '$') fg = KD_YELLOW;
            kd_put(i, y, c, fg, KD_BLACK);
        }
    }
    for (int m = 0; m < nmon; m++)
        if (monhp[m] > 0 && seen[mony[m]][monx[m]] && iabs(monx[m] - px) <= 8 && iabs(mony[m] - py) <= 5)
            kd_put(monx[m], mony[m] + 1, monchar[monkind[m]], moncolor[monkind[m]], KD_BLACK);
    kd_put(px, py + 1, '@', KD_WHITE, KD_BLACK);
    /* status lines */
    char line[80]; int n = 0;
    const char *parts[] = {"Depth ", "  HP ", "/", "  Gold ", "  Potions ", "  Kills ", 0};
    int vals[] = {depth, hp, maxhp, gold, potions, kills};
    for (int k = 0; parts[k]; k++) {
        for (const char *s = parts[k]; *s; s++) line[n++] = *s;
        char b[8]; int v = vals[k], bi = 7; b[bi] = 0;
        do { b[--bi] = '0' + v % 10; v /= 10; } while (v);
        for (const char *s = b + bi; *s; s++) line[n++] = *s;
    }
    while (n < 78) line[n++] = ' ';
    line[78] = 0;
    kd_puts(0, 0, line, KD_LIGHTCYAN, KD_BLACK);
    int mi = 0;
    for (; msg[mi] && mi < 78; mi++) kd_put(mi, 23, msg[mi], KD_YELLOW, KD_BLACK);
    for (; mi < 78; mi++) kd_put(mi, 23, ' ', KD_BLACK, KD_BLACK);
    kd_puts(0, 24, "arrows move  q or ESC quits  ? help                                           ", KD_DARKGRAY, KD_BLACK);
}

/* --- the turn ------------------------------------------------------------ */
static int monster_at(int x, int y) {
    for (int m = 0; m < nmon; m++)
        if (monhp[m] > 0 && monx[m] == x && mony[m] == y) return m;
    return -1;
}

static void player_move(int dx, int dy) {
    int nx = px + dx, ny = py + dy;
    if (nx < 0 || ny < 0 || nx >= W || ny >= H) return;
    int m = monster_at(nx, ny);
    if (m >= 0) {
        int dmg = 1 + rnd(2 + depth / 2);
        monhp[m] -= dmg;
        if (monhp[m] <= 0) {
            kills++;
            kd_beep(880, 40);
            say("You defeated it!");
        } else {
            kd_beep(440, 30);
            say("You hit it.");
        }
        return;
    }
    char c = map[ny][nx];
    if (c == '#') return;
    px = nx; py = ny;
    if (c == '$') { int g = 5 + rnd(10) * depth; gold += g; map[ny][nx] = '.'; say("Gold!"); kd_beep(1200, 30); }
    if (c == '!') { potions++; map[ny][nx] = '.'; say("A potion. Press p to drink it when you are hurt."); }
    if (c == '>') say("Stairs down. Press > to descend.");
}

static void monsters_move(void) {
    for (int m = 0; m < nmon; m++) {
        if (monhp[m] <= 0) continue;
        int dx = px - monx[m], dy = py - mony[m];
        if (iabs(dx) > 9 || iabs(dy) > 6) continue;           /* asleep */
        if (iabs(dx) + iabs(dy) == 1) {                       /* adjacent: attack */
            int dmg = mondmg[monkind[m]] + rnd(2);
            hp -= dmg;
            kd_beep(200, 60);
            say(monname[monkind[m]]);
            for (int i = 0; msg[i]; i++) ; /* append */
            const char *tail = " bites you!";
            int i = 0; while (msg[i]) i++;
            for (int k = 0; tail[k] && i < 78; k++) msg[i++] = tail[k];
            msg[i] = 0;
            continue;
        }
        int sx = dx > 0 ? 1 : (dx < 0 ? -1 : 0), sy = dy > 0 ? 1 : (dy < 0 ? -1 : 0);
        int nx = monx[m], ny = mony[m];
        if (iabs(dx) > iabs(dy)) nx += sx; else ny += sy;
        if (map[ny][nx] != '#' && monster_at(nx, ny) < 0 && !(nx == px && ny == py)) { monx[m] = nx; mony[m] = ny; }
        else {
            nx = monx[m]; ny = mony[m];
            if (iabs(dx) > iabs(dy)) ny += sy; else nx += sx;
            if (map[ny][nx] != '#' && monster_at(nx, ny) < 0 && !(nx == px && ny == py)) { monx[m] = nx; mony[m] = ny; }
        }
    }
}

static void help(void) {
    kd_clear(KD_BLACK);
    kd_puts(2, 2, "ROGUE", KD_YELLOW, KD_BLACK);
    kd_puts(2, 4, "You are @ in a dungeon. Arrows move. Walk into a monster to fight it.", KD_GRAY, KD_BLACK);
    kd_puts(2, 5, "$ gold   ! potion (press p to drink)   > stairs (press > to go down)", KD_GRAY, KD_BLACK);
    kd_puts(2, 6, "r rat  b bat  g goblin  o ogre  D dragon. Deeper floors are worse.", KD_GRAY, KD_BLACK);
    kd_puts(2, 8, "This whole game is one C file: cat /games/rogue/rogue.c", KD_LIGHTCYAN, KD_BLACK);
    kd_puts(2, 10, "Press any key.", KD_WHITE, KD_BLACK);
    kd_readkey();
    kd_clear(KD_BLACK);
}

int main(void) {
    kd_cursor_show(0);
    kd_clear(KD_BLACK);
    hp = maxhp = 12; gold = 0; depth = 1; kills = 0; potions = 0;
    new_floor();
    say("Welcome to the dungeon. Find the > on each floor. ? for help.");
    for (;;) {
        draw();
        int k = kd_readkey();
        if (k == KD_KEY_ESC || k == 'q') break;
        if (k == '?') { help(); continue; }
        if (k == KD_KEY_UP || k == 'k') player_move(0, -1);
        else if (k == KD_KEY_DOWN || k == 'j') player_move(0, 1);
        else if (k == KD_KEY_LEFT || k == 'h') player_move(-1, 0);
        else if (k == KD_KEY_RIGHT || k == 'l') player_move(1, 0);
        else if (k == 'p') {
            if (potions > 0) { potions--; hp = maxhp; say("You feel better."); kd_beep(660, 80); }
            else say("You have no potion.");
        } else if (k == '>') {
            if (map[py][px] == '>') {
                depth++; maxhp += 2; hp = maxhp;
                new_floor();
                say("You climb down. It is darker here.");
                kd_beep(330, 100);
                kd_clear(KD_BLACK);
            } else say("There are no stairs here.");
        } else continue;
        monsters_move();
        if (hp <= 0) {
            draw();
            kd_puts(24, 11, "   YOU DIED. Press a key.   ", KD_WHITE, KD_RED);
            kd_speak("You died");
            kd_readkey();
            break;
        }
    }
    kd_clear(KD_BLACK);
    kd_cursor_show(1);
    kd_print("You left the dungeon with ");
    kd_print_int(gold);
    kd_print(" gold, ");
    kd_print_int(kills);
    kd_print(" kills, depth ");
    kd_print_int(depth);
    kd_print(".\n");
    return 0;
}
