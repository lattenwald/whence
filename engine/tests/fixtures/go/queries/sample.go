package sample

import "fmt"

type S struct {
	X int
	Y int
}

type Abser interface {
	Abs() int
}

func (s *S) Bump(d int) int {
	s.X += d
	return s.X
}

func (s S) Get() int {
	return s.Y
}

func g(a int) (int, int) {
	return a, a + 1
}

func two(a, b int, rest ...int) int {
	return a + b + len(rest)
}

func h(p *int, q *S) {
	*p = 1
	q.X = 2
}

func named(a int) (n int, err error) {
	if a > 0 {
		n = a
		return
	}
	return
}

func run(a int, m map[string]int, xs []int, i Abser) int {
	v := a
	v = v + 1
	v++
	var w int = v
	var z int
	q, r := g(v)
	x, ok := m["k"]
	if y, ok2 := m["j"]; ok2 {
		v += y
	}
	s := S{X: 1, Y: 2}
	s.X = 3
	p := &s
	h(&v, p)
	p.Bump(1)
	xs[0] = v
	c := 0
	for k, e := range xs {
		c += k + e
	}
	fn := func(t int) int { return t + 1 }
	go fn(1)
	defer fmt.Println(c)
	switch v {
	case 0:
		c = 1
	default:
		c = 2
	}
	t := (v)
	u := S{X: q}
	fmt.Println(w, z, r, x, ok, s, u, t, i.Abs())
	return fn(v)
}
