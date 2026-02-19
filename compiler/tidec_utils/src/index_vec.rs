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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
