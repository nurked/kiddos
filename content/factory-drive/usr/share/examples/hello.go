// Your first Go program. Copy it home, then:   goc hello.go   and   ./hello.wasm
package main

import "kiddos"

func main() {
	kiddos.Println("Hello from Go!")
	kiddos.Print("The screen is ")
	kiddos.PrintInt(kiddos.Cols())
	kiddos.Print(" by ")
	kiddos.PrintInt(kiddos.Rows())
	kiddos.Println(" letters.")
}
