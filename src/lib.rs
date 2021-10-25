use std::{
    ops::{Bound, RangeBounds},
    sync::Arc,
};

use btree::Tree;

mod btree;

/// A persistent, efficiently concatenable and sliceable vector. The const-generic type parameter ORD is the maximum fanout factor; a value from 32 to 128 usually works well.
#[derive(Clone)]
pub struct CatVec<T: Clone, const ORD: usize> {
    inner: Tree<T, ORD>,
}

impl<T: Clone, const ORD: usize> CatVec<T, ORD> {
    /// Creates a new empty CatVec.
    pub fn new() -> Self {
        Self { inner: Tree::new() }
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
        self.inner.concat(other.inner)
    }

    /// Inserts the given element at the given position, shifting all elements after that rightwards.
    pub fn insert(&mut self, idx: usize, val: T) {
        self.inner.insert(idx, val);
    }
}

impl<T: Clone, const ORD: usize> Default for CatVec<T, ORD> {
    fn default() -> Self {
        Self::new()
    }
}
