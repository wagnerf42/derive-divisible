extern crate derive_divisible;
use derive_divisible::{Divisible, ParallelIterator};
use std::iter;
use std::iter::empty;
use std::marker::PhantomData;
use std::slice::Iter;

struct IndexedPower();
enum Policy {
    Rayon(usize),
}

/// Iterator on some `Divisible` input by blocks.
struct BlocksIterator<P, I: Divisible<P>, S: Iterator<Item = usize>> {
    /// sizes of all the remaining blocks
    pub(crate) sizes: S,
    /// remaining input
    pub(crate) remaining: Option<I>,
    pub(crate) phantom: PhantomData<P>,
}

impl<P, I: Divisible<P>, S: Iterator<Item = usize>> Iterator for BlocksIterator<P, I, S> {
    type Item = I;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_none() {
            // no input left
            return None;
        }

        let remaining_length = self.remaining.as_ref().and_then(I::base_length);
        if let Some(length) = remaining_length {
            if length == 0 {
                // no input left
                return None;
            }
        }

        let current_size = self.sizes.next();
        if let Some(size) = current_size {
            let remaining_input = self.remaining.take().unwrap();
            let (left, right) = remaining_input.divide_at(size);
            std::mem::replace(&mut self.remaining, Some(right));
            Some(left)
        } else {
            // no sizes left, return everything thats left to process
            self.remaining.take()
        }
    }
}

// let's start by re-declaring the traits
trait Divisible<P>: Sized {
    fn base_length(&self) -> Option<usize>;
    fn divide_at(self, index: usize) -> (Self, Self);
    fn divide(self) -> (Self, Self) {
        let mid = self.base_length().expect("infinite") / 2;
        self.divide_at(mid)
    }
    /// Return a sequential iterator on blocks of Self of given sizes.
    fn blocks<S: Iterator<Item = usize>>(self, sizes: S) -> BlocksIterator<P, Self, S> {
        BlocksIterator {
            sizes,
            remaining: Some(self),
            phantom: PhantomData,
        }
    }
}

/// We can produce sequential iterators to be eaten slowly.
trait Edible: Sized + Send {
    /// This registers the type of output produced (it IS the item of the SequentialIterator).
    type Item: Send;
    /// This registers the type of iterators produced.
    type SequentialIterator: Iterator<Item = Self::Item>;
    /// Give us a sequential iterator corresponding to `size` iterations.
    fn iter(self, size: usize) -> (Self::SequentialIterator, Self);
}

trait ParallelIterator<P>: Divisible<P> {
    type SequentialIterator: Iterator<Item = Self::Item>;
    type Item: Send;
    /// Extract sequential iterator of given size.
    fn iter(self, size: usize) -> (Self::SequentialIterator, Self);
    /// Return an iterator on sizes of all macro blocks.
    fn blocks_sizes(&mut self) -> Box<Iterator<Item = usize>> {
        Box::new(empty())
    }
    /// Return current scheduling `Policy`.
    fn policy(&self) -> Policy {
        Policy::Rayon(1)
    }
}

// now implement basic traits for some basic type
impl<T> Divisible<IndexedPower> for &[T] {
    fn base_length(&self) -> Option<usize> {
        Some(self.len())
    }
    fn divide_at(self, index: usize) -> (Self, Self) {
        self.split_at(index)
    }
}

impl<'a, T: 'a + Sync> ParallelIterator<IndexedPower> for &'a [T] {
    type SequentialIterator = Iter<'a, T>;
    type Item = &'a T;
    fn iter(self, size: usize) -> (Self::SequentialIterator, Self) {
        (self[..size].iter(), &self[size..])
    }
}

/// now let's derive some stuff

//#[derive(Divisible, ParallelIterator, Debug)]
//#[power(P)]
//#[item(R)]
//#[sequential_iterator(iter::Map<I::SequentialIterator, F>)]
//#[iterator_extraction(i.map(self.map_op.clone()))]
#[derive(Divisible, Debug)]
#[power(P)]
struct Map<R: Send, P: Send, I: ParallelIterator<P>, F: Clone + Send + Fn(I::Item) -> R> {
    #[divide_by(clone)]
    map_op: F,
    iterator: I,
    #[divide_by(default)]
    phantom: PhantomData<P>,
}

// impl<P: Send, I: ParallelIterator<P>, R: Send, F: Clone + Send + Fn(I::Item) -> R> Edible
//     for Map<R, P, I, F>
// {
//     type Item = R;
//     type SequentialIterator = iter::Map<I::SequentialIterator, F>;
//     fn iter(mut self, size: usize) -> (Self::SequentialIterator, Self) {
//         let (i, remaining) = self.iterator.iter(size);
//         self.iterator = remaining;
//         (i.map(self.map_op.clone()), self)
//     }
// }

fn main() {
    let v1 = vec![1, 2, 3];
    let m = Map {
        map_op: |x: &u32| -> u32 { *x + 1 },
        iterator: v1.as_slice(),
        phantom: PhantomData,
    };
    println!("l: {:?}", m.base_length());
    let (m1, m2) = m.divide();
    println!(
        "left: {:?}, right: {:?}",
        m1.base_length(),
        m2.base_length()
    );
}
