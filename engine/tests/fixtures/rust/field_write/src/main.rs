struct P {
    x: i32,
    y: i32,
}

fn show(n: i32) {
    let _ = n;
}

fn main() {
    let mut p = P { x: 1, y: 2 };
    p.x = 9;
    p.y = 3;
    let v = p.x;
    show(v);
}
