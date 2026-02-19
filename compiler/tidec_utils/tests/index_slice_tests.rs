use tidec_utils::idx::Idx;
use tidec_utils::index_slice::IdxSlice;

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

    assert_eq!(
        indices,
        vec![
            TestIdx::new(0),
            TestIdx::new(1),
            TestIdx::new(2),
            TestIdx::new(3)
        ]
    );
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
