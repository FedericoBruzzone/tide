//! Integration test: program that returns 255 (max exit code).

mod common;

use std::num::NonZero;

use common::{TestContext, TestRunner};
use tidec_tir::body::{
    CallConv, DefId, Linkage, TirBody, TirBodyKind, TirBodyMetadata, TirItemKind, TirUnit,
    TirUnitMetadata, UnnamedAddress, Visibility,
};
use tidec_tir::ctx::{InternCtx, TirCtx};
use tidec_tir::syntax::{
    BasicBlockData, ConstOperand, ConstScalar, ConstValue, LocalData, Operand, Place, RValue,
    RawScalarValue, Statement, Terminator, UnaryOp, RETURN_LOCAL,
};
use tidec_tir::ty::TirTy;
use tidec_utils::index_vec::IdxVec;

/// Create a simple main function that returns 255.
fn create_return_255<'a>(tir_ctx: &TirCtx<'a>) -> TirUnit<'a> {
    let i32_ty = tir_ctx.intern_ty(TirTy::<TirCtx>::I32);

    let main_metadata = TirBodyMetadata {
        def_id: DefId(0),
        name: "main".to_string(),
        kind: TirBodyKind::Item(TirItemKind::Function),
        inlined: false,
        linkage: Linkage::External,
        visibility: Visibility::Default,
        unnamed_address: UnnamedAddress::None,
        call_conv: CallConv::C,
        is_varargs: false,
        is_declaration: false,
    };

    let main_body = TirBody {
        metadata: main_metadata,
        ret_and_args: IdxVec::from_raw(vec![LocalData {
            ty: i32_ty,
            mutable: false,
        }]),
        locals: IdxVec::new(),
        basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place {
                    local: RETURN_LOCAL,
                    projection: vec![],
                },
                RValue::UnaryOp(
                    UnaryOp::Pos,
                    Operand::Const(ConstOperand::Value(
                        ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                            data: 255u128,
                            size: NonZero::new(4).unwrap(),
                        })),
                        i32_ty,
                    )),
                ),
            )))],
            terminator: Terminator::Return,
        }]),
    };

    TirUnit {
        metadata: TirUnitMetadata {
            unit_name: "main".to_string(),
        },
        bodies: IdxVec::from_raw(vec![main_body]),
    }
}

/// Test that a program returning 255 (max exit code) compiles and runs correctly.
#[test]
fn test_return_255() {
    let runner = TestRunner::new("return_255");

    let test_ctx = TestContext::new();
    let intern_ctx = InternCtx::new(&test_ctx.arena);
    let tir_ctx = TirCtx::new(&test_ctx.target, &test_ctx.arguments, &intern_ctx);

    let tir_unit = create_return_255(&tir_ctx);
    runner.compile(tir_ctx, tir_unit);

    runner.link().expect("Linking failed");
    assert_eq!(runner.run(), Some(255), "Expected exit code 255");
}
