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
