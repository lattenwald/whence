trait Ext {
    fn bump(&mut self, d: usize);
    fn peek(&self) -> usize;
}

impl Ext for String {
    fn bump(&mut self, d: usize) {
        self.push_str(&d.to_string());
    }

    fn peek(&self) -> usize {
        self.capacity()
    }
}

fn mutate(s: &mut String) {
    s.push('x');
}

fn show(s: &str) -> usize {
    s.len()
}

fn main() {
    let mut s = String::from("a");
    mutate(&mut s);
    s.bump(1);
    s.peek();
    s.len();
    show(&s);
}
