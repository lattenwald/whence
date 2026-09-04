package main

type I interface {
	Abs() int
}

type A struct{}

type B struct{}

func (a A) Abs() int {
	return 10
}

func (b B) Abs() int {
	return 20
}

func use(i I) int {
	v := i.Abs()
	return v
}

func main() {
	_ = use(A{})
}
