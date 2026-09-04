fn fill(v: &mut Vec<i32>) {
    v.push(1);
}

fn count(v: &Vec<i32>) -> usize {
    v.len()
}

fn run(v: &mut Vec<i32>) -> usize {
    fill(v);
    count(v);
    v.capacity()
}

fn main() {
    let mut xs = vec![1];
    let n = run(&mut xs);
    println!("{n}");
}
