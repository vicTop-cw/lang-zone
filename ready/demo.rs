use std::collections::HashMap;

// ── Lang-Zong 构建块 prelude ──
pub trait BuildParams { type Args; fn into_args(self) -> Self::Args; }
pub struct IterStopException;
#[derive(Clone)]
pub enum __Pack {
    Tuple(Vec<*const ()>),
    Dict(std::collections::HashMap<String, *const ()>),
    Single(*const ()),
}

// ── defer guard (LIFO drop-order) ──
struct DeferGuard<F: FnMut()>(Option<F>);
impl<F: FnMut()> Drop for DeferGuard<F> {
    fn drop(&mut self) { if let Some(mut f) = self.0.take() { f(); } }
}

const PI: f64 = 3.14;

const MAX_SIZE: i64 = 100;

trait Drawable {
    fn draw(&mut self);
}

#[derive(Clone)]
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn distance(&mut self, mut other: Point) -> f64 {
        let mut dx = ((self.x - other.x)).clone();
        let mut dy = ((self.y - other.y)).clone();
        (((dx * dx) + (dy * dy))).sqrt()
    }

    fn translate(&mut self, mut dx: f64, mut dy: f64) -> Point {
        Point { x: ((self.x + dx)).clone(), y: ((self.y + dy)).clone() }
    }

}

#[derive(Clone)]
struct Circle {
    x: f64,
    y: f64,
    radius: f64,
}

impl Circle {
    fn area(&mut self) -> f64 {
        ((PI * self.radius) * self.radius)
    }

}

#[derive(Clone)]
enum Shape {
    Circle_(f64, f64, f64),
    Rect(f64, f64),
    Square(f64),
}

#[derive(Clone)]
struct Person {
    name: String,
    tag: Option<i64>,
}

impl Drawable for Circle {
    fn draw(&mut self) {
        println!("Circle at ({}, {})", self.x, self.y)
    }

}

fn add(mut a: i64, mut b: i64) -> i64 {
    (a + b)
}


fn greet(mut name: String) {
    println!("Hello, {}", name)
}


fn power(mut base: i64, mut exp: i64) -> i64 {
    if (exp == 0) {
        1
    } else {
        (base * power((base).clone(), ((exp - 1)).clone()))
    }
}


fn fib(mut n: i64) -> i64 {
    if (n <= 1) {
        n
    } else {
        (fib(((n - 1)).clone()) + fib(((n - 2)).clone()))
    }
}


fn factorial(mut n: i64) -> i64 {
    if (n <= 1) {
        1
    } else {
        (n * factorial(((n - 1)).clone()))
    }
}


fn identity<T>(mut x: T) -> T {
    x
}


fn safe_divide(mut a: i64, mut b: i64) -> Option<i64> {
    if (b == 0) {
        None
    } else {
        Some(((a / b)).clone())
    }
}


fn safe_sqrt(mut x: f64) -> Option<f64> {
    if !((x >= 0.0)) {
        return None;
    }
    Some(((x).sqrt()).clone())
}


fn shape_area(mut s: Shape) -> f64 {
    match s {
        Shape::Circle_(_, _, r) => ((PI * r) * r),
        Shape::Rect(w, h) => (w * h),
        Shape::Square(side) => (side * side),
    }
}


fn classify(mut n: i64) -> String {
    if (n > 0) {
        "positive".to_string()
    } else if (n < 0) {
        "negative".to_string()
    } else {
        "zero".to_string()
    }
}


fn sum_range(mut start: i64, mut end: i64) -> i64 {
    let mut total = 0;
    for mut i in start..end {
        total += i;
    }
    total
}


fn countdown(mut n: i64) {
    let mut i = (n).clone();
    while (i > 0) {
        println!("  {}...", i);
        i -= 1;
    }
    println!("  Liftoff!")
}


fn pipe_double(mut x: i64) -> i64 {
    (x * 2)
}


fn pipe_add_one(mut x: i64) -> i64 {
    (x + 1)
}


fn pipe_add3(mut a: i64, mut b: i64, mut c: i64) -> i64 {
    ((a + b) + c)
}


fn person_name(mut p: Option<Person>) -> String {
    ((p).map(|x| x.name)).unwrap_or("anonymous".to_string())
}


fn greet_person(mut p: Option<Person>) -> String {
    let Some(pp) = p else {
        return "no person".to_string();
    };
    let Some(t) = pp.tag else {
        return format!("hi {}", pp.name);
    };
    format!("hi {} (tag {})", pp.name, t)
}


fn consume_person(mut p: Person) -> String {
    format!("owned: {}", p.name)
}


fn xor_demo(mut a: i64, mut b: i64) -> i64 {
    (a ^ b)
}


fn main() {
    println!("=== Lang-Zong Demo (Phase 2 + 3) ===");
    println!("add(3, 4) = {}", add(3, 4));
    println!("power(2, 10) = {}", power(2, 10));
    println!("fib(10) = {}", fib(10));
    println!("factorial(5) = {}", factorial(5));
    println!("identity(99) = {}", identity(99));
    greet("World".to_string());
    println!("classify(5) = {}", classify(5));
    println!("classify(-3) = {}", classify(((-3)).clone()));
    println!("classify(0) = {}", classify(0));
    println!("sum_range(1, 100) = {}", sum_range(1, 100));
    println!("Countdown:");
    countdown(3);
    let mut p1 = (Point { x: 0.0, y: 0.0 }).clone();
    let mut p2 = (Point { x: 3.0, y: 4.0 }).clone();
    let mut d = (p1.distance(p2)).clone();
    println!("distance = {}", d);
    let mut p3 = (p1.translate(1.0, 2.0)).clone();
    println!("translated = ({}, {})", p3.x, p3.y);
    let mut c = (Circle { x: 0.0, y: 0.0, radius: 5.0 }).clone();
    println!("circle area = {}", c.area());
    c.draw();
    let mut s1 = (Shape::Circle_(0.0, 0.0, 3.0)).clone();
    let mut s2 = (Shape::Rect(4.0, 5.0)).clone();
    let mut s3 = (Shape::Square(6.0)).clone();
    println!("circle area = {}", shape_area((s1).clone()));
    println!("rect area = {}", shape_area((s2).clone()));
    println!("square area = {}", shape_area((s3).clone()));
    match safe_divide(10, 2) {
        Some(v) => println!("10 / 2 = {}", v),
        None => println!("division by zero"),
    };
    match safe_divide(10, 0) {
        Some(v) => println!("10 / 0 = {}", v),
        None => println!("10 / 0 = division by zero"),
    };
    match safe_sqrt(16.0) {
        Some(v) => println!("sqrt(16) = {}", v),
        None => println!("sqrt of negative"),
    };
    println!("--- Phase 3 features ---");
    let mut pc = (pipe_add_one(pipe_add_one(pipe_double(5)))).clone();
    println!("pipe-chain: {}", pc);
    let mut pm = (pipe_add3(1, 2, 3)).clone();
    println!("pipe-multi: {}", pm);
    let mut bob = (Person { name: "Bob".to_string(), tag: (Some(7)).clone() }).clone();
    println!("person_name Some: {}", person_name((Some((bob).clone())).clone()));
    println!("person_name None: {}", person_name(None));
    println!("greet with tag: {}", greet_person((Some((bob).clone())).clone()));
    println!("greet none: {}", greet_person(None));
    println!("owned consume: {}", consume_person(bob));
    println!("xor 5 ^ 3 = {}", xor_demo(5, 3));
    let mut registry: HashMap<String, i64> = (HashMap::new()).clone();
    println!("import HashMap ok: len={}", registry.len());
    println!("=== Done ===")
}


