fn pair(x: i32) -> (i32, i32) {
    let a = x;
    let b = x + 1;
    (a, b)
}

fn main() {
    let (q, r) = pair(1);
    let s = q + r;
    println!("{s}");
}
