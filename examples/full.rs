extern crate deriving;
use deriving::Divisible;

trait Divisible: Sized {
    fn base_length(&self) -> usize;
    fn divide(self) -> (Self, Self);
}

impl<T> Divisible for &[T] {
    fn base_length(&self) -> usize {
        self.len()
    }
    fn divide(self) -> (Self, Self) {
        let mid = self.len() / 2;
        self.split_at(mid)
    }
}

#[derive(Divisible, Debug)]
struct Foo<'a, 'b, T: Sized + Copy> {
    #[divide_by(copy)]
    foo: T,
    #[divide_by(default)]
    bar: f64,
    baz: &'a [u32],
    baz2: &'b [f64],
}

fn main() {
    let v1 = vec![1, 2, 3];
    let v2 = vec![2.4, 3.3];
    let f = Foo {
        foo: 3,
        bar: 0.5,
        baz: &v1,
        baz2: &v2,
    };
    println!("l: {}", f.base_length());
    let (f1, f2) = f.divide();
    println!("left: {:?}, right: {:?}", f1, f2);
}
