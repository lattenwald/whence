package main

func g(x int) (int, int) {
	a := x
	b := x + 1
	return a, b
}

func show(n int) {
	_ = n
}

func main() {
	q, r := g(1)
	show(q + r)
}
