use tidec_abi::target::{BackendKind, TirTarget};
use tidec_tir::ctx::{EmitKind, InternCtx, TirArena, TirArgs, TirCtx};
use tidec_tir::syntax::*;
use tidec_tir::ty;
use tidec_utils::idx::Idx;

/// Helper to create a TirCtx for interning types in tests.
fn with_ctx<F, R>(f: F) -> R
where
    F: for<'ctx> FnOnce(TirCtx<'ctx>) -> R,
{
    let target = TirTarget::new(BackendKind::Llvm);
    let args = TirArgs {
        emit_kind: EmitKind::Object,
    };
    let arena = TirArena::default();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);
    f(tir_ctx)
}

// ---- Local tests ----

#[test]
fn local_next_increments() {
    let l = Local::new(0);
    assert_eq!(l.next(), Local::new(1));
}

#[test]
fn return_local_is_zero() {
    assert_eq!(RETURN_LOCAL, Local::new(0));
}

#[test]
fn local_idx_trait() {
    let mut l = Local::new(3);
    assert_eq!(l.idx(), 3);
    l.incr();
    assert_eq!(l.idx(), 4);
    l.incr_by(10);
    assert_eq!(l.idx(), 14);
}

// ---- Place tests ----

#[test]
fn place_from_local_has_empty_projection() {
    let local = Local::new(5);
    let place: Place<'_> = Place::from(local);
    assert_eq!(place.local, Local::new(5));
    assert!(place.projection.is_empty());
}

#[test]
fn place_try_local_without_projections() {
    let place: Place<'_> = Place::from(Local::new(3));
    assert_eq!(place.try_local(), Some(Local::new(3)));
}

#[test]
fn place_try_local_with_projection_returns_none() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let place: Place<'_> = Place {
            local: Local::new(0),
            projection: vec![Projection::Field(0, i32_ty)],
        };
        assert!(place.try_local().is_none());
    });
}

// ---- Projection variant construction tests ----

#[test]
fn projection_deref_variant() {
    let proj: Projection<'_> = Projection::Deref;
    assert!(matches!(proj, Projection::Deref));
}

#[test]
fn projection_field_variant() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let proj = Projection::Field(2, i32_ty);
        match proj {
            Projection::Field(idx, ty) => {
                assert_eq!(idx, 2);
                assert_eq!(ty, i32_ty);
            }
            _ => panic!("Expected Field variant"),
        }
    });
}

#[test]
fn projection_index_variant() {
    let proj: Projection<'_> = Projection::Index(Local::new(7));
    match proj {
        Projection::Index(local) => assert_eq!(local, Local::new(7)),
        _ => panic!("Expected Index variant"),
    }
}

#[test]
fn projection_constant_index_variant() {
    let proj: Projection<'_> = Projection::ConstantIndex {
        offset: 3,
        from_end: true,
        min_length: 10,
    };
    match proj {
        Projection::ConstantIndex {
            offset,
            from_end,
            min_length,
        } => {
            assert_eq!(offset, 3);
            assert!(from_end);
            assert_eq!(min_length, 10);
        }
        _ => panic!("Expected ConstantIndex variant"),
    }
}

#[test]
fn projection_subslice_variant() {
    let proj: Projection<'_> = Projection::Subslice {
        from: 1,
        to: 5,
        from_end: false,
    };
    match proj {
        Projection::Subslice { from, to, from_end } => {
            assert_eq!(from, 1);
            assert_eq!(to, 5);
            assert!(!from_end);
        }
        _ => panic!("Expected Subslice variant"),
    }
}

#[test]
fn projection_downcast_variant() {
    let proj: Projection<'_> = Projection::Downcast(42);
    match proj {
        Projection::Downcast(idx) => assert_eq!(idx, 42),
        _ => panic!("Expected Downcast variant"),
    }
}

// ---- Place with projection chain ----

#[test]
fn place_with_deref_and_field_chain() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let place: Place<'_> = Place {
            local: Local::new(1),
            projection: vec![Projection::Deref, Projection::Field(0, i32_ty)],
        };
        assert_eq!(place.local, Local::new(1));
        assert_eq!(place.projection.len(), 2);
        assert!(matches!(place.projection[0], Projection::Deref));
        assert!(matches!(place.projection[1], Projection::Field(0, _)));
    });
}

// ---- Statement and Terminator construction ----

#[test]
fn statement_assign_with_place() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let place: Place<'_> = Place::from(RETURN_LOCAL);
        let const_op = ConstOperand::Value(ConstValue::ZST, i32_ty);
        let rv = RValue::Operand(Operand::Const(const_op));
        let stmt = Statement::Assign(Box::new((place, rv)));
        assert!(matches!(stmt, Statement::Assign(_)));
    });
}

// ---- Terminator construction tests ----

#[test]
fn terminator_return() {
    let term: Terminator<'_> = Terminator::Return;
    assert!(matches!(term, Terminator::Return));
}

#[test]
fn terminator_goto() {
    let target = BasicBlock::new(3);
    let term: Terminator<'_> = Terminator::Goto { target };
    match term {
        Terminator::Goto { target: t } => assert_eq!(t, BasicBlock::new(3)),
        _ => panic!("Expected Goto variant"),
    }
}

#[test]
fn terminator_unreachable() {
    let term: Terminator<'_> = Terminator::Unreachable;
    assert!(matches!(term, Terminator::Unreachable));
}

#[test]
fn terminator_switch_int() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let discr = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 1,
                size: std::num::NonZero::new(4).unwrap(),
            })),
            i32_ty,
        ));
        let targets = SwitchTargets::new(
            vec![(0, BasicBlock::new(1)), (1, BasicBlock::new(2))],
            BasicBlock::new(3),
        );
        let term = Terminator::SwitchInt { discr, targets };
        assert!(matches!(term, Terminator::SwitchInt { .. }));
    });
}

// ---- SwitchTargets tests ----

#[test]
fn switch_targets_new() {
    let targets = SwitchTargets::new(
        vec![(10, BasicBlock::new(1)), (20, BasicBlock::new(2))],
        BasicBlock::new(0),
    );
    assert_eq!(targets.len(), 2);
    assert!(!targets.is_empty());
    assert_eq!(targets.otherwise, BasicBlock::new(0));
}

#[test]
fn switch_targets_if_then() {
    let targets = SwitchTargets::if_then(BasicBlock::new(1), BasicBlock::new(2));
    assert_eq!(targets.len(), 1);
    assert_eq!(targets.otherwise, BasicBlock::new(2));
    let arms: Vec<_> = targets.iter().collect();
    assert_eq!(arms.len(), 1);
    assert_eq!(arms[0], (1, BasicBlock::new(1)));
}

#[test]
fn switch_targets_empty() {
    let targets = SwitchTargets::new(vec![], BasicBlock::new(5));
    assert!(targets.is_empty());
    assert_eq!(targets.len(), 0);
    assert_eq!(targets.otherwise, BasicBlock::new(5));
}

#[test]
fn switch_targets_iter() {
    let targets = SwitchTargets::new(
        vec![
            (100, BasicBlock::new(1)),
            (200, BasicBlock::new(2)),
            (300, BasicBlock::new(3)),
        ],
        BasicBlock::new(0),
    );
    let arms: Vec<_> = targets.iter().collect();
    assert_eq!(arms.len(), 3);
    assert_eq!(arms[0], (100, BasicBlock::new(1)));
    assert_eq!(arms[1], (200, BasicBlock::new(2)));
    assert_eq!(arms[2], (300, BasicBlock::new(3)));
}

// ---- BinaryOp comparison tests ----

#[test]
fn comparison_ops_return_bool_type() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let bool_ty = ctx.intern_ty(ty::TirTy::Bool);

        let ops = [
            BinaryOp::Eq,
            BinaryOp::Ne,
            BinaryOp::Lt,
            BinaryOp::Le,
            BinaryOp::Gt,
            BinaryOp::Ge,
        ];
        for op in &ops {
            let result_ty = op.ty(&ctx, i32_ty, i32_ty);
            assert_eq!(
                result_ty, bool_ty,
                "{:?} should return Bool, got {:?}",
                op, result_ty
            );
        }
    });
}

#[test]
fn arithmetic_ops_return_lhs_type() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);

        let ops = [BinaryOp::Add, BinaryOp::Sub, BinaryOp::Mul, BinaryOp::Div];
        for op in &ops {
            let result_ty = op.ty(&ctx, i32_ty, i32_ty);
            assert_eq!(
                result_ty, i32_ty,
                "{:?} should return I32, got {:?}",
                op, result_ty
            );
        }
    });
}

// ---- Remainder, Bitwise, Shift ops return lhs type ----

#[test]
fn remainder_op_returns_lhs_type() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let result_ty = BinaryOp::Rem.ty(&ctx, i32_ty, i32_ty);
        assert_eq!(result_ty, i32_ty);
    });
}

#[test]
fn bitwise_ops_return_lhs_type() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let ops = [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::BitXor];
        for op in &ops {
            let result_ty = op.ty(&ctx, i32_ty, i32_ty);
            assert_eq!(
                result_ty, i32_ty,
                "{:?} should return I32, got {:?}",
                op, result_ty
            );
        }
    });
}

#[test]
fn shift_ops_return_lhs_type() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let ops = [BinaryOp::Shl, BinaryOp::Shr];
        for op in &ops {
            let result_ty = op.ty(&ctx, i32_ty, i32_ty);
            assert_eq!(
                result_ty, i32_ty,
                "{:?} should return I32, got {:?}",
                op, result_ty
            );
        }
    });
}

#[test]
fn unary_not_variant() {
    let op = UnaryOp::Not;
    assert!(matches!(op, UnaryOp::Not));
}

// ---- UnaryOp position and negation variant tests ----

#[test]
fn unary_pos_variant() {
    let op = UnaryOp::Pos;
    assert!(matches!(op, UnaryOp::Pos));
}

#[test]
fn unary_neg_variant() {
    let op = UnaryOp::Neg;
    assert!(matches!(op, UnaryOp::Neg));
}

// ---- Remainder with different type families ----

#[test]
fn remainder_unsigned_returns_lhs_type() {
    with_ctx(|ctx| {
        let u32_ty = ctx.intern_ty(ty::TirTy::U32);
        let result_ty = BinaryOp::Rem.ty(&ctx, u32_ty, u32_ty);
        assert_eq!(result_ty, u32_ty);
    });
}

#[test]
fn remainder_float_returns_lhs_type() {
    with_ctx(|ctx| {
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);
        let result_ty = BinaryOp::Rem.ty(&ctx, f64_ty, f64_ty);
        assert_eq!(result_ty, f64_ty);
    });
}

// ---- Shift ops with unsigned type ----

#[test]
fn shift_ops_unsigned_return_lhs_type() {
    with_ctx(|ctx| {
        let u64_ty = ctx.intern_ty(ty::TirTy::U64);
        let ops = [BinaryOp::Shl, BinaryOp::Shr];
        for op in &ops {
            let result_ty = op.ty(&ctx, u64_ty, u64_ty);
            assert_eq!(
                result_ty, u64_ty,
                "{:?} should return U64, got {:?}",
                op, result_ty
            );
        }
    });
}

// ---- Unchecked arithmetic ops return lhs type ----

#[test]
fn unchecked_arithmetic_ops_return_lhs_type() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let ops = [
            BinaryOp::AddUnchecked,
            BinaryOp::SubUnchecked,
            BinaryOp::MulUnchecked,
        ];
        for op in &ops {
            let result_ty = op.ty(&ctx, i32_ty, i32_ty);
            assert_eq!(
                result_ty, i32_ty,
                "{:?} should return I32, got {:?}",
                op, result_ty
            );
        }
    });
}

// ---- RValue construction tests ----

#[test]
fn rvalue_binary_op_rem_construction() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let lhs = Operand::Const(ConstOperand::Value(ConstValue::ZST, i32_ty));
        let rhs = Operand::Const(ConstOperand::Value(ConstValue::ZST, i32_ty));
        let rv = RValue::BinaryOp(BinaryOp::Rem, lhs, rhs);
        assert!(matches!(rv, RValue::BinaryOp(BinaryOp::Rem, _, _)));
    });
}

#[test]
fn rvalue_binary_op_bitwise_construction() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let lhs = Operand::Const(ConstOperand::Value(ConstValue::ZST, i32_ty));
        let rhs = Operand::Const(ConstOperand::Value(ConstValue::ZST, i32_ty));
        let ops = [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::BitXor];
        for op in ops {
            let rv = RValue::BinaryOp(op.clone(), lhs.clone(), rhs.clone());
            assert!(matches!(rv, RValue::BinaryOp(_, _, _)));
        }
    });
}

#[test]
fn rvalue_binary_op_shift_construction() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let lhs = Operand::Const(ConstOperand::Value(ConstValue::ZST, i32_ty));
        let rhs = Operand::Const(ConstOperand::Value(ConstValue::ZST, i32_ty));
        let rv_shl = RValue::BinaryOp(BinaryOp::Shl, lhs.clone(), rhs.clone());
        let rv_shr = RValue::BinaryOp(BinaryOp::Shr, lhs, rhs);
        assert!(matches!(rv_shl, RValue::BinaryOp(BinaryOp::Shl, _, _)));
        assert!(matches!(rv_shr, RValue::BinaryOp(BinaryOp::Shr, _, _)));
    });
}

#[test]
fn rvalue_unary_op_not_construction() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let operand = Operand::Const(ConstOperand::Value(ConstValue::ZST, i32_ty));
        let rv = RValue::UnaryOp(UnaryOp::Not, operand);
        assert!(matches!(rv, RValue::UnaryOp(UnaryOp::Not, _)));
    });
}

// ---- Comparison ops with unsigned types ----

#[test]
fn comparison_ops_unsigned_return_bool() {
    with_ctx(|ctx| {
        let u32_ty = ctx.intern_ty(ty::TirTy::U32);
        let bool_ty = ctx.intern_ty(ty::TirTy::Bool);
        let ops = [
            BinaryOp::Eq,
            BinaryOp::Ne,
            BinaryOp::Lt,
            BinaryOp::Le,
            BinaryOp::Gt,
            BinaryOp::Ge,
        ];
        for op in &ops {
            let result_ty = op.ty(&ctx, u32_ty, u32_ty);
            assert_eq!(
                result_ty, bool_ty,
                "{:?} on U32 should return Bool, got {:?}",
                op, result_ty
            );
        }
    });
}

// ---- Bitwise ops with different integer widths ----

#[test]
fn bitwise_ops_i64_return_lhs_type() {
    with_ctx(|ctx| {
        let i64_ty = ctx.intern_ty(ty::TirTy::I64);
        let ops = [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::BitXor];
        for op in &ops {
            let result_ty = op.ty(&ctx, i64_ty, i64_ty);
            assert_eq!(
                result_ty, i64_ty,
                "{:?} should return I64, got {:?}",
                op, result_ty
            );
        }
    });
}

// ---- BasicBlock tests ----

#[test]
fn basic_block_entry_is_zero() {
    assert_eq!(ENTRY_BLOCK, BasicBlock::new(0));
}

#[test]
fn basic_block_idx_trait() {
    let mut bb = BasicBlock::new(2);
    assert_eq!(bb.idx(), 2);
    bb.incr();
    assert_eq!(bb.idx(), 3);
    bb.incr_by(5);
    assert_eq!(bb.idx(), 8);
}

// ---- CastKind tests ----

#[test]
fn cast_kind_int_to_int_variant() {
    let kind = CastKind::IntToInt;
    assert!(matches!(kind, CastKind::IntToInt));
}

#[test]
fn cast_kind_float_to_float_variant() {
    let kind = CastKind::FloatToFloat;
    assert!(matches!(kind, CastKind::FloatToFloat));
}

#[test]
fn cast_kind_int_to_float_variant() {
    let kind = CastKind::IntToFloat;
    assert!(matches!(kind, CastKind::IntToFloat));
}

#[test]
fn cast_kind_float_to_int_variant() {
    let kind = CastKind::FloatToInt;
    assert!(matches!(kind, CastKind::FloatToInt));
}

#[test]
fn cast_kind_ptr_to_int_variant() {
    let kind = CastKind::PtrToInt;
    assert!(matches!(kind, CastKind::PtrToInt));
}

#[test]
fn cast_kind_int_to_ptr_variant() {
    let kind = CastKind::IntToPtr;
    assert!(matches!(kind, CastKind::IntToPtr));
}

#[test]
fn cast_kind_bitcast_variant() {
    let kind = CastKind::Bitcast;
    assert!(matches!(kind, CastKind::Bitcast));
}

#[test]
fn cast_kind_ptr_to_ptr_variant() {
    let kind = CastKind::PtrToPtr;
    assert!(matches!(kind, CastKind::PtrToPtr));
}

// ---- RValue::Cast construction tests ----

#[test]
fn rvalue_cast_int_to_int_construction() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let i64_ty = ctx.intern_ty(ty::TirTy::I64);
        let operand = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 42,
                size: std::num::NonZero::new(4).unwrap(),
            })),
            i32_ty,
        ));
        let rvalue = RValue::Cast(CastKind::IntToInt, operand, i64_ty);
        assert!(matches!(rvalue, RValue::Cast(CastKind::IntToInt, _, _)));
    });
}

#[test]
fn rvalue_cast_float_to_float_construction() {
    with_ctx(|ctx| {
        let f32_ty = ctx.intern_ty(ty::TirTy::F32);
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);
        let operand = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x40490FDB, // ~pi as f32 bits
                size: std::num::NonZero::new(4).unwrap(),
            })),
            f32_ty,
        ));
        let rvalue = RValue::Cast(CastKind::FloatToFloat, operand, f64_ty);
        assert!(matches!(rvalue, RValue::Cast(CastKind::FloatToFloat, _, _)));
    });
}

#[test]
fn rvalue_cast_int_to_float_construction() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);
        let operand = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 10,
                size: std::num::NonZero::new(4).unwrap(),
            })),
            i32_ty,
        ));
        let rvalue = RValue::Cast(CastKind::IntToFloat, operand, f64_ty);
        assert!(matches!(rvalue, RValue::Cast(CastKind::IntToFloat, _, _)));
    });
}

#[test]
fn rvalue_cast_float_to_int_construction() {
    with_ctx(|ctx| {
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let operand = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x4024000000000000, // 10.0 as f64 bits
                size: std::num::NonZero::new(8).unwrap(),
            })),
            f64_ty,
        ));
        let rvalue = RValue::Cast(CastKind::FloatToInt, operand, i32_ty);
        assert!(matches!(rvalue, RValue::Cast(CastKind::FloatToInt, _, _)));
    });
}

#[test]
fn rvalue_cast_int_to_ptr_construction() {
    with_ctx(|ctx| {
        let u64_ty = ctx.intern_ty(ty::TirTy::U64);
        let ptr_ty = ctx.intern_ty(ty::TirTy::RawPtr(u64_ty, ty::Mutability::Imm));
        let operand = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0xDEAD_BEEF,
                size: std::num::NonZero::new(8).unwrap(),
            })),
            u64_ty,
        ));
        let rvalue = RValue::Cast(CastKind::IntToPtr, operand, ptr_ty);
        assert!(matches!(rvalue, RValue::Cast(CastKind::IntToPtr, _, _)));
    });
}

#[test]
fn rvalue_cast_ptr_to_int_construction() {
    with_ctx(|ctx| {
        let u64_ty = ctx.intern_ty(ty::TirTy::U64);
        let ptr_ty = ctx.intern_ty(ty::TirTy::RawPtr(u64_ty, ty::Mutability::Imm));
        let operand = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0xCAFE_BABE,
                size: std::num::NonZero::new(8).unwrap(),
            })),
            ptr_ty,
        ));
        let rvalue = RValue::Cast(CastKind::PtrToInt, operand, u64_ty);
        assert!(matches!(rvalue, RValue::Cast(CastKind::PtrToInt, _, _)));
    });
}

#[test]
fn rvalue_cast_bitcast_construction() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let f32_ty = ctx.intern_ty(ty::TirTy::F32);
        let operand = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x42280000, // 42.0 as i32 bits
                size: std::num::NonZero::new(4).unwrap(),
            })),
            i32_ty,
        ));
        let rvalue = RValue::Cast(CastKind::Bitcast, operand, f32_ty);
        assert!(matches!(rvalue, RValue::Cast(CastKind::Bitcast, _, _)));
    });
}

#[test]
fn rvalue_cast_ptr_to_ptr_construction() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let i64_ty = ctx.intern_ty(ty::TirTy::I64);
        let ptr_i32 = ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Imm));
        let ptr_i64 = ctx.intern_ty(ty::TirTy::RawPtr(i64_ty, ty::Mutability::Mut));
        let operand = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x1000,
                size: std::num::NonZero::new(8).unwrap(),
            })),
            ptr_i32,
        ));
        let rvalue = RValue::Cast(CastKind::PtrToPtr, operand, ptr_i64);
        assert!(matches!(rvalue, RValue::Cast(CastKind::PtrToPtr, _, _)));
    });
}

// ---- TirTy helper method tests ----

#[test]
fn tir_ty_is_integer() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let u64_ty = ctx.intern_ty(ty::TirTy::U64);
        let f32_ty = ctx.intern_ty(ty::TirTy::F32);
        let bool_ty = ctx.intern_ty(ty::TirTy::Bool);
        let unit_ty = ctx.intern_ty(ty::TirTy::Unit);

        assert!(i32_ty.is_integer());
        assert!(u64_ty.is_integer());
        assert!(!f32_ty.is_integer());
        assert!(!bool_ty.is_integer());
        assert!(!unit_ty.is_integer());
    });
}

#[test]
fn tir_ty_is_pointer() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let ptr_ty = ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Imm));
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);

        assert!(ptr_ty.is_pointer());
        assert!(!i32_ty.is_pointer());
        assert!(!f64_ty.is_pointer());
    });
}
