use tidec_abi::size_and_align::Size;
use tidec_tir::alloc::{AllocId, Allocation};

#[test]
fn test_alloc_id_unique() {
    let id1 = AllocId::new();
    let id2 = AllocId::new();
    assert_ne!(id1, id2);
}

#[test]
fn test_allocation_from_c_str() {
    let alloc = Allocation::from_c_str("hello");
    assert_eq!(alloc.bytes(), b"hello\0");
    assert_eq!(alloc.size(), Size::from_bytes(6));
}
