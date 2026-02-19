use tidec_utils::idx::Idx;
use tidec_utils::index_vec::IdxVec;

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

    assert_eq!(
        drained,
        vec![
            (TestIdx::new(1), 20),
            (TestIdx::new(2), 30),
            (TestIdx::new(3), 40)
        ]
    );
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
