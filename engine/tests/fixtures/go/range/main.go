package main

func show(n int) {
	_ = n
}

func main() {
	xs := []int{1, 2, 3}
	for _, e := range xs {
		show(e)
	}
}
