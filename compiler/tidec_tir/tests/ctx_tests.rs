use tidec_abi::size_and_align::Size;
use tidec_abi::target::{BackendKind, TirTarget};
use tidec_tir::alloc::{Allocation, GlobalAlloc};
use tidec_tir::body::DefId;
use tidec_tir::ctx::{EmitKind, GlobalAllocMap, InternCtx, TirArena, TirArgs, TirCtx};
use tidec_tir::ty;

/// Helper to build a `TirCtx` for type-interning tests.
fn make_tir_ctx_components() -> (TirTarget, TirArgs) {
    let target = TirTarget::new(BackendKind::Llvm);
    let args = TirArgs {
        emit_kind: EmitKind::Object,
    };
    (target, args)
}

#[test]
fn test_allocation_interning_deduplication() {
    let arena = TirArena::default();
    let intern_ctx = InternCtx::new(&arena);

    // Intern the same string twice
    let alloc1 = Allocation::from_c_str("hello");
    let alloc2 = Allocation::from_c_str("hello");

    let interned1 = intern_ctx.intern_allocation(alloc1);
    let interned2 = intern_ctx.intern_allocation(alloc2);

    // They should be equal (deduplicated via Interned)
    assert_eq!(interned1, interned2);
}

#[test]
fn test_different_allocations_not_deduplicated() {
    let arena = TirArena::default();
    let intern_ctx = InternCtx::new(&arena);

    let alloc1 = Allocation::from_c_str("hello");
    let alloc2 = Allocation::from_c_str("world");

    let interned1 = intern_ctx.intern_allocation(alloc1);
    let interned2 = intern_ctx.intern_allocation(alloc2);

    // They should be different (not deduplicated)
    assert_ne!(interned1, interned2);
}

#[test]
fn test_global_alloc_map() {
    let alloc_map = GlobalAllocMap::new();

    // Insert a function allocation
    let def_id = DefId(42);
    let alloc_id = alloc_map.insert(GlobalAlloc::Function(def_id));

    // Retrieve it
    let retrieved = alloc_map.get_unwrap(alloc_id);
    assert!(matches!(retrieved, GlobalAlloc::Function(id) if id == def_id));
}

// ---- Unit type interning tests ----

#[test]
fn test_intern_unit_type_deduplication() {
    let (target, args) = make_tir_ctx_components();
    let arena = TirArena::default();
    let intern_ctx = InternCtx::new(&arena);
    let ctx = TirCtx::new(&target, &args, &intern_ctx);

    let unit1 = ctx.intern_ty(ty::TirTy::Unit);
    let unit2 = ctx.intern_ty(ty::TirTy::Unit);

    // Interning the same type twice yields pointer-equal values.
    assert_eq!(unit1, unit2);
}

#[test]
fn test_unit_not_equal_to_i32() {
    let (target, args) = make_tir_ctx_components();
    let arena = TirArena::default();
    let intern_ctx = InternCtx::new(&arena);
    let ctx = TirCtx::new(&target, &args, &intern_ctx);

    let unit = ctx.intern_ty(ty::TirTy::Unit);
    let i32_ty = ctx.intern_ty(ty::TirTy::I32);

    assert_ne!(unit, i32_ty);
}

#[test]
fn test_unit_type_layout_via_ctx() {
    let (target, args) = make_tir_ctx_components();
    let arena = TirArena::default();
    let intern_ctx = InternCtx::new(&arena);
    let ctx = TirCtx::new(&target, &args, &intern_ctx);

    let unit = ctx.intern_ty(ty::TirTy::Unit);
    let ty_and_layout = ctx.layout_of(unit);

    assert_eq!(ty_and_layout.layout.size, Size::ZERO);
    assert_eq!(ty_and_layout.ty, unit);
}

#[test]
fn test_raw_ptr_interning() {
    let (target, args) = make_tir_ctx_components();
    let arena = TirArena::default();
    let intern_ctx = InternCtx::new(&arena);
    let ctx = TirCtx::new(&target, &args, &intern_ctx);

    let i32_ty = ctx.intern_ty(ty::TirTy::I32);
    let ptr1 = ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Imm));
    let ptr2 = ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Imm));

    assert_eq!(ptr1, ptr2, "Identical pointer types should be deduplicated");
}

#[test]
fn test_raw_ptr_mut_differs_from_imm() {
    let (target, args) = make_tir_ctx_components();
    let arena = TirArena::default();
    let intern_ctx = InternCtx::new(&arena);
    let ctx = TirCtx::new(&target, &args, &intern_ctx);

    let i32_ty = ctx.intern_ty(ty::TirTy::I32);
    let ptr_imm = ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Imm));
    let ptr_mut = ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Mut));

    assert_ne!(ptr_imm, ptr_mut);
}
