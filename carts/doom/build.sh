#!/bin/sh
# build.sh: make dist/doom.kdc, the Doom cartridge.
#
#   carts/doom/build.sh <wasi-sdk-dir> [out-dir]
#
# Needs a full wasi-sdk (for wasi-libc), curl, unzip, zip. Downloads
# doomgeneric (pinned) and Freedoom 0.13.0 into build/doom/ on first use.
set -eu
SDK="$1"; OUT="${2:-dist}"
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
WORK="$ROOT/build/doom"
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
DG_REV=dcb7a8dbc7a16ce3dda29382ac9aae9d77d21284
FREEDOOM=0.13.0
mkdir -p "$WORK" "$OUT"

if [ ! -d "$WORK/doomgeneric" ]; then
  git clone -q https://github.com/ozkl/doomgeneric.git "$WORK/doomgeneric"
  (cd "$WORK/doomgeneric" && git checkout -q "$DG_REV")
fi
if [ ! -f "$WORK/freedoom1.wad" ]; then
  curl -sSL -o "$WORK/freedoom.zip" "https://github.com/freedoom/freedoom/releases/download/v$FREEDOOM/freedoom-$FREEDOOM.zip"
  (cd "$WORK" && unzip -q -o freedoom.zip && cp "freedoom-$FREEDOOM/freedoom1.wad" "freedoom-$FREEDOOM/COPYING.txt" .)
fi

SRC="$WORK/doomgeneric/doomgeneric"
CC="$SDK/bin/clang"
CFLAGS="--target=wasm32-wasip1 --sysroot=$SDK/share/wasi-sysroot -O2 -DCMAP256 -DDOOMGENERIC_RESX=320 -DDOOMGENERIC_RESY=200 -DNORMALUNIX -DLINUX -D_DEFAULT_SOURCE -Wno-everything -I$SRC -I$ROOT/content/factory-drive/usr/include"
OBJ="$WORK/obj"; mkdir -p "$OBJ"
FILES="dummy am_map doomdef doomstat dstrings d_event d_items d_iwad d_loop d_main d_mode d_net f_finale f_wipe g_game hu_lib hu_stuff info i_cdmus i_endoom i_joystick i_scale i_sound i_system i_timer memio m_argv m_bbox m_cheat m_config m_controls m_fixed m_menu m_misc m_random p_ceilng p_doors p_enemy p_floor p_inter p_lights p_map p_maputl p_mobj p_plats p_pspr p_saveg p_setup p_sight p_spec p_switch p_telept p_tick p_user r_bsp r_data r_draw r_main r_plane r_segs r_sky r_things sha1 sounds statdump st_lib st_stuff s_sound tables v_video wi_stuff w_checksum w_file w_main w_wad z_zone w_file_stdc i_input i_video doomgeneric"
OBJS=""
for f in $FILES; do
  "$CC" $CFLAGS -c "$SRC/$f.c" -o "$OBJ/$f.o"
  OBJS="$OBJS $OBJ/$f.o"
done
"$CC" $CFLAGS -c "$HERE/doomgeneric_kiddos.c" -o "$OBJ/doomgeneric_kiddos.o"
"$CC" $CFLAGS -Wl,-z,stack-size=1048576 -o "$WORK/doom.wasm" $OBJS "$OBJ/doomgeneric_kiddos.o"

STAGE="$WORK/stage"; rm -rf "$STAGE"; mkdir -p "$STAGE/man"
cp "$HERE/cart.toml" "$HERE/README.md" "$STAGE/"
cp "$HERE/man/doom.md" "$STAGE/man/"
cp "$WORK/doom.wasm" "$WORK/freedoom1.wad" "$STAGE/"
cp "$WORK/COPYING.txt" "$STAGE/FREEDOOM-LICENSE.txt"
rm -f "$OUT/doom.kdc"
(cd "$STAGE" && zip -q -r "$OUT/doom.kdc" .)
ls -la "$OUT/doom.kdc" "$WORK/doom.wasm"
