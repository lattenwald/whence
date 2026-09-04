struct P {
    x: i32,
    y: i32,
}

struct Q {
    x: i32,
}

struct E;

trait Shape {
    fn abs(&self) -> i32;

    fn dflt(&self) -> i32 {
        7
    }
}

impl P {
    fn bump(&mut self, d: i32) -> i32 {
        self.x += d;
        self.x
    }

    fn get(&self) -> i32 {
        self.y
    }

    fn consume(mut self) -> i32 {
        self.x = 0;
        self.x
    }
}

impl Q {
    fn get(&self) -> i32 {
        self.x
    }
}

fn base() -> P {
    P { x: 0, y: 0 }
}

fn split(n: i32, b: &mut Vec<i32>) -> (i32, i32) {
    b.push(n);
    (n, n + 1)
}

fn tail(x: i32) -> i32 {
    x
}

fn tail_return(a: i32) -> i32 {
    return a
}

fn hold(e: &mut i32) {
    *e += 1;
}

fn maybe() -> Option<i32> {
    Some(1)
}

fn run(p: P, b: &mut Vec<i32>, flag: bool, n: Result<i32, E>) -> Result<(i32, i32), E> {
    let mut v = 1;
    v = v + 1;
    let (q, r) = split(1, b);
    let P { x, y: yy } = P { x: 10, y: 20 };
    let mut w = vec![];
    w.push(v);
    *b = w;
    b.push(v);
    let m = if flag { p.x } else { yy };
    let s = match r {
        0 => q,
        _ => x,
    };
    let z = n?;
    let mut c = 0;
    for i in 0..3 {
        c += i;
    }
    if let Some(k) = maybe() {
        c += k;
    }
    let f = |t| t + 1;
    let mut e;
    e = 5;
    hold(&mut e);
    let (a, .., zz) = (1, 2, 3);
    let u = P { x: 3, ..base() };
    let _ = (s, z, c, e, f(1), u, tail(m), tail_return(a), zz, Q::get(&Q { x: 1 }));
    return (m, p.y);
}
