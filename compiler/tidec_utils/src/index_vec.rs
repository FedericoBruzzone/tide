//! A vector-like data structure that uses an index type to access elements.
//!
//! It is inspired by the `IndexVec` type from the `rustc` compiler.

use crate::idx::Idx;
use crate::index_slice::IdxSlice;
use std::{
    borrow::{Borrow, BorrowMut},
    marker::PhantomData,
    ops::{Deref, DerefMut, RangeBounds},
    slice, vec,
};

/// An owned contiguous collection of `T`s, indexed by `I` rather than by `usize`.
///
/// ## Why use this instead of a `Vec`?
///
/// An `IdxVec` allows element access only via a specific associated index type, meaning that
/// trying to use the wrong index type (possibly accessing an invalid element) will fail at
/// compile time.
///
/// It also documents what the index is indexing: in a `HashMap<usize, Something>` it's not
/// immediately clear what the `usize` means, while a `HashMap<FieldIdx, Something>` makes it obvious.
///
/// While it's possible to use `u32` or `usize` directly for `I`,
/// you almost certainly want to use a newtype for the index type.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct IdxVec<I: Idx, T> {
    _marker: PhantomData<I>,
    pub raw: Vec<T>,
}

impl<I: Idx, T> Default for IdxVec<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: Idx, T> IdxVec<I, T> {
    /// Constructs a new, empty `IdxVec<I, T>`.
    #[inline]
    pub const fn new() -> Self {
        IdxVec::from_raw(Vec::new())
    }

    /// Constructs a new `IdxVec<I, T>` from a `Vec<T>`.
    #[inline]
    pub const fn from_raw(raw: Vec<T>) -> Self {
        IdxVec {
            raw,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        IdxVec::from_raw(Vec::with_capacity(capacity))
    }

    #[inline]
    /// Pushes a value to the end of the vector, returning the index at which it was inserted.
    pub fn push(&mut self, value: T) -> I {
        let idx = self.next_index();
        self.raw.push(value);
        idx
    }

    /// Creates a new vector with a copy of `elem` for each index in `universe`.
    ///
    /// Thus `IdxVec::from_elem(elem, &universe)` is equivalent to
    /// `IdxVec::<I, _>::from_elem_n(elem, universe.len())`. That can help
    /// type inference as it ensures that the resulting vector uses the same
    /// index type as `universe`, rather than something potentially surprising.
    ///
    /// For example, if you want to store data for each local in a MIR body,
    /// using `let mut uses = IdxVec::from_elem(vec![], &body.local_decls);`
    /// ensures that `uses` is an `IdxVec<Local, _>`, and thus can give
    /// better error messages later if one accidentally mismatches indices.
    #[inline]
    pub fn from_elem<S>(elem: T, universe: &IdxSlice<I, S>) -> Self
    where
        T: Clone,
    {
        IdxVec::from_raw(vec![elem; universe.len()])
    }

    /// Creates a new IdxVec with n copies of the `elem`.
    #[inline]
    pub fn from_elem_n(elem: T, n: usize) -> Self
    where
        T: Clone,
    {
        IdxVec::from_raw(vec![elem; n])
    }

    /// Create an `IdxVec` with `n` elements, where the value of each
    /// element is the result of `func(i)`. (The underlying vector will
    /// be allocated only once, with a capacity of at least `n`.)
    #[inline]
    pub fn from_fn_n(func: impl FnMut(I) -> T, n: usize) -> Self {
        IdxVec::from_raw((0..n).map(I::new).map(func).collect())
    }

    #[inline]
    pub fn as_slice(&self) -> &IdxSlice<I, T> {
        IdxSlice::from_raw(&self.raw)
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut IdxSlice<I, T> {
        IdxSlice::from_raw_mut(&mut self.raw)
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        self.raw.pop()
    }

    #[inline]
    pub fn into_iter_enumerated(
        self,
    ) -> impl DoubleEndedIterator<Item = (I, T)> + ExactSizeIterator {
        self.raw
            .into_iter()
            .enumerate()
            .map(|(n, t)| (I::new(n), t))
    }

    #[inline]
    pub fn drain<R: RangeBounds<usize>>(
        &mut self,
        range: R,
    ) -> impl Iterator<Item = T> + use<'_, R, I, T> {
        self.raw.drain(range)
    }

    #[inline]
    pub fn drain_enumerated<R: RangeBounds<usize>>(
        &mut self,
        range: R,
    ) -> impl Iterator<Item = (I, T)> + use<'_, R, I, T> {
        let begin = match range.start_bound() {
            std::ops::Bound::Included(i) => *i,
            std::ops::Bound::Excluded(i) => i.checked_add(1).unwrap(),
            std::ops::Bound::Unbounded => 0,
        };
        self.raw
            .drain(range)
            .enumerate()
            .map(move |(n, t)| (I::new(begin + n), t))
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.raw.shrink_to_fit()
    }

    #[inline]
    pub fn truncate(&mut self, a: usize) {
        self.raw.truncate(a)
    }

    /// Grows the index vector so that it contains an entry for
    /// `elem`; if that is already true, then has no
    /// effect. Otherwise, inserts new values as needed by invoking
    /// `fill_value`.
    ///
    /// Returns a reference to the `elem` entry.
    #[inline]
    pub fn ensure_contains_elem(&mut self, elem: I, fill_value: impl FnMut() -> T) -> &mut T {
        let min_new_len = elem.idx() + 1;
        if self.len() < min_new_len {
            self.raw.resize_with(min_new_len, fill_value);
        }

        &mut self[elem]
    }

    #[inline]
    pub fn resize(&mut self, new_len: usize, value: T)
    where
        T: Clone,
    {
        self.raw.resize(new_len, value)
    }

    #[inline]
    pub fn resize_to_elem(&mut self, elem: I, fill_value: impl FnMut() -> T) {
        let min_new_len = elem.idx() + 1;
        self.raw.resize_with(min_new_len, fill_value);
    }

    #[inline]
    pub fn append(&mut self, other: &mut Self) {
        self.raw.append(&mut other.raw);
    }
}

////////// Trait implementations  //////////

impl<I: Idx, T> Deref for IdxVec<I, T> {
    type Target = IdxSlice<I, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<I: Idx, T> DerefMut for IdxVec<I, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<I: Idx, T> Borrow<IdxSlice<I, T>> for IdxVec<I, T> {
    fn borrow(&self) -> &IdxSlice<I, T> {
        self
    }
}

impl<I: Idx, T> BorrowMut<IdxSlice<I, T>> for IdxVec<I, T> {
    fn borrow_mut(&mut self) -> &mut IdxSlice<I, T> {
        self
    }
}

impl<I: Idx, T> FromIterator<T> for IdxVec<I, T> {
    #[inline]
    fn from_iter<J>(iter: J) -> Self
    where
        J: IntoIterator<Item = T>,
    {
        IdxVec::from_raw(Vec::from_iter(iter))
    }
}

impl<I: Idx, T> IntoIterator for IdxVec<I, T> {
    type Item = T;
    type IntoIter = vec::IntoIter<T>;

    #[inline]
    fn into_iter(self) -> vec::IntoIter<T> {
        self.raw.into_iter()
    }
}

impl<'a, I: Idx, T> IntoIterator for &'a IdxVec<I, T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> slice::Iter<'a, T> {
        self.iter()
    }
}

impl<'a, I: Idx, T> IntoIterator for &'a mut IdxVec<I, T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> slice::IterMut<'a, T> {
        self.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idx::Idx;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct TestIdx(usize);

    impl Idx for TestIdx {
        fn new(idx: usize) -> Self {
            TestIdx(idx)
        }

        fn idx(&self) -> usize {
            self.0
        }

        fn incr(&mut self) {
            self.0 += 1;
        }

        fn incr_by(&mut self, by: usize) {
            self.0 += by;
        }
    }

    #[test]
    fn test_new() {
        let vec: IdxVec<TestIdx, i32> = IdxVec::new();
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
    }

    #[test]
    fn test_from_raw() {
        let raw = vec![1, 2, 3];
        let idx_vec: IdxVec<TestIdx, i32> = IdxVec::from_raw(raw);
        assert_eq!(idx_vec.len(), 3);
        assert_eq!(idx_vec[TestIdx::new(0)], 1);
        assert_eq!(idx_vec[TestIdx::new(1)], 2);
        assert_eq!(idx_vec[TestIdx::new(2)], 3);
    }

    #[test]
    fn test_with_capacity() {
        let vec: IdxVec<TestIdx, i32> = IdxVec::with_capacity(10);
        assert_eq!(vec.len(), 0);
        assert!(vec.raw.capacity() >= 10);
    }

    #[test]
    fn test_push() {
        let mut vec: IdxVec<TestIdx, i32> = IdxVec::new();
        let idx1 = vec.push(42);
        let idx2 = vec.push(84);
        
        assert_eq!(idx1, TestIdx::new(0));
        assert_eq!(idx2, TestIdx::new(1));
        assert_eq!(vec[idx1], 42);
        assert_eq!(vec[idx2], 84);
        assert_eq!(vec.len(), 2);
    }

    #[test]
    fn test_from_elem_n() {
        let vec: IdxVec<TestIdx, i32> = IdxVec::from_elem_n(5, 3);
        assert_eq!(vec.len(), 3);
        assert_eq!(vec[TestIdx::new(0)], 5);
        assert_eq!(vec[TestIdx::new(1)], 5);
        assert_eq!(vec[TestIdx::new(2)], 5);
    }

    #[test]
    fn test_from_fn_n() {
        let vec: IdxVec<TestIdx, i32> = IdxVec::from_fn_n(|idx: TestIdx| idx.idx() as i32 * 2, 3);
        assert_eq!(vec.len(), 3);
        assert_eq!(vec[TestIdx::new(0)], 0);
        assert_eq!(vec[TestIdx::new(1)], 2);
        assert_eq!(vec[TestIdx::new(2)], 4);
    }

    #[test]
    fn test_pop() {
        let mut vec: IdxVec<TestIdx, i32> = IdxVec::new();
        vec.push(1);
        vec.push(2);
        
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.pop(), Some(1));
        assert_eq!(vec.pop(), None);
        assert_eq!(vec.len(), 0);
    }

    #[test]
    fn test_into_iter_enumerated() {
        let vec: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![10, 20, 30]);
        let items: Vec<_> = vec.into_iter_enumerated().collect();
        
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], (TestIdx::new(0), 10));
        assert_eq!(items[1], (TestIdx::new(1), 20));
        assert_eq!(items[2], (TestIdx::new(2), 30));
    }

    #[test]
    fn test_drain() {
        let mut vec: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![1, 2, 3, 4, 5]);
        let drained: Vec<_> = vec.drain(1..4).collect();
        
        assert_eq!(drained, vec![2, 3, 4]);
        assert_eq!(vec.raw, vec![1, 5]);
    }

    #[test]
    fn test_drain_enumerated() {
        let mut vec: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![10, 20, 30, 40, 50]);
        let drained: Vec<_> = vec.drain_enumerated(1..4).collect();
        
        assert_eq!(drained, vec![(TestIdx::new(1), 20), (TestIdx::new(2), 30), (TestIdx::new(3), 40)]);
        assert_eq!(vec.raw, vec![10, 50]);
    }

    #[test]
    fn test_ensure_contains_elem() {
        let mut vec: IdxVec<TestIdx, i32> = IdxVec::new();
        let elem = TestIdx::new(5);
        
        let value = vec.ensure_contains_elem(elem, || 42);
        assert_eq!(*value, 42);
        assert_eq!(vec.len(), 6);
        assert_eq!(vec[elem], 42);
        
        // Test that it doesn't resize if element already exists
        *vec.ensure_contains_elem(elem, || 99) = 100;
        assert_eq!(vec[elem], 100);
        assert_eq!(vec.len(), 6);
    }

    #[test]
    fn test_resize() {
        let mut vec: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![1, 2]);
        vec.resize(5, 42);
        
        assert_eq!(vec.len(), 5);
        assert_eq!(vec[TestIdx::new(0)], 1);
        assert_eq!(vec[TestIdx::new(1)], 2);
        assert_eq!(vec[TestIdx::new(2)], 42);
        assert_eq!(vec[TestIdx::new(3)], 42);
        assert_eq!(vec[TestIdx::new(4)], 42);
    }

    #[test]
    fn test_resize_to_elem() {
        let mut vec: IdxVec<TestIdx, i32> = IdxVec::new();
        let target = TestIdx::new(3);
        
        vec.resize_to_elem(target, || 99);
        assert_eq!(vec.len(), 4);
        for i in 0..4 {
            assert_eq!(vec[TestIdx::new(i)], 99);
        }
    }

    #[test]
    fn test_append() {
        let mut vec1: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![1, 2]);
        let mut vec2: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![3, 4, 5]);
        
        vec1.append(&mut vec2);
        assert_eq!(vec1.raw, vec![1, 2, 3, 4, 5]);
        assert_eq!(vec2.raw, Vec::<i32>::new());
    }

    #[test]
    fn test_from_iterator() {
        let vec: IdxVec<TestIdx, i32> = [1, 2, 3, 4].iter().copied().collect();
        assert_eq!(vec.len(), 4);
        assert_eq!(vec[TestIdx::new(0)], 1);
        assert_eq!(vec[TestIdx::new(3)], 4);
    }

    #[test]
    fn test_indexing() {
        let vec: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![10, 20, 30]);
        
        assert_eq!(vec[TestIdx::new(0)], 10);
        assert_eq!(vec[TestIdx::new(1)], 20);
        assert_eq!(vec[TestIdx::new(2)], 30);
    }

    #[test]
    fn test_mut_indexing() {
        let mut vec: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![10, 20, 30]);
        
        vec[TestIdx::new(1)] = 99;
        assert_eq!(vec[TestIdx::new(1)], 99);
    }

    #[test]
    fn test_iter() {
        let vec: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![1, 2, 3]);
        let items: Vec<_> = vec.iter().copied().collect();
        assert_eq!(items, vec![1, 2, 3]);
    }

    #[test]
    fn test_iter_mut() {
        let mut vec: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![1, 2, 3]);
        for item in vec.iter_mut() {
            *item *= 2;
        }
        assert_eq!(vec.raw, vec![2, 4, 6]);
    }

    #[test]
    fn test_eq_and_hash() {
        let vec1: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![1, 2, 3]);
        let vec2: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![1, 2, 3]);
        let vec3: IdxVec<TestIdx, i32> = IdxVec::from_raw(vec![1, 2, 4]);

        assert_eq!(vec1, vec2);
        assert_ne!(vec1, vec3);
        
        // Test that they can be used in hash collections
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(vec1);
        set.insert(vec2); // Should not increase size due to equality
        set.insert(vec3);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_default() {
        let vec: IdxVec<TestIdx, i32> = IdxVec::default();
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
    }
}
