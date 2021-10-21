use std::{fmt::Debug, sync::Arc};

use arrayvec::ArrayVec;

/// An implementation of a relative-indexed, immutable B+tree, const-generic over the fanout degree ORD.
/// https://github.com/jafingerhut/core.btree-vector/blob/master/doc/intro.md
#[derive(Clone)]
pub enum Tree<T: Clone, const ORD: usize> {
    Internal(Internal<T, ORD>),
    Array(ArrayVec<T, ORD>),
}

impl<T: Clone + Debug, const ORD: usize> Tree<T, ORD> {
    fn eprint_graphviz(&self) -> u64 {
        let my_id = fastrand::u64(0..u64::MAX);
        match self {
            Tree::Array(vals) => {
                eprintln!(
                    "{} [label = \"[{}, {:?}]\"  shape=box];",
                    my_id,
                    vals.len(),
                    vals
                );
            }
            Tree::Internal(int) => {
                for child in int.children.iter() {
                    let child_id = child.eprint_graphviz();
                    eprintln!("{} -> {};", my_id, child_id);
                }
                if int.root {
                    eprintln!("{} [label = \"ROOT[{}]\" shape=box];", my_id, int.length);
                } else {
                    eprintln!("{} [label = \"[{}]\"  shape=box];", my_id, int.length);
                }
            }
        }
        my_id
    }
}

impl<T: Clone, const ORD: usize> Tree<T, ORD> {
    pub fn new() -> Self {
        Tree::Internal(Internal {
            length: 0,
            children: {
                let mut v = ArrayVec::new();
                v.push(Arc::new(Tree::Array(ArrayVec::new())));
                v
            },
            root: true,
        })
    }

    pub fn len(&self) -> usize {
        match self {
            Tree::Internal(internal) => internal.length,
            Tree::Array(inner) => inner.len(),
        }
    }

    pub fn get(&self, idx: usize) -> Option<&T> {
        match self {
            Tree::Internal(internal) => internal.get(idx),
            Tree::Array(items) => items.get(idx),
        }
    }

    pub fn insert(&mut self, key: usize, value: T) -> Option<Self> {
        match self {
            Tree::Internal(internal) => internal.insert(key, value),
            Tree::Array(values) => {
                if !values.is_full() {
                    values.insert(key, value);
                    None
                } else {
                    let split_point = values.len() / 2;
                    let mut other_values: ArrayVec<_, ORD> = values.drain(split_point..).collect();
                    if key >= split_point {
                        other_values.insert(key - split_point, value);
                    } else {
                        values.insert(key, value);
                    }
                    Some(Tree::Array(other_values))
                }
            }
        }
    }

    fn drop_head(&mut self, key: usize) {
        match self {
            Tree::Internal(internal) => internal.drop_head(key),
            Tree::Array(arr) => {
                arr.drain(0..key);
            }
        }
    }

    fn take_head(&mut self, key: usize) {
        match self {
            Tree::Internal(internal) => internal.take_head(key),
            Tree::Array(arr) => {
                arr.drain(key..);
            }
        }
    }

    fn fixup(&mut self, is_right: bool) {
        if let Tree::Internal(int) = self {
            int.fixup(is_right)
        }
    }
}

#[derive(Clone)]
pub struct Internal<T: Clone, const ORD: usize> {
    length: usize,
    children: ArrayVec<Arc<Tree<T, ORD>>, ORD>,
    root: bool,
}

impl<T: Clone, const ORD: usize> Internal<T, ORD> {
    fn get(&self, key: usize) -> Option<&T> {
        if key >= self.length {
            return None;
        }
        let (idx, offset) = self.key_to_idx_and_offset(key);
        self.children[idx].get(key - offset)
    }

    fn insert(&mut self, key: usize, value: T) -> Option<Tree<T, ORD>> {
        if !self.children.is_full() {
            // we have room to stuff some more, this is the easy case
            let (idx, offset) = self.key_to_idx_and_offset(key);
            let correct_child = Arc::make_mut(&mut self.children[idx]);
            // try inserting into that child
            let other = correct_child.insert(key - offset, value);
            // if the other side is Some, this means that we need to insert an extra child.
            if let Some(other) = other {
                self.children.insert(idx + 1, Arc::new(other));
            }
            self.length += 1;
            // no need to twiddle with our parents at all
            None
        } else if self.root {
            // just make another level, stupid
            let mut self_copy = self.clone();
            self_copy.root = false;
            self.children.clear();
            self.children.push(Arc::new(Tree::Internal(self_copy)));
            self.insert(key, value)
        } else {
            // the more complicated case. we split off like half of the nodes
            let split_point = self.children.len() / 2;
            let other_children: ArrayVec<_, ORD> = self.children.drain(split_point..).collect();
            assert_eq!(self.children.len() + other_children.len(), ORD);
            let mut other = Tree::Internal(Internal {
                length: other_children.iter().map(|f| f.len()).sum(),
                children: other_children,
                root: false,
            });
            let split_point = self.length - other.len();
            self.length -= other.len();
            // insert into the other side. this CANNOT cause an overflow no matter what!
            if key >= split_point {
                assert!(other.insert(key - split_point, value).is_none());
            } else {
                assert!(self.insert(key, value).is_none());
            }
            Some(other)
        }
    }

    fn key_to_idx_and_offset(&self, key: usize) -> (usize, usize) {
        let mut offset = 0;
        for (idx, child) in self.children.iter().enumerate() {
            if key - offset < child.len() || idx + 1 == self.children.len() {
                return (idx, offset);
            }
            offset += child.len()
        }
        unreachable!()
    }

    fn drop_head(&mut self, key: usize) {
        assert!(key < self.length);
        self.length -= key;
        let (idx, offset) = self.key_to_idx_and_offset(key);
        self.children.drain(0..idx);
        if !self.children.is_empty() {
            Arc::make_mut(&mut self.children[0]).drop_head(key - offset);
        }
        if self.root {
            self.fixup(false)
        }
    }

    fn take_head(&mut self, key: usize) {
        assert!(key < self.length);
        self.length = key;
        let (idx, offset) = self.key_to_idx_and_offset(key);
        self.children.drain(idx + 1..);
        if let Some(last) = self.children.last_mut() {
            Arc::make_mut(last).take_head(key - offset);
        }
        if self.root {
            self.fixup(true)
        }
    }

    fn fixup(&mut self, is_right: bool) {
        loop {
            if self.children.is_empty() {
                return;
            }
            if is_right {
                Arc::make_mut(self.children.last_mut().unwrap()).fixup(is_right)
            } else {
                Arc::make_mut(self.children.first_mut().unwrap()).fixup(is_right)
            }

            let (chhead, chtail) = if is_right {
                let pt = self.children.len() - 1;
                let (l, r) = self.children.split_at_mut(pt);
                (r, l)
            } else {
                self.children.split_at_mut(1)
            };
            let eff = &mut chhead[0];
            // split1: F has no right neighbour
            if chtail.is_empty() {
                // Dude, this means that we can just replace self with eff!
                let eff = eff.as_ref().clone();
                if let Tree::Internal(mut eff) = eff {
                    eff.root = self.root;
                    *self = eff;
                } else {
                    break;
                }
            } else {
                let arr = if !is_right {
                    &mut chtail[0]
                } else {
                    chtail.last_mut().unwrap()
                };
                if let Tree::Internal(eff_int) = eff.as_ref() {
                    if eff_int.children.len() < ORD / 2 {
                        let arr = match Arc::make_mut(arr) {
                            Tree::Internal(internal) => internal,
                            _ => unreachable!(),
                        };
                        let eff = match Arc::make_mut(eff) {
                            Tree::Internal(internal) => internal,
                            _ => unreachable!(),
                        };
                        // split4: f+r does not overflow. just add it all to f and delete r
                        if eff.children.len() + arr.children.len() <= ORD {
                            if !is_right {
                                eff.length += arr.length;
                                eff.children.extend(arr.children.drain(0..));
                                self.children.remove(1);
                            } else {
                                arr.length += eff.length;
                                arr.children.extend(eff.children.drain(0..));
                                self.children.pop();
                            }
                        } else {
                            // split5: f+r does overflow. instead of merging, we move elements from r to f.
                            let to_move = (ORD / 2) - eff.length;
                            let mut delta_len = 0;
                            if !is_right {
                                for to_move in arr.children.drain(0..to_move) {
                                    delta_len += to_move.len();
                                    eff.children.push(to_move);
                                }
                            } else {
                                let start_idx = arr.children.len() - to_move;
                                let new_eff_children: ArrayVec<_, ORD> = arr
                                    .children
                                    .drain(start_idx..)
                                    .map(|d| {
                                        delta_len += d.len();
                                        d
                                    })
                                    .chain(eff.children.drain(0..))
                                    .collect();
                                eff.children = new_eff_children;
                            }
                            arr.length -= delta_len;
                            eff.length += delta_len;
                        }
                    } else {
                        return;
                    }
                } else if let Tree::Array(eff_array) = eff.as_ref() {
                    if eff_array.len() < ORD / 2 {
                        let arr = match Arc::make_mut(arr) {
                            Tree::Array(arr) => arr,
                            _ => unreachable!(),
                        };
                        let eff = match Arc::make_mut(eff) {
                            Tree::Array(eff) => eff,
                            _ => unreachable!(),
                        };
                        if eff.len() + arr.len() <= ORD {
                            if !is_right {
                                eff.extend(arr.drain(0..));
                                self.children.remove(1);
                            } else {
                                arr.extend(eff.drain(0..));
                                self.children.pop();
                            }
                        } else {
                            let to_move = (ORD / 2) - eff.len();
                            if !is_right {
                                eff.extend(arr.drain(0..to_move))
                            } else {
                                let start_idx = arr.len() - to_move;
                                let new: ArrayVec<_, ORD> =
                                    arr.drain(start_idx..).chain(eff.drain(0..)).collect();
                                *eff = new;
                            }
                        }
                    } else {
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic_insertion() {
        let mut tree: Tree<usize, 5> = Tree::new();
        let mut vec = Vec::new();
        for i in 0..20 {
            let idx = tree.len() / 2;
            tree.insert(idx, i);
            assert_eq!(tree.get(idx).copied().unwrap(), i);
            vec.insert(idx, i)
        }
        tree.take_head(5);
        tree.eprint_graphviz();
    }
}
