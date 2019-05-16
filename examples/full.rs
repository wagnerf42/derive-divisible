extern crate derive_divisible;
use derive_divisible::Divisible;

struct IndexedPower();

trait Divisible<P>: Sized {
    fn base_length(&self) -> Option<usize>;
    fn divide_at(self, index: usize) -> (Self, Self);
    fn divide(self) -> (Self, Self) {
        let mid = self.base_length().expect("infinite") / 2;
        self.divide_at(mid)
    }
}

impl<T> Divisible<IndexedPower> for &[T] {
    fn base_length(&self) -> Option<usize> {
        Some(self.len())
    }
    fn divide_at(self, index: usize) -> (Self, Self) {
        self.split_at(index)
    }
}

#[derive(Divisible, Debug)]
#[power(IndexedPower)]
struct Foo<'a, 'b, T: Sized + Copy> {
    #[divide_by(clone)]
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
    println!("l: {:?}", f.base_length());
    let (f1, f2) = f.divide();
    println!("left: {:?}, right: {:?}", f1, f2);
}
