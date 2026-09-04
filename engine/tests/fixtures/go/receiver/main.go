package main

type S struct {
	n int
}

func (s *S) Bump(d int) {
	s.n += d
}

func (s S) Get() int {
	return s.n
}

func h(p *S) {
	p.n = 7
}

func show(v S) {
	_ = v
}

func main() {
	s := S{}
	s.Bump(1)
	s.Get()
	h(&s)
	show(s)
}
