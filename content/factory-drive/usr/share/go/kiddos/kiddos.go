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
