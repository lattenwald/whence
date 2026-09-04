struct A;
struct B;

impl A {
    fn get(&self) -> i32 {
        1
    }
}

impl B {
    fn get(&self) -> i32 {
        2
    }
}

fn show(n: i32) {
    let _ = n;
}

fn main() {
    let a = A;
    let v = a.get();
    show(v);
}
