use std::{
    ops::{Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive},
    slice::SliceIndex,
};

pub trait Idx: 'static + Eq + PartialEq {
    fn new(idx: usize) -> Self;
    fn idx(&self) -> usize;
    fn incr(&mut self);
    fn incr_by(&mut self, by: usize);
}

pub trait IntoSliceIdx<I, T: ?Sized> {
    type Output: SliceIndex<T>;
    fn into_slice_idx(self) -> Self::Output;
}

impl<I: Idx, T> IntoSliceIdx<I, [T]> for I {
    type Output = usize;
    #[inline]
    fn into_slice_idx(self) -> Self::Output {
        self.idx()
    }
}

impl<I, T> IntoSliceIdx<I, [T]> for RangeFull {
    type Output = RangeFull;
    #[inline]
    fn into_slice_idx(self) -> Self::Output {
        self
    }
}

impl<I: Idx, T> IntoSliceIdx<I, [T]> for Range<I> {
    type Output = Range<usize>;
    #[inline]
    fn into_slice_idx(self) -> Self::Output {
        self.start.idx()..self.end.idx()
    }
}

impl<I: Idx, T> IntoSliceIdx<I, [T]> for RangeFrom<I> {
    type Output = RangeFrom<usize>;
    #[inline]
    fn into_slice_idx(self) -> Self::Output {
        self.start.idx()..
    }
}

impl<I: Idx, T> IntoSliceIdx<I, [T]> for RangeTo<I> {
    type Output = RangeTo<usize>;
    #[inline]
    fn into_slice_idx(self) -> Self::Output {
        ..self.end.idx()
    }
}

impl<I: Idx, T> IntoSliceIdx<I, [T]> for RangeInclusive<I> {
    type Output = RangeInclusive<usize>;
    #[inline]
    fn into_slice_idx(self) -> Self::Output {
        self.start().idx()..=self.end().idx()
    }
}

impl<I: Idx, T> IntoSliceIdx<I, [T]> for RangeToInclusive<I> {
    type Output = RangeToInclusive<usize>;
    #[inline]
    fn into_slice_idx(self) -> Self::Output {
        ..=self.end.idx()
    }
}
