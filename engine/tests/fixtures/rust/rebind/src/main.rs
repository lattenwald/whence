fn show(x: i32) {
    println!("{x}");
}

fn main() {
    let a = 1;
    let mut v = a;
    v = v + 1;
    v += 2;
    show(v);
}
