use tidec_abi::layout::{BackendRepr, Primitive};
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

#[test]
fn bool_layout_is_1_byte() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let bool_ty = tir_ctx.intern_ty(ty::TirTy::Bool);
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(bool_ty);

    assert_eq!(layout.size, Size::from_bytes(1), "Bool should be 1 byte");
}

#[test]
fn bool_layout_has_scalar_u8_repr() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let bool_ty = tir_ctx.intern_ty(ty::TirTy::Bool);
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(bool_ty);

    assert!(
        matches!(layout.backend_repr, BackendRepr::Scalar(Primitive::U8)),
        "Bool should have Scalar(U8) backend repr, got {:?}",
        layout.backend_repr
    );
}

// ---- Struct layout tests ----

#[test]
fn struct_empty_layout_is_zero_sized() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let fields = tir_ctx.intern_type_list(&[]);
    let struct_ty = tir_ctx.intern_ty(ty::TirTy::Struct {
        fields,
        packed: false,
    });
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(struct_ty);

    assert_eq!(layout.size, Size::ZERO, "Empty struct should have size 0");
    assert!(
        matches!(layout.backend_repr, BackendRepr::Memory),
        "Empty struct should have Memory backend repr, got {:?}",
        layout.backend_repr
    );
}

#[test]
fn struct_single_i32_field_layout() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let i32_ty = tir_ctx.intern_ty(ty::TirTy::I32);
    let fields = tir_ctx.intern_type_list(&[i32_ty]);
    let struct_ty = tir_ctx.intern_ty(ty::TirTy::Struct {
        fields,
        packed: false,
    });
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(struct_ty);

    assert_eq!(
        layout.size,
        Size::from_bytes(4),
        "Struct {{ i32 }} should be 4 bytes"
    );
    assert!(
        matches!(layout.backend_repr, BackendRepr::Memory),
        "Struct should have Memory backend repr"
    );
}

#[test]
fn struct_two_i32_fields_layout() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let i32_ty = tir_ctx.intern_ty(ty::TirTy::I32);
    let fields = tir_ctx.intern_type_list(&[i32_ty, i32_ty]);
    let struct_ty = tir_ctx.intern_ty(ty::TirTy::Struct {
        fields,
        packed: false,
    });
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(struct_ty);

    assert_eq!(
        layout.size,
        Size::from_bytes(8),
        "Struct {{ i32, i32 }} should be 8 bytes"
    );
}

#[test]
fn struct_i8_i32_padding_layout() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let i8_ty = tir_ctx.intern_ty(ty::TirTy::I8);
    let i32_ty = tir_ctx.intern_ty(ty::TirTy::I32);
    let fields = tir_ctx.intern_type_list(&[i8_ty, i32_ty]);
    let struct_ty = tir_ctx.intern_ty(ty::TirTy::Struct {
        fields,
        packed: false,
    });
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(struct_ty);

    // C layout: i8 (1 byte) + 3 bytes padding + i32 (4 bytes) = 8 bytes
    assert_eq!(
        layout.size,
        Size::from_bytes(8),
        "Struct {{ i8, i32 }} should be 8 bytes (with padding)"
    );
}

#[test]
fn struct_packed_no_padding() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let i8_ty = tir_ctx.intern_ty(ty::TirTy::I8);
    let i32_ty = tir_ctx.intern_ty(ty::TirTy::I32);
    let fields = tir_ctx.intern_type_list(&[i8_ty, i32_ty]);
    let struct_ty = tir_ctx.intern_ty(ty::TirTy::Struct {
        fields,
        packed: true,
    });
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(struct_ty);

    // Packed: i8 (1 byte) + i32 (4 bytes) = 5 bytes, no padding
    assert_eq!(
        layout.size,
        Size::from_bytes(5),
        "Packed struct {{ i8, i32 }} should be 5 bytes"
    );
}

#[test]
fn struct_f64_i8_alignment() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let f64_ty = tir_ctx.intern_ty(ty::TirTy::F64);
    let i8_ty = tir_ctx.intern_ty(ty::TirTy::I8);
    let fields = tir_ctx.intern_type_list(&[f64_ty, i8_ty]);
    let struct_ty = tir_ctx.intern_ty(ty::TirTy::Struct {
        fields,
        packed: false,
    });
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(struct_ty);

    // C layout: f64 (8 bytes) + i8 (1 byte) + 7 bytes tail padding = 16 bytes
    assert_eq!(
        layout.size,
        Size::from_bytes(16),
        "Struct {{ f64, i8 }} should be 16 bytes (with tail padding)"
    );
}

// ---- Array layout tests ----

#[test]
fn array_i32_3_layout() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let i32_ty = tir_ctx.intern_ty(ty::TirTy::I32);
    let array_ty = tir_ctx.intern_ty(ty::TirTy::Array(i32_ty, 3));
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(array_ty);

    assert_eq!(
        layout.size,
        Size::from_bytes(12),
        "[i32; 3] should be 12 bytes"
    );
    assert!(
        matches!(layout.backend_repr, BackendRepr::Memory),
        "Array should have Memory backend repr"
    );
}

#[test]
fn array_f64_2_layout() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let f64_ty = tir_ctx.intern_ty(ty::TirTy::F64);
    let array_ty = tir_ctx.intern_ty(ty::TirTy::Array(f64_ty, 2));
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(array_ty);

    assert_eq!(
        layout.size,
        Size::from_bytes(16),
        "[f64; 2] should be 16 bytes"
    );
}

#[test]
fn array_zero_length_is_zero_sized() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let i32_ty = tir_ctx.intern_ty(ty::TirTy::I32);
    let array_ty = tir_ctx.intern_ty(ty::TirTy::Array(i32_ty, 0));
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(array_ty);

    assert_eq!(layout.size, Size::ZERO, "[i32; 0] should have size 0");
    assert!(
        matches!(layout.backend_repr, BackendRepr::Memory),
        "Zero-length array should have Memory backend repr"
    );
}

#[test]
fn array_i8_5_layout() {
    let (target, args, arena) = make_ctx();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);

    let i8_ty = tir_ctx.intern_ty(ty::TirTy::I8);
    let array_ty = tir_ctx.intern_ty(ty::TirTy::Array(i8_ty, 5));
    let layout_ctx = LayoutCtx::new(tir_ctx);
    let layout = layout_ctx.compute_layout(array_ty);

    assert_eq!(
        layout.size,
        Size::from_bytes(5),
        "[i8; 5] should be 5 bytes"
    );
}
