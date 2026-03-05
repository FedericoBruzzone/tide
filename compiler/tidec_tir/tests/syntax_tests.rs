use tidec_abi::target::{BackendKind, TirTarget};
use tidec_tir::ctx::{EmitKind, InternCtx, TirArena, TirArgs, TirCtx};
use tidec_tir::syntax::*;
use tidec_tir::ty;
use tidec_utils::idx::Idx;
use tidec_utils::interner::TypeList;

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

// ---- Composite type construction tests ----

#[test]
fn tir_ty_struct_is_struct() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);
        let fields = ctx.intern_type_list(&[i32_ty, f64_ty]);
        let struct_ty = ctx.intern_ty(ty::TirTy::Struct {
            fields,
            packed: false,
        });
        assert!(struct_ty.is_struct());
    });
}

#[test]
fn tir_ty_struct_is_sized() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let fields = ctx.intern_type_list(&[i32_ty]);
        let struct_ty = ctx.intern_ty(ty::TirTy::Struct {
            fields,
            packed: false,
        });
        assert!(struct_ty.is_sized());
    });
}

#[test]
fn tir_ty_struct_is_not_integer() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let fields = ctx.intern_type_list(&[i32_ty]);
        let struct_ty = ctx.intern_ty(ty::TirTy::Struct {
            fields,
            packed: false,
        });
        assert!(!struct_ty.is_integer());
        assert!(!struct_ty.is_floating_point());
        assert!(!struct_ty.is_bool());
        assert!(!struct_ty.is_unit());
        assert!(!struct_ty.is_pointer());
    });
}

#[test]
fn tir_ty_struct_packed_vs_unpacked_differ() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let fields = ctx.intern_type_list(&[i32_ty]);
        let packed = ctx.intern_ty(ty::TirTy::Struct {
            fields,
            packed: true,
        });
        let unpacked = ctx.intern_ty(ty::TirTy::Struct {
            fields,
            packed: false,
        });
        // Packed and unpacked structs with the same fields are different types.
        assert_ne!(packed, unpacked);
    });
}

#[test]
fn tir_ty_struct_same_fields_are_equal() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);
        let fields = ctx.intern_type_list(&[i32_ty, f64_ty]);
        let s1 = ctx.intern_ty(ty::TirTy::Struct {
            fields,
            packed: false,
        });
        let s2 = ctx.intern_ty(ty::TirTy::Struct {
            fields,
            packed: false,
        });
        assert_eq!(s1, s2);
    });
}

#[test]
fn tir_ty_struct_empty() {
    with_ctx(|ctx| {
        let fields = ctx.intern_type_list(&[]);
        let empty_struct = ctx.intern_ty(ty::TirTy::Struct {
            fields,
            packed: false,
        });
        assert!(empty_struct.is_struct());
        assert!(empty_struct.is_sized());
    });
}

#[test]
fn tir_ty_array_is_array() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let array_ty = ctx.intern_ty(ty::TirTy::Array(i32_ty, 10));
        assert!(array_ty.is_array());
    });
}

#[test]
fn tir_ty_array_is_sized() {
    with_ctx(|ctx| {
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);
        let array_ty = ctx.intern_ty(ty::TirTy::Array(f64_ty, 5));
        assert!(array_ty.is_sized());
    });
}

#[test]
fn tir_ty_array_is_not_integer() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let array_ty = ctx.intern_ty(ty::TirTy::Array(i32_ty, 3));
        assert!(!array_ty.is_integer());
        assert!(!array_ty.is_floating_point());
        assert!(!array_ty.is_bool());
        assert!(!array_ty.is_unit());
        assert!(!array_ty.is_pointer());
    });
}

#[test]
fn tir_ty_array_same_elem_same_count_equal() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let a1 = ctx.intern_ty(ty::TirTy::Array(i32_ty, 4));
        let a2 = ctx.intern_ty(ty::TirTy::Array(i32_ty, 4));
        assert_eq!(a1, a2);
    });
}

#[test]
fn tir_ty_array_different_count_differ() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let a1 = ctx.intern_ty(ty::TirTy::Array(i32_ty, 3));
        let a2 = ctx.intern_ty(ty::TirTy::Array(i32_ty, 5));
        assert_ne!(a1, a2);
    });
}

#[test]
fn tir_ty_array_different_elem_differ() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let f32_ty = ctx.intern_ty(ty::TirTy::F32);
        let a1 = ctx.intern_ty(ty::TirTy::Array(i32_ty, 3));
        let a2 = ctx.intern_ty(ty::TirTy::Array(f32_ty, 3));
        assert_ne!(a1, a2);
    });
}

#[test]
fn tir_ty_array_zero_length() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let empty_array = ctx.intern_ty(ty::TirTy::Array(i32_ty, 0));
        assert!(empty_array.is_array());
        assert!(empty_array.is_sized());
    });
}

#[test]
fn tir_ty_struct_not_equal_to_array() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let fields = ctx.intern_type_list(&[i32_ty]);
        let struct_ty = ctx.intern_ty(ty::TirTy::Struct {
            fields,
            packed: false,
        });
        let array_ty = ctx.intern_ty(ty::TirTy::Array(i32_ty, 1));
        assert_ne!(struct_ty, array_ty);
        assert!(struct_ty.is_struct());
        assert!(!struct_ty.is_array());
        assert!(array_ty.is_array());
        assert!(!array_ty.is_struct());
    });
}

// ---- RValue::Aggregate construction tests ----

#[test]
fn rvalue_aggregate_struct_construction() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);
        let fields = ctx.intern_type_list(&[i32_ty, f64_ty]);
        let struct_ty = ctx.intern_ty(ty::TirTy::Struct {
            fields,
            packed: false,
        });

        let op_i32 = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 42,
                size: std::num::NonZero::new(4).unwrap(),
            })),
            i32_ty,
        ));
        let op_f64 = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x4024000000000000, // 10.0
                size: std::num::NonZero::new(8).unwrap(),
            })),
            f64_ty,
        ));

        let rvalue = RValue::Aggregate(AggregateKind::Struct(struct_ty), vec![op_i32, op_f64]);
        assert!(matches!(
            rvalue,
            RValue::Aggregate(AggregateKind::Struct(_), _)
        ));
    });
}

#[test]
fn rvalue_aggregate_array_construction() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);

        let ops: Vec<Operand<'_>> = (0..3)
            .map(|i| {
                Operand::Const(ConstOperand::Value(
                    ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                        data: i as u128,
                        size: std::num::NonZero::new(4).unwrap(),
                    })),
                    i32_ty,
                ))
            })
            .collect();

        let rvalue = RValue::Aggregate(AggregateKind::Array(i32_ty), ops);
        assert!(matches!(
            rvalue,
            RValue::Aggregate(AggregateKind::Array(_), _)
        ));
    });
}

// ---- Place with composite projections ----

#[test]
fn place_with_field_projection_on_struct() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let place = Place {
            local: Local::new(1),
            projection: vec![Projection::Field(0, i32_ty)],
        };
        assert!(place.try_local().is_none());
        assert_eq!(place.projection.len(), 1);
        assert!(matches!(place.projection[0], Projection::Field(0, _)));
    });
}

#[test]
fn place_with_index_projection_on_array() {
    let place: Place<'_> = Place {
        local: Local::new(1),
        projection: vec![Projection::Index(Local::new(2))],
    };
    assert!(place.try_local().is_none());
    assert_eq!(place.projection.len(), 1);
    assert!(matches!(place.projection[0], Projection::Index(_)));
}

#[test]
fn place_with_field_then_index() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        // Access: _1.field0[_2]
        let place = Place {
            local: Local::new(1),
            projection: vec![
                Projection::Field(0, i32_ty),
                Projection::Index(Local::new(2)),
            ],
        };
        assert_eq!(place.projection.len(), 2);
        assert!(matches!(place.projection[0], Projection::Field(0, _)));
        assert!(matches!(place.projection[1], Projection::Index(_)));
    });
}

// ---- intern_type_list tests ----

#[test]
fn intern_type_list_empty() {
    with_ctx(|ctx| {
        let tl = ctx.intern_type_list(&[]);
        assert!(tl.is_empty());
        assert_eq!(tl.len(), 0);
    });
}

#[test]
fn intern_type_list_single_element() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let tl = ctx.intern_type_list(&[i32_ty]);
        assert_eq!(tl.len(), 1);
        assert_eq!(tl.as_slice()[0], i32_ty);
    });
}

#[test]
fn intern_type_list_multiple_elements() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);
        let bool_ty = ctx.intern_ty(ty::TirTy::Bool);
        let tl = ctx.intern_type_list(&[i32_ty, f64_ty, bool_ty]);
        assert_eq!(tl.len(), 3);
        let slice = tl.as_slice();
        assert_eq!(slice[0], i32_ty);
        assert_eq!(slice[1], f64_ty);
        assert_eq!(slice[2], bool_ty);
    });
}

// ---- RValue::AddressOf tests ----

#[test]
fn rvalue_address_of_variant() {
    with_ctx(|_ctx| {
        let rvalue: RValue<'_> = RValue::AddressOf(ty::Mutability::Mut, Place::from(Local::new(1)));
        assert!(matches!(rvalue, RValue::AddressOf(ty::Mutability::Mut, _)));
    });
}

#[test]
fn rvalue_address_of_immutable() {
    with_ctx(|_ctx| {
        let rvalue: RValue<'_> = RValue::AddressOf(ty::Mutability::Imm, Place::from(Local::new(2)));
        assert!(matches!(rvalue, RValue::AddressOf(ty::Mutability::Imm, _)));
    });
}

#[test]
fn rvalue_address_of_with_projection() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let place = Place {
            local: Local::new(1),
            projection: vec![Projection::Field(0, i32_ty)],
        };
        let rvalue: RValue<'_> = RValue::AddressOf(ty::Mutability::Mut, place);
        match rvalue {
            RValue::AddressOf(m, p) => {
                assert_eq!(m, ty::Mutability::Mut);
                assert!(p.try_local().is_none()); // has projection
                assert_eq!(p.projection.len(), 1);
            }
            _ => panic!("Expected AddressOf variant"),
        }
    });
}

#[test]
fn rvalue_address_of_array_element() {
    with_ctx(|_ctx| {
        // &arr[idx] → AddressOf(Imm, Place { local: arr, projection: [Index(idx)] })
        let place = Place {
            local: Local::new(1),
            projection: vec![Projection::Index(Local::new(2))],
        };
        let rvalue: RValue<'_> = RValue::AddressOf(ty::Mutability::Imm, place);
        match rvalue {
            RValue::AddressOf(m, p) => {
                assert_eq!(m, ty::Mutability::Imm);
                assert!(matches!(p.projection[0], Projection::Index(_)));
            }
            _ => panic!("Expected AddressOf variant"),
        }
    });
}

// ---- ConstValue::NullPtr tests ----

#[test]
fn const_value_null_ptr_variant() {
    let cv = ConstValue::NullPtr;
    assert_eq!(cv, ConstValue::NullPtr);
}

#[test]
fn const_value_null_ptr_ne_zst() {
    assert_ne!(ConstValue::NullPtr, ConstValue::ZST);
}

#[test]
fn const_operand_with_null_ptr() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let ptr_ty = ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Imm));
        let operand = ConstOperand::Value(ConstValue::NullPtr, ptr_ty);
        assert_eq!(operand.ty(), ptr_ty);
        assert_eq!(operand.value(), ConstValue::NullPtr);
    });
}

#[test]
fn operand_const_null_ptr() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let ptr_ty = ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Mut));
        let op = Operand::Const(ConstOperand::Value(ConstValue::NullPtr, ptr_ty));
        assert!(matches!(
            op,
            Operand::Const(ConstOperand::Value(ConstValue::NullPtr, _))
        ));
    });
}

#[test]
fn statement_assign_null_ptr_to_place() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let ptr_ty = ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Mut));
        let stmt = Statement::Assign(Box::new((
            Place::from(Local::new(0)),
            RValue::Operand(Operand::Const(ConstOperand::Value(
                ConstValue::NullPtr,
                ptr_ty,
            ))),
        )));
        assert!(matches!(stmt, Statement::Assign(_)));
    });
}

#[test]
fn statement_assign_address_of() {
    with_ctx(|_ctx| {
        let stmt = Statement::Assign(Box::new((
            Place::from(Local::new(0)),
            RValue::AddressOf(ty::Mutability::Mut, Place::from(Local::new(1))),
        )));
        match stmt {
            Statement::Assign(assig) => {
                assert!(matches!(assig.1, RValue::AddressOf(_, _)));
            }
        }
    });
}

// ---- Global variable & TirUnit tests ----

use std::num::NonZero;
use tidec_tir::body::{
    GlobalId, Linkage, TirGlobal, TirUnit, TirUnitMetadata, UnnamedAddress, Visibility,
};
use tidec_utils::index_vec::IdxVec;

#[test]
fn global_id_idx_trait() {
    let mut g = GlobalId::new(0);
    assert_eq!(g.idx(), 0);
    g.incr();
    assert_eq!(g.idx(), 1);
    g.incr_by(5);
    assert_eq!(g.idx(), 6);
}

#[test]
fn global_id_equality_and_hash() {
    use std::collections::HashSet;
    let a = GlobalId::new(3);
    let b = GlobalId::new(3);
    let c = GlobalId::new(4);
    assert_eq!(a, b);
    assert_ne!(a, c);
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
    assert!(!set.contains(&c));
}

#[test]
fn tir_global_scalar_initializer() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let global = TirGlobal {
            name: "my_global".to_string(),
            ty: i32_ty,
            initializer: Some(ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 42 as u128,
                size: NonZero::new(4).unwrap(),
            }))),
            mutable: true,
            linkage: Linkage::External,
            visibility: Visibility::Default,
            unnamed_address: UnnamedAddress::None,
        };
        assert_eq!(global.name, "my_global");
        assert_eq!(global.ty, i32_ty);
        assert!(global.mutable);
        assert!(global.initializer.is_some());
    });
}

#[test]
fn tir_global_no_initializer_declaration() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let global = TirGlobal {
            name: "extern_var".to_string(),
            ty: i32_ty,
            initializer: None,
            mutable: true,
            linkage: Linkage::External,
            visibility: Visibility::Default,
            unnamed_address: UnnamedAddress::None,
        };
        assert!(global.initializer.is_none());
    });
}

#[test]
fn tir_global_constant_immutable() {
    with_ctx(|ctx| {
        let i64_ty = ctx.intern_ty(ty::TirTy::I64);
        let global = TirGlobal {
            name: "MAGIC".to_string(),
            ty: i64_ty,
            initializer: Some(ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0xDEAD_BEEF_u128,
                size: NonZero::new(8).unwrap(),
            }))),
            mutable: false,
            linkage: Linkage::Private,
            visibility: Visibility::Default,
            unnamed_address: UnnamedAddress::Global,
        };
        assert!(!global.mutable);
        assert!(matches!(global.linkage, Linkage::Private));
        assert!(matches!(global.unnamed_address, UnnamedAddress::Global));
    });
}

#[test]
fn tir_global_null_ptr_initializer() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let ptr_ty = ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Mut));
        let global = TirGlobal {
            name: "null_ptr_global".to_string(),
            ty: ptr_ty,
            initializer: Some(ConstValue::NullPtr),
            mutable: false,
            linkage: Linkage::Internal,
            visibility: Visibility::Default,
            unnamed_address: UnnamedAddress::None,
        };
        assert!(matches!(global.initializer, Some(ConstValue::NullPtr)));
        assert!(matches!(global.linkage, Linkage::Internal));
    });
}

#[test]
fn tir_global_zst_initializer() {
    with_ctx(|ctx| {
        let unit_ty = ctx.intern_ty(ty::TirTy::Unit);
        let global = TirGlobal {
            name: "zst_global".to_string(),
            ty: unit_ty,
            initializer: Some(ConstValue::ZST),
            mutable: false,
            linkage: Linkage::External,
            visibility: Visibility::Default,
            unnamed_address: UnnamedAddress::None,
        };
        assert!(matches!(global.initializer, Some(ConstValue::ZST)));
    });
}

#[test]
fn tir_unit_with_globals() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let g1 = TirGlobal {
            name: "counter".to_string(),
            ty: i32_ty,
            initializer: Some(ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0 as u128,
                size: NonZero::new(4).unwrap(),
            }))),
            mutable: true,
            linkage: Linkage::External,
            visibility: Visibility::Default,
            unnamed_address: UnnamedAddress::None,
        };
        let g2 = TirGlobal {
            name: "LIMIT".to_string(),
            ty: i32_ty,
            initializer: Some(ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 100 as u128,
                size: NonZero::new(4).unwrap(),
            }))),
            mutable: false,
            linkage: Linkage::Private,
            visibility: Visibility::Default,
            unnamed_address: UnnamedAddress::None,
        };

        let unit = TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "globals_unit".to_string(),
            },
            globals: IdxVec::from_raw(vec![g1, g2]),
            bodies: IdxVec::new(),
        };

        assert_eq!(unit.globals.len(), 2);
        assert_eq!(unit.globals[GlobalId::new(0)].name, "counter");
        assert_eq!(unit.globals[GlobalId::new(1)].name, "LIMIT");
        assert!(unit.globals[GlobalId::new(0)].mutable);
        assert!(!unit.globals[GlobalId::new(1)].mutable);
        assert!(unit.bodies.is_empty());
    });
}

#[test]
fn tir_unit_empty_globals() {
    let unit: TirUnit<'_> = TirUnit {
        metadata: TirUnitMetadata {
            unit_name: "no_globals".to_string(),
        },
        globals: IdxVec::new(),
        bodies: IdxVec::new(),
    };
    assert!(unit.globals.is_empty());
}

#[test]
fn tir_global_all_linkage_variants() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let linkages = [
            Linkage::Private,
            Linkage::Internal,
            Linkage::External,
            Linkage::Weak,
            Linkage::WeakODR,
            Linkage::LinkOnce,
            Linkage::LinkOnceODR,
            Linkage::Common,
        ];
        for linkage in linkages {
            let global = TirGlobal {
                name: "linkage_test".to_string(),
                ty: i32_ty,
                initializer: Some(ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                    data: 0 as u128,
                    size: NonZero::new(4).unwrap(),
                }))),
                mutable: false,
                linkage,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::None,
            };
            // Just verify construction doesn't panic
            let _ = global.name;
        }
    });
}

#[test]
fn tir_global_indirect_initializer() {
    with_ctx(|ctx| {
        use tidec_abi::size_and_align::Size;
        use tidec_tir::alloc::Allocation;

        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let alloc = Allocation::from_bytes(&[1, 2, 3, 4]);
        let alloc_id = ctx.insert_alloc(tidec_tir::alloc::GlobalAlloc::Memory(
            ctx.intern_alloc(alloc),
        ));

        let global = TirGlobal {
            name: "byte_global".to_string(),
            ty: i32_ty,
            initializer: Some(ConstValue::Indirect {
                alloc_id,
                offset: Size::ZERO,
            }),
            mutable: false,
            linkage: Linkage::External,
            visibility: Visibility::Default,
            unnamed_address: UnnamedAddress::None,
        };

        match &global.initializer {
            Some(ConstValue::Indirect { alloc_id: aid, .. }) => {
                assert_eq!(*aid, alloc_id);
            }
            _ => panic!("Expected Indirect initializer"),
        }
    });
}

// ---- Statement::assign and Operand::use_local helpers ----

#[test]
fn statement_assign_creates_assign_variant() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let place = Place::from(Local::new(1));
        let rvalue = RValue::Operand(Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 42,
                size: NonZero::new(4).unwrap(),
            })),
            i32_ty,
        )));
        let stmt = Statement::assign(place, rvalue);
        match &stmt {
            Statement::Assign(inner) => {
                let (p, rv) = inner.as_ref();
                assert_eq!(p.local, Local::new(1));
                assert!(p.projection.is_empty());
                assert!(matches!(rv, RValue::Operand(_)));
            }
        }
    });
}

#[test]
fn statement_assign_preserves_place_projections() {
    with_ctx(|ctx| {
        let bool_ty = ctx.intern_ty(ty::TirTy::Bool);
        let place = Place {
            local: Local::new(2),
            projection: vec![Projection::Field(0, bool_ty)],
        };
        let rvalue = RValue::Operand(Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0,
                size: NonZero::new(1).unwrap(),
            })),
            bool_ty,
        )));
        let stmt = Statement::assign(place, rvalue);
        match &stmt {
            Statement::Assign(inner) => {
                let (p, _) = inner.as_ref();
                assert_eq!(p.local, Local::new(2));
                assert_eq!(p.projection.len(), 1);
                assert!(matches!(p.projection[0], Projection::Field(0, _)));
            }
        }
    });
}

#[test]
fn operand_use_local_produces_use_with_empty_projection() {
    let op = Operand::use_local(Local::new(5));
    match &op {
        Operand::Use(place) => {
            assert_eq!(place.local, Local::new(5));
            assert!(place.projection.is_empty());
        }
        _ => panic!("Expected Use operand"),
    }
}

#[test]
fn operand_use_local_return_local() {
    let op = Operand::use_local(RETURN_LOCAL);
    match &op {
        Operand::Use(place) => {
            assert_eq!(place.local, RETURN_LOCAL);
        }
        _ => panic!("Expected Use operand"),
    }
}
