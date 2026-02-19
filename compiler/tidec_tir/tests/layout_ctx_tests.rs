use tidec_abi::layout::BackendRepr;
use tidec_abi::size_and_align::Size;
use tidec_abi::target::{BackendKind, TirTarget};
use tidec_tir::ctx::{EmitKind, InternCtx, TirArena, TirArgs, TirCtx};
use tidec_tir::layout_ctx::LayoutCtx;
use tidec_tir::ty;

/// Creates a `TirCtx` for testing. Uses the default LLVM target configuration.
fn make_ctx() -> (TirTarget, TirArgs, TirArena<'static>) {
    let target = TirTarget::new(BackendKind::Llvm);
    let args = TirArgs {
        emit_kind: EmitKind::Object,
    };
    let arena = TirArena::default();
    (target, args, arena)
}

#[test]
fn unit_layout_is_zero_sized() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let unit_ty = tir_ctx.intern_ty(ty::TirTy::Unit);
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(unit_ty);

    assert_eq!(layout.size, Size::ZERO, "Unit type should have size 0");
}

#[test]
fn unit_layout_has_memory_repr() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let unit_ty = tir_ctx.intern_ty(ty::TirTy::Unit);
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(unit_ty);

    assert!(
        matches!(layout.backend_repr, BackendRepr::Memory),
        "Unit type should have Memory backend repr, got {:?}",
        layout.backend_repr
    );
}

#[test]
fn i32_layout_is_4_bytes() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let i32_ty = tir_ctx.intern_ty(ty::TirTy::I32);
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(i32_ty);

    assert_eq!(layout.size, Size::from_bytes(4), "I32 should be 4 bytes");
}

#[test]
fn pointer_layout_is_8_bytes_on_64bit() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let i32_ty = tir_ctx.intern_ty(ty::TirTy::I32);
    let ptr_ty = tir_ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Imm));
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(ptr_ty);

    // Default target has 64-bit pointers
    assert_eq!(
        layout.size,
        Size::from_bytes(8),
        "pointer should be 8 bytes on 64-bit target"
    );
}
