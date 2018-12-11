extern crate deriving;
use deriving::Divisible;

trait Divisible {
    fn length(&self) -> usize;
}

impl<T> Divisible for Vec<T> {
    fn length(&self) -> usize {
        self.len()
    }
}

#[derive(Divisible)]
struct Foo<T: Sized + Copy> {
    #[divide_by(copy)]
    foo: T,
    #[divide_by(default)]
    bar: f64,
    baz: Vec<u32>,
    baz2: Vec<f64>,
}

fn main() {
    let f = Foo {
        foo: 3,
        bar: 0.5,
        baz: vec![1, 2, 3],
        baz2: vec![2.2, 3.3],
    };
    println!("l: {}", f.length());
}
