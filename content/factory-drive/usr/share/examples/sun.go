// Sun: a sunset drawn in pixel mode. Any key ends it.
//     goc sun.go      ./sun.wasm
package main

import "kiddos"

func main() {
	for y := 0; y < kiddos.GfxH; y++ {
		// sky: blue at the top fading to orange at the horizon
		level := y * 6 / kiddos.GfxH
		kiddos.GfxLine(0, y, kiddos.GfxW-1, y, kiddos.RGB(level, 2, 5-level))
	}
	kiddos.GfxCircle(160, 140, 40, kiddos.Yellow, true)
	kiddos.GfxFill(0, 150, kiddos.GfxW, 50, kiddos.RGB(0, 1, 0))
	kiddos.GfxText(8, 8, "a sunset from Go", kiddos.White, -1)
	kiddos.GfxFlip()
	kiddos.ReadKey()
}
