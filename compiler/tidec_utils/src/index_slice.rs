//! A slice-like data structure that uses an index type to access elements.
//!
//! It is inspired by the `IndexSlice` type from the `rustc` compiler.

use crate::idx::{Idx, IntoSliceIdx};
use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
    slice::{self, SliceIndex},
};

/// A view into contiguous `T`s, indexed by `I` rather than by `usize`.
///
/// One common pattern you'll see is code that uses [`IdxVec::from_elem`]
/// to create the storage needed for a particular "universe" (aka the set of all
/// the possible keys that need an associated value) then passes that working
/// area as `&mut IdxSlice<I, T>` to clarify that nothing will be added nor
/// removed during processing (and, as a bonus, to chase fewer pointers).
#[derive(PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct IdxSlice<I: Idx, T> {
    _marker: PhantomData<fn(&I)>,
    pub raw: [T],
}

impl<I: Idx, T> IdxSlice<I, T> {
    #[inline]
    pub const fn empty<'a>() -> &'a Self {
        Self::from_raw(&[])
    }

    #[inline]
    pub const fn from_raw(raw: &[T]) -> &Self {
        let ptr: *const [T] = raw;
        // SAFETY: `IdxSlice` is `repr(transparent)` over a normal slice
        unsafe { &*(ptr as *const Self) }
    }

    #[inline]
    pub fn from_raw_mut(raw: &mut [T]) -> &mut Self {
        let ptr: *mut [T] = raw;
        // SAFETY: `IdxSlice` is `repr(transparent)` over a normal slice
        unsafe { &mut *(ptr as *mut Self) }
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.raw.len()
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// Gives the next index that will be assigned when `push` is called.
    ///
    /// Manual bounds checks can be done using `idx < slice.next_index()`
    /// (as opposed to `idx.index() < slice.len()`).
    #[inline]
    pub fn next_index(&self) -> I {
        I::new(self.len())
    }

    #[inline]
    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.raw.iter()
    }

    #[inline]
    pub fn iter_enumerated(&self) -> impl DoubleEndedIterator<Item = (I, &T)> + ExactSizeIterator {
        self.raw.iter().enumerate().map(|(n, t)| (I::new(n), t))
    }

    #[inline]
    pub fn indices(
        &self,
    ) -> impl DoubleEndedIterator<Item = I> + ExactSizeIterator + Clone + 'static {
        (0..self.len()).map(|n| I::new(n))
    }

    #[inline]
    pub fn iter_mut(&mut self) -> slice::IterMut<'_, T> {
        self.raw.iter_mut()
    }

    #[inline]
    pub fn iter_enumerated_mut(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = (I, &mut T)> + ExactSizeIterator {
        self.raw.iter_mut().enumerate().map(|(n, t)| (I::new(n), t))
    }

    #[inline]
    pub fn last_index(&self) -> Option<I> {
        self.len().checked_sub(1).map(I::new)
    }

    #[inline]
    pub fn swap(&mut self, a: I, b: I) {
        self.raw.swap(a.idx(), b.idx())
    }

    #[inline]
    pub fn get<R: IntoSliceIdx<I, [T]>>(
        &self,
        index: R,
    ) -> Option<&<R::Output as SliceIndex<[T]>>::Output> {
        self.raw.get(index.into_slice_idx())
    }

    #[inline]
    pub fn get_mut<R: IntoSliceIdx<I, [T]>>(
        &mut self,
        index: R,
    ) -> Option<&mut <R::Output as SliceIndex<[T]>>::Output> {
        self.raw.get_mut(index.into_slice_idx())
    }

    /// Returns mutable references to two distinct elements, `a` and `b`.
    ///
    /// Panics if `a == b`.
    #[inline]
    pub fn pick2_mut(&mut self, a: I, b: I) -> (&mut T, &mut T) {
        let (ai, bi) = (a.idx(), b.idx());
        assert!(ai != bi);

        if ai < bi {
            let (c1, c2) = self.raw.split_at_mut(bi);
            (&mut c1[ai], &mut c2[0])
        } else {
            let (c2, c1) = self.pick2_mut(b, a);
            (c1, c2)
        }
    }

    /// Returns mutable references to three distinct elements.
    ///
    /// Panics if the elements are not distinct.
    #[inline]
    pub fn pick3_mut(&mut self, a: I, b: I, c: I) -> (&mut T, &mut T, &mut T) {
        let (ai, bi, ci) = (a.idx(), b.idx(), c.idx());
        assert!(ai != bi && bi != ci && ci != ai);
        let len = self.raw.len();
        assert!(ai < len && bi < len && ci < len);
        let ptr = self.raw.as_mut_ptr();
        unsafe { (&mut *ptr.add(ai), &mut *ptr.add(bi), &mut *ptr.add(ci)) }
    }

    #[inline]
    pub fn binary_search(&self, value: &T) -> Result<I, I>
    where
        T: Ord,
    {
        match self.raw.binary_search(value) {
            Ok(i) => Ok(Idx::new(i)),
            Err(i) => Err(Idx::new(i)),
        }
    }
}

////////// Trait implementations  //////////

impl<I: Idx, T, R: IntoSliceIdx<I, [T]>> Index<R> for IdxSlice<I, T> {
    type Output = <R::Output as SliceIndex<[T]>>::Output;

    #[inline]
    fn index(&self, index: R) -> &Self::Output {
        &self.raw[index.into_slice_idx()]
    }
}

impl<I: Idx, T, R: IntoSliceIdx<I, [T]>> IndexMut<R> for IdxSlice<I, T> {
    #[inline]
    fn index_mut(&mut self, index: R) -> &mut Self::Output {
        &mut self.raw[index.into_slice_idx()]
    }
}

impl<'a, I: Idx, T> IntoIterator for &'a IdxSlice<I, T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> slice::Iter<'a, T> {
        self.raw.iter()
    }
}

impl<'a, I: Idx, T> IntoIterator for &'a mut IdxSlice<I, T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> slice::IterMut<'a, T> {
        self.raw.iter_mut()
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
    fn test_empty() {
        let slice: &IdxSlice<TestIdx, i32> = IdxSlice::empty();
        assert_eq!(slice.len(), 0);
        assert!(slice.is_empty());
    }

    #[test]
    fn test_from_raw() {
        let raw = [1, 2, 3, 4, 5];
        let slice = IdxSlice::from_raw(&raw);
        
        assert_eq!(slice.len(), 5);
        assert!(!slice.is_empty());
        assert_eq!(slice[TestIdx::new(0)], 1);
        assert_eq!(slice[TestIdx::new(4)], 5);
    }

    #[test]
    fn test_from_raw_mut() {
        let mut raw = [1, 2, 3];
        let slice = IdxSlice::from_raw_mut(&mut raw);
        
        slice[TestIdx::new(1)] = 99;
        assert_eq!(raw[1], 99);
    }

    #[test]
    fn test_next_index() {
        let raw = [10, 20, 30];
        let slice: &IdxSlice<TestIdx, i32> = IdxSlice::from_raw(&raw);
        let next = slice.next_index();
        
        assert_eq!(next, TestIdx::new(3));
    }

    #[test]
    fn test_iter() {
        let raw = [1, 2, 3];
        let slice: &IdxSlice<TestIdx, i32> = IdxSlice::from_raw(&raw);
        let items: Vec<_> = slice.iter().copied().collect();
        
        assert_eq!(items, vec![1, 2, 3]);
    }

    #[test]
    fn test_iter_enumerated() {
        let raw = [10, 20, 30];
        let slice: &IdxSlice<TestIdx, i32> = IdxSlice::from_raw(&raw);
        let items: Vec<_> = slice.iter_enumerated().collect();
        
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], (TestIdx::new(0), &10));
        assert_eq!(items[1], (TestIdx::new(1), &20));
        assert_eq!(items[2], (TestIdx::new(2), &30));
    }

    #[test]
    fn test_indices() {
        let raw = [1, 2, 3, 4];
        let slice: &IdxSlice<TestIdx, i32> = IdxSlice::from_raw(&raw);
        let indices: Vec<_> = slice.indices().collect();
        
        assert_eq!(indices, vec![TestIdx::new(0), TestIdx::new(1), TestIdx::new(2), TestIdx::new(3)]);
    }

    #[test]
    fn test_iter_mut() {
        let mut raw = [1, 2, 3];
        let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
        
        for item in slice.iter_mut() {
            *item *= 2;
        }
        
        assert_eq!(raw, [2, 4, 6]);
    }

    #[test]
    fn test_iter_enumerated_mut() {
        let mut raw = [10, 20, 30];
        let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
        
        for (idx, item) in slice.iter_enumerated_mut() {
            *item = (idx.idx() * 100) as i32;
        }
        
        assert_eq!(raw, [0, 100, 200]);
    }

    #[test]
    fn test_last_index() {
        let raw = [1, 2, 3];
        let slice: &IdxSlice<TestIdx, i32> = IdxSlice::from_raw(&raw);
        assert_eq!(slice.last_index(), Some(TestIdx::new(2)));
        
        let empty_raw: [i32; 0] = [];
        let empty_slice: &IdxSlice<TestIdx, i32> = IdxSlice::from_raw(&empty_raw);
        assert_eq!(empty_slice.last_index(), None);
    }

    #[test]
    fn test_swap() {
        let mut raw = [1, 2, 3, 4];
        {
            let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
            slice.swap(TestIdx::new(0), TestIdx::new(3));
        }
        assert_eq!(raw, [4, 2, 3, 1]);
        
        {
            let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
            slice.swap(TestIdx::new(1), TestIdx::new(2));
        }
        assert_eq!(raw, [4, 3, 2, 1]);
    }

    #[test]
    fn test_get() {
        let raw = [10, 20, 30, 40, 50];
        let slice: &IdxSlice<TestIdx, i32> = IdxSlice::from_raw(&raw);
        
        assert_eq!(slice.get(TestIdx::new(2)), Some(&30));
        assert_eq!(slice.get(TestIdx::new(10)), None);
        
        // Test range get
        let range_result = slice.get(TestIdx::new(1)..TestIdx::new(4));
        assert_eq!(range_result, Some(&[20, 30, 40][..]));
    }

    #[test]
    fn test_get_mut() {
        let mut raw = [10, 20, 30, 40, 50];
        {
            let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
            if let Some(item) = slice.get_mut(TestIdx::new(2)) {
                *item = 99;
            }
            assert!(slice.get_mut(TestIdx::new(10)).is_none());
        }
        assert_eq!(raw[2], 99);
    }

    #[test]
    fn test_pick2_mut() {
        let mut raw = [1, 2, 3, 4, 5];
        {
            let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
            let (a, b) = slice.pick2_mut(TestIdx::new(1), TestIdx::new(3));
            *a = 99;
            *b = 88;
        }
        assert_eq!(raw, [1, 99, 3, 88, 5]);
        
        {
            let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
            let (c, d) = slice.pick2_mut(TestIdx::new(4), TestIdx::new(0));
            *c = 77;
            *d = 66;
        }
        assert_eq!(raw, [66, 99, 3, 88, 77]);
    }

    #[test]
    #[should_panic]
    fn test_pick2_mut_panic_same_index() {
        let mut raw = [1, 2, 3];
        let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
        slice.pick2_mut(TestIdx::new(1), TestIdx::new(1));
    }

    #[test]
    fn test_pick3_mut() {
        let mut raw = [1, 2, 3, 4, 5];
        {
            let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
            let (a, b, c) = slice.pick3_mut(TestIdx::new(0), TestIdx::new(2), TestIdx::new(4));
            *a = 10;
            *b = 30;
            *c = 50;
        }
        assert_eq!(raw, [10, 2, 30, 4, 50]);
    }

    #[test]
    #[should_panic]
    fn test_pick3_mut_panic_duplicate_indices() {
        let mut raw = [1, 2, 3, 4, 5];
        let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
        slice.pick3_mut(TestIdx::new(0), TestIdx::new(2), TestIdx::new(0));
    }

    #[test]
    fn test_binary_search() {
        let raw = [10, 20, 30, 40, 50];
        let slice: &IdxSlice<TestIdx, i32> = IdxSlice::from_raw(&raw);
        
        assert_eq!(slice.binary_search(&30), Ok(TestIdx::new(2)));
        assert_eq!(slice.binary_search(&35), Err(TestIdx::new(3)));
        assert_eq!(slice.binary_search(&5), Err(TestIdx::new(0)));
        assert_eq!(slice.binary_search(&60), Err(TestIdx::new(5)));
    }

    #[test]
    fn test_index_operations() {
        let raw = [100, 200, 300, 400, 500];
        let slice: &IdxSlice<TestIdx, i32> = IdxSlice::from_raw(&raw);
        
        // Test single index
        assert_eq!(slice[TestIdx::new(2)], 300);
        
        // Test range indexing
        let sub_slice = &slice[TestIdx::new(1)..TestIdx::new(4)];
        assert_eq!(sub_slice, &[200, 300, 400]);
    }

    #[test]
    fn test_index_mut_operations() {
        let mut raw = [1, 2, 3, 4, 5];
        {
            let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
            slice[TestIdx::new(2)] = 99;
        }
        assert_eq!(raw[2], 99);
        
        {
            let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
            // Test range mutable indexing
            let sub_slice = &mut slice[TestIdx::new(0)..TestIdx::new(2)];
            sub_slice[0] = 88;
            sub_slice[1] = 77;
        }
        assert_eq!(raw, [88, 77, 99, 4, 5]);
    }

    #[test]
    fn test_into_iterator() {
        let raw = [1, 2, 3, 4];
        let slice: &IdxSlice<TestIdx, i32> = IdxSlice::from_raw(&raw);
        
        let items: Vec<_> = slice.into_iter().copied().collect();
        assert_eq!(items, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_into_iterator_mut() {
        let mut raw = [1, 2, 3, 4];
        {
            let slice: &mut IdxSlice<TestIdx, i32> = IdxSlice::from_raw_mut(&mut raw);
            for item in slice.into_iter() {
                *item *= 3;
            }
        }
        assert_eq!(raw, [3, 6, 9, 12]);
    }
}
