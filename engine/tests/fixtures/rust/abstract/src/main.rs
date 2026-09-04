trait T {
    fn abs(&self) -> i32;

    fn dflt(&self) -> i32 {
        1
    }
}

struct A;
struct B;

impl T for A {
    fn abs(&self) -> i32 {
        10
    }

    fn dflt(&self) -> i32 {
        11
    }
}

impl T for B {
    fn abs(&self) -> i32 {
        20
    }

    fn dflt(&self) -> i32 {
        21
    }
}

fn pick(t: &dyn T) -> i32 {
    let x = t.abs();
    let y = t.dflt();
    x + y
}

fn main() {
    println!("{}", pick(&A));
}
