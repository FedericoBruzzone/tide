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
