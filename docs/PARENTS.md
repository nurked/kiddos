# READ THIS BEFORE YOU HAND IT TO A CHILD

## ⚠ THE WARNING, IN BIG LETTERS

**KIDDOS TAKES OVER THE WHOLE SCREEN AND IS DESIGNED TO BE HARD TO LEAVE.**

**THERE IS NO CLOSE BUTTON, NO MENU BAR, AND CMD-Q / ALT-F4 DO NOTHING.**

**LEARN THE WAY OUT (BELOW) BEFORE YOU START IT, AND SET THE PARENT PASSWORD YOURSELF ON THE FIRST RUN.**

**IT IS NOT A LOCK. A DETERMINED CHILD CAN STILL FORCE-QUIT IT (SEE "IF ALL ELSE FAILS"). IT IS MEANT TO BE HARD TO LEAVE BY ACCIDENT, NOT IMPOSSIBLE TO LEAVE ON PURPOSE.**

What it does *not* do: it never touches your files (the child's whole
world is one file inside KidDOS's own folder), it has no internet at
all, and it runs no program outside its own sandbox. It does use your
computer's voice (the `speak` command) and speaker.

---

## HOW TO START IT

### macOS
1. Download `KidDOS-macos.zip` (one app for Apple silicon and Intel),
   unzip, you get `KidDOS.app`. Drag it to Applications if you like.
2. Double-click it. The app is signed and notarized with Apple, so
   there is nothing to bypass. (If a dialog still appears, right-click
   the app and choose Open.)
3. It opens full screen. Type `hi` and press Enter.

### Windows
1. Download `KidDOS-windows-x86_64.zip`, unzip, run `kiddos.exe`.
2. Windows SmartScreen will complain because the file is unsigned:
   click **More info**, then **Run anyway**.

### Linux
1. Download `KidDOS-linux-x86_64.tar.gz` (or `-aarch64` for a Raspberry
   Pi 4/5 with a 64-bit OS), unpack, `chmod +x kiddos`, run it.
2. It needs ALSA, and X11 or Wayland: on Debian/Ubuntu
   `sudo apt install libasound2 libxkbcommon0`.

### To try it without full screen
Run it from a terminal with `KIDDOS_WINDOWED=1` in the environment. Good
for a first look. Do not give a child the windowed version: they can
click out of it.

---

## SET THE PARENT PASSWORD (FIRST RUN, YOU, NOT THE CHILD)

1. Start KidDOS. At the prompt type `parent` and press Enter.
2. It says *No parent password yet. Choose one:* — type a password (it
   is not shown), Enter, then the same again, Enter.
3. You are now in parent mode: the prompt ends with `#`. Type `exit` to
   go back to the child's prompt.

Do this before the child ever sits down. Whoever types `parent` first
picks the password. The password is stored hashed in KidDOS's folder on
your computer, never inside the child's drive.

Forgot it? Delete the file `parent.hash` in the KidDOS folder (paths
below); the next `parent` asks for a new one.

---

## HOW TO EXIT

Parent mode is the way out, and it works even when a program is
running.

1. Press **Ctrl + Alt + Shift + P** (on a Mac, **Cmd + Alt + Shift + P**
   also works). This interrupts whatever is running and types `parent`
   for you. Or, at a prompt, just type `parent`.
2. Enter your password.
3. Now type one of:
   - `shutdown` — saves everything and quits KidDOS.
   - `exit-fullscreen` — turns it into a normal window you can close or
     move aside (`fullscreen` puts it back).
   - `exit` — leaves parent mode and gives the machine back to the child.

Five wrong passwords lock `parent` for five minutes. Every entry into
parent mode is written to the log (`log` shows it).

### If all else fails
- **macOS**: Cmd + Option + Esc opens Force Quit; pick KidDOS.
- **Windows**: Ctrl + Alt + Del, Task Manager, end `kiddos.exe`.
- **Linux**: Ctrl + Alt + F3 for a text console, or your desktop's
  force-quit.

Nothing is lost: the child's drive is saved every half second and on
every clean shutdown.

---

## USEFUL PARENT COMMANDS

| command | does |
|---|---|
| `set-name Sam` | the name the machine greets (or the child types `hi` and is asked) |
| `reset-drive` | wipes the child's drive back to new (asks for `yes`) |
| `log` | what happened: lessons done, games played, parent-mode entries |
| `carts`, `install`, `uninstall`, `share` | game cartridges in and out (see docs/cartridges) |
| `packs`, `install-pack` | compilers for C and Go (see docs/PACKS.md) |
| `passwd` | change the parent password |
| `crt off` | flat screen instead of the curved-TV look (the child can type this too) |

## WHERE THINGS LIVE ON YOUR COMPUTER

| OS | folder |
|---|---|
| macOS | `~/Library/Application Support/KidDOS/` |
| Windows | `%APPDATA%\KidDOS\` |
| Linux | `~/.local/share/kiddos/` |

Inside: `drive.kdd` (the child's whole world, one file; copy it to back
it up), `config.toml`, `parent.hash`, `log.txt`, and the `carts/` and
`packs/` folders. That is everything KidDOS ever writes.
