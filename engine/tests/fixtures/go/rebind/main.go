package main

func show(n int) {
	_ = n
}

func main() {
	a := 1
	v := a
	v = v + 1
	v++
	show(v)
}
