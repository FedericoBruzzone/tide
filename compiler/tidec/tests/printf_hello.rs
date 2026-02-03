//! Integration test: program that calls printf and prints "Hello, Test!".

mod common;

use std::num::NonZero;

use common::{TestContext, TestRunner};
use tidec_abi::size_and_align::Size;
use tidec_tir::body::{
    CallConv, DefId, Linkage, TirBody, TirBodyKind, TirBodyMetadata, TirItemKind, TirUnit,
    TirUnitMetadata, UnnamedAddress, Visibility,
};
use tidec_tir::ctx::{InternCtx, TirCtx};
use tidec_tir::syntax::{
    BasicBlock, BasicBlockData, ConstOperand, ConstScalar, ConstValue, Local, LocalData, Operand,
    Place, RValue, RawScalarValue, Statement, Terminator, UnaryOp, RETURN_LOCAL,
};
use tidec_tir::ty::{Mutability, TirTy};
use tidec_utils::idx::Idx;
use tidec_utils::index_vec::IdxVec;

/// Create a program that calls printf and returns 0.
fn create_printf_hello<'a>(tir_ctx: &TirCtx<'a>) -> TirUnit<'a> {
    let i8_ty = tir_ctx.intern_ty(TirTy::<TirCtx>::I8);
    let ptr_i8_ty = tir_ctx.intern_ty(TirTy::RawPtr(i8_ty, Mutability::Imm));
    let i32_ty = tir_ctx.intern_ty(TirTy::<TirCtx>::I32);

    // Declare printf
    let printf_def_id = DefId(0);
    let printf_metadata = TirBodyMetadata {
        def_id: printf_def_id,
        name: "printf".to_string(),
        kind: TirBodyKind::Item(TirItemKind::Function),
        inlined: false,
        linkage: Linkage::External,
        visibility: Visibility::Default,
        unnamed_address: UnnamedAddress::None,
        call_conv: CallConv::C,
        is_varargs: true,
        is_declaration: true,
    };

    let printf_body = TirBody {
        metadata: printf_metadata,
        ret_and_args: IdxVec::from_raw(vec![
            LocalData {
                ty: i32_ty,
                mutable: false,
            },
            LocalData {
                ty: ptr_i8_ty,
                mutable: false,
            },
        ]),
        locals: IdxVec::new(),
        basic_blocks: IdxVec::new(),
    };

    // Register printf and format string
    let printf_alloc_id = tir_ctx.intern_fn(printf_def_id);
    let format_alloc_id = tir_ctx.intern_c_str("Hello, Test!\n");

    // Create main
    let main_metadata = TirBodyMetadata {
        def_id: DefId(1),
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

    let bb0 = BasicBlockData {
        statements: vec![],
        terminator: Terminator::Call {
            func: Operand::Const(ConstOperand::Value(
                ConstValue::Indirect {
                    alloc_id: printf_alloc_id,
                    offset: Size::ZERO,
                },
                ptr_i8_ty,
            )),
            args: vec![Operand::Const(ConstOperand::Value(
                ConstValue::Indirect {
                    alloc_id: format_alloc_id,
                    offset: Size::ZERO,
                },
                ptr_i8_ty,
            ))],
            destination: Place {
                local: Local::new(1),
                projection: vec![],
            },
            target: BasicBlock::new(1),
        },
    };

    let bb1 = BasicBlockData {
        statements: vec![Statement::Assign(Box::new((
            Place {
                local: RETURN_LOCAL,
                projection: vec![],
            },
            RValue::UnaryOp(
                UnaryOp::Pos,
                Operand::Const(ConstOperand::Value(
                    ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                        data: 0u128,
                        size: NonZero::new(4).unwrap(),
                    })),
                    i32_ty,
                )),
            ),
        )))],
        terminator: Terminator::Return,
    };

    let main_body = TirBody {
        metadata: main_metadata,
        ret_and_args: IdxVec::from_raw(vec![LocalData {
            ty: i32_ty,
            mutable: false,
        }]),
        locals: IdxVec::from_raw(vec![LocalData {
            ty: i32_ty,
            mutable: false,
        }]),
        basic_blocks: IdxVec::from_raw(vec![bb0, bb1]),
    };

    TirUnit {
        metadata: TirUnitMetadata {
            unit_name: "main".to_string(),
        },
        bodies: IdxVec::from_raw(vec![printf_body, main_body]),
    }
}

/// Test that a program calling printf compiles, runs, and produces expected output.
#[test]
fn test_printf_hello() {
    let runner = TestRunner::new("printf_hello");

    let test_ctx = TestContext::new();
    let intern_ctx = InternCtx::new(&test_ctx.arena);
    let tir_ctx = TirCtx::new(&test_ctx.target, &test_ctx.arguments, &intern_ctx);

    let tir_unit = create_printf_hello(&tir_ctx);
    runner.compile(tir_ctx, tir_unit);

    runner.link().expect("Linking failed");

    let (exit_code, stdout) = runner.run_with_output().expect("Failed to run executable");
    assert_eq!(exit_code, 0, "Expected exit code 0");
    assert_eq!(stdout, "Hello, Test!\n", "Unexpected output");
}
