use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tidec_tir::ty::{Mutability, TirTy};
use tidec_utils::interner::{Interner, Ty};

/// A minimal interner for testing `TirTy` in isolation.
#[derive(Debug, Clone, Copy)]
struct DummyInterner;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
struct DummyTy;

impl Interner for DummyInterner {
    type Ty = DummyTy;
}

impl Ty<DummyInterner> for DummyTy {}

fn hash_of<T: Hash>(val: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    val.hash(&mut hasher);
    hasher.finish()
}

// ---- Unit type property tests ----

#[test]
fn unit_is_unit() {
    let ty: TirTy<DummyInterner> = TirTy::Unit;
    assert!(ty.is_unit());
}

#[test]
fn non_unit_types_are_not_unit() {
    let types: Vec<TirTy<DummyInterner>> = vec![
        TirTy::I8,
        TirTy::I32,
        TirTy::U64,
        TirTy::F32,
        TirTy::Metadata,
    ];
    for ty in &types {
        assert!(!ty.is_unit(), "{:?} should not be unit", ty);
    }
}

#[test]
fn unit_is_sized() {
    let ty: TirTy<DummyInterner> = TirTy::Unit;
    assert!(ty.is_sized());
}

#[test]
fn unit_is_not_floating_point() {
    let ty: TirTy<DummyInterner> = TirTy::Unit;
    assert!(!ty.is_floating_point());
}

#[test]
fn unit_is_not_signed_integer() {
    let ty: TirTy<DummyInterner> = TirTy::Unit;
    assert!(!ty.is_signed_integer());
}

#[test]
fn unit_equality() {
    let a: TirTy<DummyInterner> = TirTy::Unit;
    let b: TirTy<DummyInterner> = TirTy::Unit;
    assert_eq!(a, b);
}

#[test]
fn unit_not_equal_to_other_types() {
    let unit: TirTy<DummyInterner> = TirTy::Unit;
    let i32_ty: TirTy<DummyInterner> = TirTy::I32;
    assert_ne!(unit, i32_ty);
}

#[test]
fn unit_hash_is_stable() {
    let a: TirTy<DummyInterner> = TirTy::Unit;
    let b: TirTy<DummyInterner> = TirTy::Unit;
    assert_eq!(hash_of(&a), hash_of(&b));
}

#[test]
fn unit_hash_differs_from_other_types() {
    let unit: TirTy<DummyInterner> = TirTy::Unit;
    let i8_ty: TirTy<DummyInterner> = TirTy::I8;
    // Hash discriminators: Unit=0, I8=1 — they should differ.
    assert_ne!(hash_of(&unit), hash_of(&i8_ty));
}

// ---- Metadata type tests ----

#[test]
fn metadata_is_not_sized() {
    let ty: TirTy<DummyInterner> = TirTy::Metadata;
    assert!(!ty.is_sized());
}

// ---- Mutability tests ----

#[test]
fn mutability_equality() {
    assert_eq!(Mutability::Mut, Mutability::Mut);
    assert_eq!(Mutability::Imm, Mutability::Imm);
    assert_ne!(Mutability::Mut, Mutability::Imm);
}
