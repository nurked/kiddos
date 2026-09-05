// Package kiddos is the whole machine, as seen from Go.
//
// A KidDOS Go program has no operating system underneath. It has this
// package: the screen, the keys, the clock, sound, voice and your files.
//
//	package main
//
//	import "kiddos"
//
//	func main() {
//	    kiddos.Print("Hello from Go!\n")
//	}
//
// Compile with:   goc hello.go        run with:   ./hello.wasm
package kiddos

import "unsafe"

//go:wasmimport kiddos print
func printRaw(ptr unsafe.Pointer, n int32)

//go:wasmimport kiddos eprint
func eprintRaw(ptr unsafe.Pointer, n int32)

//go:wasmimport kiddos put
func putRaw(x, y, ch, fg, bg int32)

//go:wasmimport kiddos cursor
func cursorRaw(x, y int32)

//go:wasmimport kiddos cursor_show
func cursorShowRaw(v int32)

//go:wasmimport kiddos clear
func clearRaw(bg int32)

//go:wasmimport kiddos color
func colorRaw(fg, bg int32)

//go:wasmimport kiddos size
func sizeRaw() int32

//go:wasmimport kiddos getkey
func getkeyRaw() int32

//go:wasmimport kiddos readkey
func readkeyRaw() int32

//go:wasmimport kiddos readline
func readlineRaw(buf unsafe.Pointer, cap int32) int32

//go:wasmimport kiddos sleep
func sleepRaw(ms int32)

//go:wasmimport kiddos tick
func tickRaw() int64

//go:wasmimport kiddos beep
func beepRaw(freq, ms int32)

//go:wasmimport kiddos speak
func speakRaw(ptr unsafe.Pointer, n int32) int32

//go:wasmimport kiddos random
func randomRaw() int32

//go:wasmimport kiddos exit
func exitRaw(code int32)

//go:wasmimport kiddos fs_read
func fsReadRaw(path unsafe.Pointer, plen int32, buf unsafe.Pointer, cap int32) int32

//go:wasmimport kiddos fs_write
func fsWriteRaw(path unsafe.Pointer, plen int32, data unsafe.Pointer, dlen int32, append int32) int32

//go:wasmimport kiddos gfx_mode
func gfxModeRaw(on int32)

//go:wasmimport kiddos gfx_clear
func gfxClearRaw(color int32)

//go:wasmimport kiddos gfx_pixel
func gfxPixelRaw(x, y, color int32)

//go:wasmimport kiddos gfx_get
func gfxGetRaw(x, y int32) int32

//go:wasmimport kiddos gfx_line
func gfxLineRaw(x1, y1, x2, y2, color int32)

//go:wasmimport kiddos gfx_rect
func gfxRectRaw(x, y, w, h, color int32)

//go:wasmimport kiddos gfx_fill
func gfxFillRaw(x, y, w, h, color int32)

//go:wasmimport kiddos gfx_circle
func gfxCircleRaw(x, y, r, color, filled int32)

//go:wasmimport kiddos gfx_blit
func gfxBlitRaw(x, y, w, h int32, pixels unsafe.Pointer, transparent int32)

//go:wasmimport kiddos gfx_read
func gfxReadRaw(x, y, w, h int32, out unsafe.Pointer) int32

//go:wasmimport kiddos gfx_palette
func gfxPaletteRaw(index, r, g, b int32)

//go:wasmimport kiddos gfx_text
func gfxTextRaw(x, y int32, s unsafe.Pointer, n int32, fg, bg int32) int32

//go:wasmimport kiddos gfx_flip
func gfxFlipRaw()

//go:wasmimport kiddos key_down
func keyDownRaw(key int32) int32

//go:wasmimport kiddos key_event
func keyEventRaw() int32

// Keys, as returned by GetKey and ReadKey. Letters come back as themselves.
const (
	KeyEnter = 0x110001
	KeyBS    = 0x110002
	KeyTab   = 0x110003
	KeyEsc   = 0x110004
	KeyUp    = 0x110005
	KeyDown  = 0x110006
	KeyLeft  = 0x110007
	KeyRight = 0x110008
	KeyHome  = 0x110009
	KeyEnd   = 0x11000A
	KeyPgUp  = 0x11000B
	KeyPgDn  = 0x11000C
	KeyIns   = 0x11000D
	KeyDel   = 0x11000E
	KeySpace = ' '
)

// Colors, the same numbers as BASIC's COLOR.
const (
	Black = iota
	Blue
	Green
	Cyan
	Red
	Magenta
	Brown
	Gray
	DarkGray
	LightBlue
	LightGreen
	LightCyan
	LightRed
	LightMagenta
	Yellow
	White
)

func strPtr(s string) (unsafe.Pointer, int32) {
	if len(s) == 0 {
		return nil, 0
	}
	return unsafe.Pointer(unsafe.StringData(s)), int32(len(s))
}

// Print writes text at the cursor, like fmt.Print would.
func Print(s string) { p, n := strPtr(s); printRaw(p, n) }

// Println writes text and a new line.
func Println(s string) { Print(s); Print("\n") }

// PrintInt writes a number.
func PrintInt(v int) { Print(Itoa(v)) }

// Itoa turns a number into text (there is no strconv here).
func Itoa(v int) string {
	if v == 0 {
		return "0"
	}
	neg := v < 0
	if neg {
		v = -v
	}
	var b [12]byte
	i := len(b)
	for v > 0 {
		i--
		b[i] = byte('0' + v%10)
		v /= 10
	}
	if neg {
		i--
		b[i] = '-'
	}
	return string(b[i:])
}

// Put draws text at a cell without moving the cursor. Colors 0-15.
func Put(x, y int, s string, fg, bg int) {
	for i, r := range []rune(s) {
		putRaw(int32(x+i), int32(y), int32(r), int32(fg), int32(bg))
	}
}

func Cursor(x, y int)       { cursorRaw(int32(x), int32(y)) }
func CursorShow(v bool)     { if v { cursorShowRaw(1) } else { cursorShowRaw(0) } }
func Clear(bg int)          { clearRaw(int32(bg)) }
func Color(fg, bg int)      { colorRaw(int32(fg), int32(bg)) }
func Cols() int             { return int(sizeRaw() >> 16) }
func Rows() int             { return int(sizeRaw() & 0xFFFF) }
func GetKey() int           { return int(getkeyRaw()) } // -1 if nothing is pressed
func ReadKey() int          { return int(readkeyRaw()) } // waits
func Sleep(ms int)          { sleepRaw(int32(ms)) }
func Tick() int64           { return tickRaw() }
func Beep(freq, ms int)     { beepRaw(int32(freq), int32(ms)) }
func Speak(s string) bool   { p, n := strPtr(s); return speakRaw(p, n) != 0 }
func Random(n int) int      { if n <= 0 { return 0 }; return int(randomRaw()) % n }
func Exit(code int)         { exitRaw(int32(code)) }

// ReadLine waits for a line of typing. ok is false at end of input.
func ReadLine() (line string, ok bool) {
	var buf [256]byte
	n := readlineRaw(unsafe.Pointer(&buf[0]), int32(len(buf)))
	if n < 0 {
		return "", false
	}
	return string(buf[:n]), true
}

// ReadFile returns a file's text, or "" and false.
func ReadFile(path string) (string, bool) {
	var buf [65536]byte
	p, n := strPtr(path)
	got := fsReadRaw(p, n, unsafe.Pointer(&buf[0]), int32(len(buf)))
	if got < 0 {
		return "", false
	}
	return string(buf[:got]), true
}

// WriteFile replaces a file's contents; AppendFile adds to the end.
func WriteFile(path, data string) bool {
	p, n := strPtr(path)
	d, dn := strPtr(data)
	return fsWriteRaw(p, n, d, dn, 0) == 0
}

func AppendFile(path, data string) bool {
	p, n := strPtr(path)
	d, dn := strPtr(data)
	return fsWriteRaw(p, n, d, dn, 1) == 0
}

// Pixel mode: 320 x 200 pixels, 256 colors, double-buffered.
//
// GfxMode(true) switches the screen to pixels (the text stays underneath
// and comes back with GfxMode(false) or when the program ends). Drawing
// goes to a hidden buffer; GfxFlip shows it. Colors are palette numbers:
// 0-15 the usual colors, 16-31 grays, 32-247 a color cube (see RGB), and
// GfxPalette can change any entry.
const (
	GfxW = 320
	GfxH = 200
	// KeyEvent sets this bit when the key went up rather than down.
	KeyReleased = 0x1000000
)

// RGB gives the palette number for red, green and blue levels 0..5.
func RGB(r, g, b int) int { return 32 + 36*r + 6*g + b }

// Gray gives the palette number for a gray level 0..15.
func Gray(v int) int { return 16 + v }

func GfxMode(on bool)                       { if on { gfxModeRaw(1) } else { gfxModeRaw(0) } }
func GfxClear(color int)                    { gfxClearRaw(int32(color)) }
func GfxPixel(x, y, color int)              { gfxPixelRaw(int32(x), int32(y), int32(color)) }
func GfxGet(x, y int) int                   { return int(gfxGetRaw(int32(x), int32(y))) }
func GfxLine(x1, y1, x2, y2, color int)     { gfxLineRaw(int32(x1), int32(y1), int32(x2), int32(y2), int32(color)) }
func GfxRect(x, y, w, h, color int)         { gfxRectRaw(int32(x), int32(y), int32(w), int32(h), int32(color)) }
func GfxFill(x, y, w, h, color int)         { gfxFillRaw(int32(x), int32(y), int32(w), int32(h), int32(color)) }
func GfxCircle(x, y, r, color int, filled bool) {
	f := int32(0)
	if filled {
		f = 1
	}
	gfxCircleRaw(int32(x), int32(y), int32(r), int32(color), f)
}
func GfxPalette(index, r, g, b int) { gfxPaletteRaw(int32(index), int32(r), int32(g), int32(b)) }
func GfxFlip()                      { gfxFlipRaw() }

// GfxBlit copies a w x h block of palette numbers (w per row) to x, y.
// Pixels equal to transparent are skipped; pass -1 to copy everything.
func GfxBlit(x, y, w, h int, pixels []byte, transparent int) {
	if len(pixels) == 0 {
		return
	}
	gfxBlitRaw(int32(x), int32(y), int32(w), int32(h), unsafe.Pointer(&pixels[0]), int32(transparent))
}

// GfxRead copies a w x h block out of the drawing buffer.
func GfxRead(x, y, w, h int) []byte {
	if w <= 0 || h <= 0 {
		return nil
	}
	out := make([]byte, w*h)
	gfxReadRaw(int32(x), int32(y), int32(w), int32(h), unsafe.Pointer(&out[0]))
	return out
}

// GfxText draws s with the 8x8 font; bg -1 keeps the background. Returns
// the x after the last letter.
func GfxText(x, y int, s string, fg, bg int) int {
	p, n := strPtr(s)
	return int(gfxTextRaw(int32(x), int32(y), p, n, int32(fg), int32(bg)))
}

// KeyDown reports whether a key is held right now (games hold a direction).
func KeyDown(key int) bool { return keyDownRaw(int32(key)) != 0 }

// KeyEvent returns the next key down or up (KeyReleased bit set), or -1.
func KeyEvent() int { return int(keyEventRaw()) }
