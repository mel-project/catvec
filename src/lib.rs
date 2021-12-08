use std::{
    ops::{Bound, RangeBounds},
    sync::Arc,
};

use btree::Tree;
use tap::Tap;

mod btree;

/// A persistent, efficiently concatenable and sliceable vector. The const-generic type parameter ORD is the maximum fanout factor; a value from 32 to 128 usually works well.
#[derive(Clone)]
pub struct CatVec<T: Clone, const ORD: usize> {
    inner: Box<Tree<T, ORD>>,
}

impl<T: Clone + PartialEq, const ORD: usize> PartialEq<CatVec<T, ORD>> for CatVec<T, ORD> {
    fn eq(&self, other: &Self) -> bool {
        let first_length: usize = self.len();
        let second_length: usize = other.len();

        let do_lengths_match: bool = first_length == second_length;

        if do_lengths_match {
            let do_all_indexes_match: bool = (0..first_length).all(|index| {
                let first_index: Option<&T> = self.get(index);
                let second_index: Option<&T> = other.get(index);

                first_index.expect("Failed to unrwap first index") == second_index.expect("Failed to unrwap second index")
            });

            do_all_indexes_match
        } else {
            do_lengths_match
        }
    }
}

impl<T: Clone + Eq, const ORD: usize> Eq for CatVec<T, ORD> {}


impl<T: Clone, V: AsRef<[T]>, const ORD: usize> From<V> for CatVec<T, ORD> {
    fn from(v: V) -> Self {
        v.as_ref()
            .iter()
            .fold(CatVec::new(), |a, b| a.tap_mut(|a| a.push_back(b.clone())))
    }
}

impl<T: Clone, const ORD: usize> From<CatVec<T, ORD>> for Vec<T> {
    fn from(cv: CatVec<T, ORD>) -> Self {
        let mut result = Vec::with_capacity(cv.len());
        for i in 0..cv.len() {
            result.push(cv.get(i).unwrap().clone());
        }
        result
    }
}

impl<T: Clone + std::fmt::Debug, const ORD: usize> std::fmt::Debug for CatVec<T, ORD> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v: Vec<_> = self.clone().into();
        std::fmt::Debug::fmt(&v, f)
    }
}

impl<T: Clone + std::fmt::Debug, const ORD: usize> CatVec<T, ORD> {
    /// Debug graphviz.
    pub fn debug_graphviz(&self) {
        Arc::new(*self.inner.clone()).eprint_graphviz();
    }
}

impl<T: Clone, const ORD: usize> CatVec<T, ORD> {
    /// Creates a new empty CatVec.
    pub fn new() -> Self {
        Self {
            inner: Tree::new().into(),
        }
    }

    /// Gets a reference to the element at a particular position.
    pub fn get(&self, i: usize) -> Option<&T> {
        self.inner.get(i)
    }

    /// Gets a mutable reference to the element at a particular position.
    pub fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        self.inner.get_mut(i)
    }

    /// Slices a subset of the vector. "Zooms into" a part of the vector.
    pub fn slice_into(&mut self, range: impl RangeBounds<usize>) {
        let start = match range.start_bound() {
            Bound::Excluded(i) => Some(*i + 1),
            Bound::Included(i) => Some(*i),
            Bound::Unbounded => None,
        };
        let end = match range.end_bound() {
            Bound::Excluded(i) => Some(*i),
            Bound::Included(i) => Some(*i + 1),
            Bound::Unbounded => None,
        };
        if let Some(end) = end {
            self.inner.take_head(end)
        }
        if let Some(start) = start {
            self.inner.drop_head(start)
        }
    }

    /// Concatenates this vector with another one. Consumes the other vector.
    pub fn append(&mut self, other: Self) {
        self.inner.concat(*other.inner)
    }

    /// Inserts the given element at the given position, shifting all elements after that rightwards.
    pub fn insert(&mut self, idx: usize, val: T) {
        self.inner.insert(idx, val);
    }

    /// Pushes to the back of the vector.
    pub fn push_back(&mut self, val: T) {
        let len = self.len();
        self.insert(len, val)
    }

    /// Length of vector.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check invariant.
    pub fn check_invariants(&self) {
        self.inner.check_invariants();
    }
}

impl<T: Clone, const ORD: usize> Default for CatVec<T, ORD> {
    fn default() -> Self {
        Self::new()
    }
}