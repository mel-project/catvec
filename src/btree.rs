use std::{collections::VecDeque, fmt::Debug, sync::Arc, thread::current};

use arrayvec::ArrayVec;
use tap::Pipe;

/// An implementation of a relative-indexed, immutable B+tree, const-generic over the fanout degree ORD.
/// https://github.com/jafingerhut/core.btree-vector/blob/master/doc/intro.md
#[derive(Clone)]
pub enum Tree<T: Clone, const ORD: usize> {
    Internal(Internal<T, ORD>),
    Array(ArrayVec<T, ORD>),
}

// impl<T: Clone + Debug, const ORD: usize> Tree<T, ORD> {
//     pub fn eprint_graphviz(self: &Arc<Self>) -> u64 {
//         // let my_id = Arc::as_ptr(self) as u64;
//         let my_id = fastrand::u64(0..u64::MAX);
//         match self.as_ref() {
//             Tree::Array(vals) => {
//                 eprintln!(
//                     "{} [label = \"[{}, {:?}]\"  shape=box];",
//                     my_id,
//                     vals.len(),
//                     vals
//                 );
//             }
//             Tree::Internal(int) => {
//                 for child in int.children.iter() {
//                     let child_id = child.eprint_graphviz();
//                     eprintln!("{} -> {};", my_id, child_id);
//                 }
//                 if int.root {
//                     eprintln!("{} [label = \"ROOT[{}]\" shape=box];", my_id, int.length);
//                 } else {
//                     eprintln!("{} [label = \"[{}]\"  shape=box];", my_id, int.length);
//                 }
//             }
//         }
//         my_id
//     }
// }

impl<T: Clone, const ORD: usize> Tree<T, ORD> {
    pub fn eprint_graphviz(self: &Arc<Self>) -> u64 {
        // let my_id = Arc::as_ptr(self) as u64;
        let my_id = fastrand::u64(0..u64::MAX);
        match self.as_ref() {
            Tree::Array(vals) => {
                eprintln!("{} [label = \"[{}, LEAF]\"  shape=box];", my_id, vals.len(),);
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

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        match self {
            Tree::Internal(internal) => internal.get_mut(idx),
            Tree::Array(items) => items.get_mut(idx),
        }
    }

    pub fn insert(&mut self, key: usize, value: T) -> Option<Self> {
        match self {
            Tree::Internal(internal) => {
                log::trace!("internal insert at key {}", key);
                internal.insert(key, value)
            }
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

    pub fn concat(&mut self, mut other: Self) {
        if self.len() == 0 {
            *self = other;
            return;
        } else if other.len() == 0 {
            return;
        }
        // first make the two heights the same
        let self_height = self.height();
        let other_height = other.height();
        // easy case: heights are the same
        if self_height == other_height {
            match self {
                Tree::Array(this) => {
                    let mut other = match other {
                        Tree::Array(other) => other,
                        _ => unreachable!(),
                    };
                    if this.len() + other.len() <= ORD {
                        // well, that's pretty trivial
                        this.extend(other.into_iter())
                    } else {
                        // okay, now we can apportion the nodes into two halves
                        if this.len() < ORD / 2 {
                            let to_move = ORD / 2 - this.len();
                            this.extend(other.drain(0..to_move));
                        } else if other.len() < ORD / 2 {
                            let to_move = ORD / 2 - other.len();
                            let start_idx = this.len() - to_move;
                            let new_other =
                                this.drain(start_idx..).chain(other.into_iter()).collect();
                            other = new_other
                        }
                        let noviy = Internal {
                            length: this.len() + other.len(),
                            children: IntoIterator::into_iter([this.clone(), other])
                                .map(|i| Arc::new(Tree::Array(i)))
                                .collect(),
                            root: true,
                        };
                        *self = Tree::Internal(noviy)
                    }
                }
                Tree::Internal(this) => {
                    let mut other = match other {
                        Tree::Internal(other) => other,
                        _ => unreachable!(),
                    };
                    if this.children.len() + other.children.len() <= ORD {
                        this.length += other.length;
                        this.children.extend(other.children.into_iter())
                    } else {
                        if this.children.len() < ORD / 2 {
                            let to_move = ORD / 2 - this.children.len();
                            for elem in other.children.drain(0..to_move) {
                                other.length -= elem.len();
                                this.length += elem.len();
                                this.children.push(elem);
                            }
                        } else if other.children.len() < ORD / 2 {
                            let to_move = ORD / 2 - other.children.len();
                            let start_idx = this.children.len() - to_move;
                            let mut new_other = ArrayVec::new();
                            for elem in this.children.drain(start_idx..) {
                                other.length += elem.len();
                                this.length -= elem.len();
                                new_other.push(elem);
                            }
                            new_other.extend(other.children.drain(0..));
                            other.children = new_other;
                        }
                        this.root = false;
                        other.root = false;
                        let this = Arc::new(Tree::Internal(this.clone()));
                        let other = Arc::new(Tree::Internal(other));
                        let noviy = Internal {
                            length: this.len() + other.len(),
                            children: IntoIterator::into_iter([this.clone(), other]).collect(),
                            root: true,
                        };
                        *self = Tree::Internal(noviy)
                    }
                }
            }
            self.fixup(true);
            self.fixup(false);
        } else {
            // hard case: heights are NOT the same. We pad the tree with useless levels until the heights are the same.
            if self_height > other_height {
                for _ in other_height..self_height {
                    other.pad_once()
                }
            } else {
                for _ in self_height..other_height {
                    self.pad_once()
                }
            }
            self.concat(other);
        }
    }

    fn pad_once(&mut self) {
        if let Tree::Internal(int) = self {
            int.root = false;
        }
        let len = self.len();
        let noo = Internal {
            root: true,
            children: IntoIterator::into_iter([Arc::new(self.clone())]).collect(),
            length: len,
        };
        *self = Tree::Internal(noo)
    }

    fn height(&self) -> usize {
        match self {
            Tree::Internal(i) => i.height(),
            _ => 0,
        }
    }

    pub fn drop_head(&mut self, key: usize) {
        match self {
            Tree::Internal(internal) => {
                internal.drop_head(key);
                if internal.root {
                    self.fixup(false)
                }
            }
            Tree::Array(arr) => {
                arr.drain(0..key);
            }
        }
    }

    pub fn take_head(&mut self, key: usize) {
        match self {
            Tree::Internal(internal) => {
                internal.take_head(key);
                if internal.root {
                    self.fixup(true)
                }
            }
            Tree::Array(arr) => {
                arr.drain(key..);
            }
        }
    }

    /// Checks invariants.
    pub fn check_invariants(&self) {
        if let Some(children) = self.children() {
            for child in children {
                child.check_invariants();
            }
            assert_eq!(children.len(), self.children_count());
            assert_eq!(self.len(), children.iter().map(|c| c.len()).sum::<usize>());
        }
        let is_root = if let Tree::Internal(int) = self {
            int.root
        } else {
            true
        };
        if !is_root {
            assert!(self.children_count() >= ORD / 2)
        }
    }

    /// Fixes stuff
    ///
    /// TODO: fix log^2(n) runtime
    fn fixup(&mut self, is_right: bool) {
        log::trace!("fixup(is_right = {})", is_right);
        for depth in (0..self.height()).rev() {
            Arc::new(self.clone()).eprint_graphviz();
            log::trace!("at depth {}", depth);
            let this = self.unwrap_internal();
            let mut stack = Vec::new();
            if is_right {
                stack.extend(this.children.iter_mut().map(|e| (e, 0usize)));
            } else {
                stack.extend(this.children.iter_mut().rev().map(|e| (e, 0usize)));
            }
            defmac::defmac!(pushch children, level => if is_right {
                stack.extend(children.iter_mut().map(|e| (e, level)));
            } else {
                stack.extend(children.iter_mut().rev().map(|e| (e, level)));
            });
            // we first go down all the way to the fringe
            for _ in 0..depth {
                let (elem, _) = stack.last().unwrap();
                match elem.as_ref() {
                    Tree::Internal(int) => {
                        // if no children, BAIL!
                        if int.children.is_empty() {
                            break;
                        }
                        let (elem, current_level) = stack.pop().unwrap();
                        let int = Arc::make_mut(elem).unwrap_internal();
                        log::trace!("pushing at level {}", current_level);
                        pushch!(&mut int.children, current_level + 1);
                    }
                    Tree::Array(_) => {
                        // BAIL out!
                        break;
                    }
                }
            }
            log::trace!("stack has {} elements", stack.len());
            if stack.is_empty() {
                break;
            }
            // At this point, the stack begins from the last level of the fringe.
            let (fringe_tip, h) = stack.pop().unwrap();
            assert!(h <= depth);
            if h < depth {
                break;
            }
            let fringe_tip = Arc::make_mut(fringe_tip);
            // We attempt to pop a neighbor at the same level
            let neighbor = loop {
                if let Some((elem, elem_level)) = stack.pop() {
                    log::trace!("finding neighbor at height {}", elem_level);
                    assert!(elem_level <= depth);
                    let top = Arc::make_mut(elem);
                    if elem_level == depth {
                        log::trace!("found the right thing");
                        break Some(top);
                    } else if let Some(children) = top.children_mut() {
                        log::trace!("pushing {} children", children.len());
                        pushch!(children, elem_level + 1);
                    } else {
                        log::trace!("skipping element with NO children");
                    }
                } else {
                    break None;
                }
            };
            log::trace!(
                "at node with {} children, found neighbor with {:?} children",
                fringe_tip.children_count(),
                neighbor.as_ref().map(|n| n.children_count())
            );
            // Fixup for that node
            let at_new_root = fringe_tip.fixup_inner(neighbor, is_right);
            if at_new_root {
                *self = fringe_tip.clone();
                break;
            }
        }
        log::trace!("final fixup!");
        self.fixup_inner(None, is_right);
    }

    /// Given a fringe node and its left/right neighbor, fix the invariants of the fringe node. Returns true if and only if the fringe node should be spun up to the root.
    fn fixup_inner(&mut self, neighbor: Option<&mut Self>, is_right: bool) -> bool {
        // We remove any empty children. These are from previous runs.
        if let Tree::Internal(fringe) = self {
            fringe.children.retain(|c| c.len() > 0);
            fringe.length = fringe.children.iter().map(|c| (c.len())).sum();
            if fringe.root && fringe.children.is_empty() {
                fringe.children.push(Arc::new(Tree::Array(ArrayVec::new())))
            }
        }

        // go through the different cases now!
        // case 1: no neighbor. This means that this node should be the root!
        match neighbor {
            None => {
                log::trace!("case 1 hit");
                if let Tree::Internal(int) = self {
                    int.root = true;
                    true
                } else {
                    false
                }
            }
            Some(neighbor) => {
                if let Tree::Internal(neighbor) = neighbor {
                    neighbor.children.retain(|c| c.len() > 0);
                    neighbor.length = neighbor.children.iter().map(|c| c.len()).sum();
                }
                // case 2: F doesn't actually violate invariants
                if self.children_count() >= ORD / 2 {
                    log::trace!("case 2 hit");
                    return false;
                }
                // case 3: F violates the invariants by having too little children.
                assert!(self.children_count() < ORD / 2);
                // case 3a: self + neighbor have at most ORD children. we merge self into neighbor.
                if self.children_count() + neighbor.children_count() <= ORD {
                    log::trace!("case 3a hit");
                    self.give_all_children_to(neighbor, is_right);
                    false
                } else {
                    // case 3b: self+neighbor overflow in children. we steal children from our neighbor.
                    log::trace!("case 3b hit");
                    self.steal_children_from(neighbor, is_right);
                    false
                }
            }
        }
    }

    /// Push children to the other node.
    fn give_all_children_to(&mut self, other: &mut Self, is_right: bool) {
        log::trace!("giving all children");
        match other {
            Tree::Array(other) => {
                let this = self.unwrap_arr();
                if is_right {
                    other.extend(this.drain(0..))
                } else {
                    this.extend(other.drain(0..));
                    std::mem::swap(this, other);
                }
            }
            Tree::Internal(other) => {
                let this = self.unwrap_internal();
                other.length += this.length;
                this.length = 0;
                if is_right {
                    other.children.extend(this.children.drain(0..));
                } else {
                    this.children.extend(other.children.drain(0..));
                    std::mem::swap(&mut this.children, &mut other.children);
                }
            }
        }
    }

    /// List of all children
    fn children_mut(&mut self) -> Option<&mut ArrayVec<Arc<Self>, ORD>> {
        match self {
            Tree::Array(_) => None,
            Tree::Internal(int) => Some(&mut int.children),
        }
    }

    /// List of all children
    fn children(&self) -> Option<&ArrayVec<Arc<Self>, ORD>> {
        match self {
            Tree::Array(_) => None,
            Tree::Internal(int) => Some(&int.children),
        }
    }

    /// Steal children from the other node until we satisfy the invariant.
    fn steal_children_from(&mut self, other: &mut Self, is_right: bool) {
        match other {
            Tree::Array(other) => {
                let this = self.unwrap_arr();
                if is_right {
                    while this.len() < ORD / 2 {
                        this.insert(0, other.pop().expect("other children ran out"))
                    }
                } else {
                    log::trace!("{} STEALING {}", this.len(), other.len());
                    let before = this.len() + other.len();
                    let to_move = ORD / 2 - this.len();
                    this.extend(other.drain(0..to_move));
                    let after = this.len() + other.len();
                    log::trace!("{} BALANCED {}", this.len(), other.len());
                    assert_eq!(before, after);
                }
            }
            Tree::Internal(other) => {
                let this = self.unwrap_internal();
                if is_right {
                    while this.children.len() < ORD / 2 {
                        let child = other.children.pop().expect("other children ran out");
                        other.length -= child.len();
                        this.length += child.len();
                        this.children.insert(0, child);
                    }
                } else {
                    let before = this.length + other.length;
                    let to_move = ORD / 2 - this.children.len();
                    for child in other.children.drain(0..to_move) {
                        other.length -= child.len();
                        this.length += child.len();
                        this.children.push(child);
                    }
                    let after = this.length + other.length;
                    assert_eq!(before, after);
                }
            }
        }
    }

    /// Unwraps as array.
    fn unwrap_arr(&mut self) -> &mut ArrayVec<T, ORD> {
        match self {
            Tree::Array(arr) => arr,
            _ => panic!("unwrap_arr called on a non-array node "),
        }
    }

    /// Unwraps as internal.
    fn unwrap_internal(&mut self) -> &mut Internal<T, ORD> {
        match self {
            Tree::Internal(int) => int,
            _ => panic!("unwrap_internal called on non-internal node"),
        }
    }

    /// Returns the count of all children, either internal or array-elements.
    fn children_count(&self) -> usize {
        match self {
            Tree::Array(arr) => arr.len(),
            Tree::Internal(it) => it.children.len(),
        }
    }

    // Returns the two first *immediate* children of this node.
    fn first_two_children(&mut self) -> (Option<&mut Self>, Option<&mut Self>) {
        match self {
            Tree::Array(_) => (None, None),
            Tree::Internal(i) => {
                if i.children.is_empty() {
                    return (None, None);
                }
                let (first, rest) = i.children.split_first_mut().unwrap();
                let rest_first = rest.split_first_mut().map(|p| p.0);
                (
                    Some(Arc::make_mut(first)),
                    rest_first.map(|rf| Arc::make_mut(rf)),
                )
            }
        }
    }

    // Returns the two last *immediate* children of this node.
    fn last_two_children(&mut self) -> (Option<&mut Self>, Option<&mut Self>) {
        match self {
            Tree::Array(_) => (None, None),
            Tree::Internal(i) => {
                if i.children.is_empty() {
                    return (None, None);
                }
                let (first, rest) = i.children.split_last_mut().unwrap();
                let rest_first = rest.split_last_mut().map(|p| p.0);
                (
                    Some(Arc::make_mut(first)),
                    rest_first.map(|rf| Arc::make_mut(rf)),
                )
            }
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

    fn get_mut(&mut self, key: usize) -> Option<&mut T> {
        if key >= self.length {
            return None;
        }
        let (idx, offset) = self.key_to_idx_and_offset(key);
        Arc::make_mut(&mut self.children[idx]).get_mut(key - offset)
    }

    fn insert(&mut self, key: usize, value: T) -> Option<Tree<T, ORD>> {
        if !self.children.is_full() {
            log::trace!("non-full case");
            // we have room to stuff some more, this is the easy case
            let (idx, offset) = self.key_to_idx_and_offset(key);
            let correct_child = Arc::make_mut(&mut self.children[idx]);
            // try inserting into that child
            let other = correct_child.insert(key - offset, value);
            // if the other side is Some, this means that we need to insert an extra child.
            if let Some(other) = other {
                self.children.insert(idx + 1, Arc::new(other));
                log::trace!("non-full case, but adding another child")
            }
            self.length += 1;
            // no need to twiddle with our parents at all
            None
        } else if self.root {
            log::trace!("full root, adding another level");
            // just make another level, stupid
            let mut self_copy = self.clone();
            self_copy.root = false;
            self.children.clear();
            self.children.push(Arc::new(Tree::Internal(self_copy)));
            self.insert(key, value)
        } else {
            log::trace!("complicated case");
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
        if key == 0 {
            return;
        }
        assert!(key <= self.length);
        self.length -= key;
        let (idx, offset) = self.key_to_idx_and_offset(key);
        self.children.drain(0..idx);
        if !self.children.is_empty() {
            Arc::make_mut(&mut self.children[0]).drop_head(key - offset);
        }
    }

    fn take_head(&mut self, key: usize) {
        assert!(key <= self.length);
        if key == self.length {
            return;
        }
        let (idx, offset) = self.key_to_idx_and_offset(key);
        self.children.drain(idx + 1..);
        if let Some(last) = self.children.last_mut() {
            Arc::make_mut(last).take_head(key - offset);
        }
    }

    fn height(&self) -> usize {
        let mut height = 1;
        let mut ptr = self;
        loop {
            if let Some(Tree::Internal(n)) = ptr.children.get(0).map(|f| f.as_ref()) {
                ptr = n;
                height += 1;
            } else {
                return height;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use log::LevelFilter;

    use super::*;

    fn init_logs() {
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(LevelFilter::Trace)
            .try_init();
    }

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
        Arc::new(tree).eprint_graphviz();
    }

    #[test]
    fn concat() {
        init_logs();
        let mut tree: Tree<usize, 5> = testvec(125);
        tree.concat(testvec(1));
        Arc::new(tree).eprint_graphviz();
    }

    fn testvec(n: usize) -> Tree<usize, 5> {
        let mut tree = Tree::new();
        for i in 0..n {
            let idx = tree.len();
            tree.insert(idx, i);
        }
        tree
    }
}
