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

#[cfg(test)]
mod tests {
    use super::*;

    // Test implementation of Idx trait
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    fn test_idx_new() {
        let idx = TestIdx::new(42);
        assert_eq!(idx.0, 42);
    }

    #[test]
    fn test_idx_idx() {
        let idx = TestIdx::new(123);
        assert_eq!(idx.idx(), 123);
    }

    #[test]
    fn test_idx_incr() {
        let mut idx = TestIdx::new(5);
        idx.incr();
        assert_eq!(idx.idx(), 6);
    }

    #[test]
    fn test_idx_incr_by() {
        let mut idx = TestIdx::new(10);
        idx.incr_by(5);
        assert_eq!(idx.idx(), 15);
    }

    #[test]
    fn test_into_slice_idx_single() {
        let idx = TestIdx::new(3);
        let slice_idx = <TestIdx as IntoSliceIdx<TestIdx, [i32]>>::into_slice_idx(idx);
        assert_eq!(slice_idx, 3);
    }

    #[test]
    fn test_into_slice_idx_range_full() {
        let range_full = ..;
        let slice_idx = <RangeFull as IntoSliceIdx<TestIdx, [i32]>>::into_slice_idx(range_full);
        assert_eq!(slice_idx, ..);
    }

    #[test]
    fn test_into_slice_idx_range() {
        let start = TestIdx::new(1);
        let end = TestIdx::new(5);
        let range = start..end;
        let slice_idx = <Range<TestIdx> as IntoSliceIdx<TestIdx, [i32]>>::into_slice_idx(range);
        assert_eq!(slice_idx, 1..5);
    }

    #[test]
    fn test_into_slice_idx_range_from() {
        let start = TestIdx::new(3);
        let range_from = start..;
        let slice_idx =
            <RangeFrom<TestIdx> as IntoSliceIdx<TestIdx, [i32]>>::into_slice_idx(range_from);
        assert_eq!(slice_idx, 3..);
    }

    #[test]
    fn test_into_slice_idx_range_to() {
        let end = TestIdx::new(7);
        let range_to = ..end;
        let slice_idx =
            <RangeTo<TestIdx> as IntoSliceIdx<TestIdx, [i32]>>::into_slice_idx(range_to);
        assert_eq!(slice_idx, ..7);
    }

    #[test]
    fn test_into_slice_idx_range_inclusive() {
        let start = TestIdx::new(2);
        let end = TestIdx::new(8);
        let range_inclusive = start..=end;
        let slice_idx = <RangeInclusive<TestIdx> as IntoSliceIdx<TestIdx, [i32]>>::into_slice_idx(
            range_inclusive,
        );
        assert_eq!(slice_idx, 2..=8);
    }

    #[test]
    fn test_into_slice_idx_range_to_inclusive() {
        let end = TestIdx::new(6);
        let range_to_inclusive = ..=end;
        let slice_idx = <RangeToInclusive<TestIdx> as IntoSliceIdx<TestIdx, [i32]>>::into_slice_idx(
            range_to_inclusive,
        );
        assert_eq!(slice_idx, ..=6);
    }

    #[test]
    fn test_idx_equality() {
        let idx1 = TestIdx::new(42);
        let idx2 = TestIdx::new(42);
        let idx3 = TestIdx::new(43);

        assert_eq!(idx1, idx2);
        assert_ne!(idx1, idx3);
    }
}
