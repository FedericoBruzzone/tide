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
