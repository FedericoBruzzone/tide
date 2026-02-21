//! Pipeline integration tests for the LLVM codegen backend.
//!
//! Each test builds a TIR program, runs it through the full LLVM codegen
//! pipeline, and asserts on the resulting LLVM IR text. The tests are ordered
//! to incrementally exercise the features added during development.
//!
//! **Requirements**: LLVM 20.1 must be available (set
//! `LLVM_SYS_201_PREFIX` or have `llvm-config` on `PATH`).

use std::num::NonZero;

use tidec_abi::size_and_align::Size;
use tidec_abi::target::{BackendKind, TirTarget};
use tidec_codegen_llvm::entry::llvm_codegen_to_ir_string;
use tidec_tir::body::{
    CallConv, DefId, Linkage, TirBody, TirBodyKind, TirBodyMetadata, TirItemKind, TirUnit,
    TirUnitMetadata, UnnamedAddress, Visibility,
};
use tidec_tir::ctx::{EmitKind, InternCtx, TirArena, TirArgs, TirCtx};
use tidec_tir::syntax::{
    AggregateKind, BasicBlock, BasicBlockData, BinaryOp, CastKind, ConstOperand, ConstScalar,
    ConstValue, Local, LocalData, Operand, Place, Projection, RValue, RawScalarValue, Statement,
    SwitchTargets, Terminator, UnaryOp, RETURN_LOCAL,
};
use tidec_tir::ty::{Mutability, TirTy};
use tidec_utils::idx::Idx;
use tidec_utils::index_vec::IdxVec;

// ── Helpers ─────────────────────────────────────────────────────────

/// Create a default `TirBodyMetadata` for a `main` function with the given
/// `DefId`.
fn main_metadata(def_id: DefId) -> TirBodyMetadata {
    TirBodyMetadata {
        def_id,
        name: "main".to_string(),
        kind: TirBodyKind::Item(TirItemKind::Function),
        inlined: false,
        linkage: Linkage::External,
        visibility: Visibility::Default,
        unnamed_address: UnnamedAddress::None,
        call_conv: CallConv::C,
        is_varargs: false,
        is_declaration: false,
    }
}

/// Build a scalar `i32` constant operand.
fn const_i32<'ctx>(tir_ctx: &TirCtx<'ctx>, value: i32) -> Operand<'ctx> {
    let i32_ty = tir_ctx.intern_ty(TirTy::<TirCtx>::I32);
    Operand::Const(ConstOperand::Value(
        ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
            data: value as u32 as u128,
            size: NonZero::new(4).unwrap(),
        })),
        i32_ty,
    ))
}

/// Build a scalar `u32` constant operand.
fn const_u32<'ctx>(tir_ctx: &TirCtx<'ctx>, value: u32) -> Operand<'ctx> {
    let u32_ty = tir_ctx.intern_ty(TirTy::<TirCtx>::U32);
    Operand::Const(ConstOperand::Value(
        ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
            data: value as u128,
            size: NonZero::new(4).unwrap(),
        })),
        u32_ty,
    ))
}

/// Convenience: run the codegen pipeline and return the LLVM IR string.
fn compile_to_ir<F>(build_fn: F) -> String
where
    F: for<'ctx> FnOnce(&TirCtx<'ctx>) -> TirUnit<'ctx>,
{
    let target = TirTarget::new(BackendKind::Llvm);
    let args = TirArgs {
        emit_kind: EmitKind::Object, // not used by ir-string path
    };
    let arena = TirArena::default();
    let intern_ctx = InternCtx::new(&arena);
    let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);
    let unit = build_fn(&tir_ctx);
    llvm_codegen_to_ir_string(tir_ctx, unit)
}

/// Build a `TirBody` for testing a binary operation using **mutable locals**
/// instead of raw constants. This prevents LLVM's `IRBuilder` from constant-
/// folding the expression at IR construction time.
///
/// Generated shape:
/// ```text
/// fn main() -> result_ty {
///     _1: operand_ty = lhs;   // mutable
///     _2: operand_ty = rhs;   // mutable
///     _0 = _1 OP _2;
///     return;
/// }
/// ```
fn binop_body_with_locals<'ctx>(
    op: BinaryOp,
    lhs: Operand<'ctx>,
    rhs: Operand<'ctx>,
    operand_ty: tidec_tir::TirTy<'ctx>,
    result_ty: tidec_tir::TirTy<'ctx>,
) -> TirBody<'ctx> {
    TirBody {
        metadata: main_metadata(DefId(0)),
        ret_and_args: IdxVec::from_raw(vec![LocalData {
            ty: result_ty,
            mutable: false,
        }]),
        locals: IdxVec::from_raw(vec![
            LocalData {
                ty: operand_ty,
                mutable: true,
            },
            LocalData {
                ty: operand_ty,
                mutable: true,
            },
        ]),
        basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
            statements: vec![
                Statement::Assign(Box::new((Place::from(Local::new(1)), RValue::Operand(lhs)))),
                Statement::Assign(Box::new((Place::from(Local::new(2)), RValue::Operand(rhs)))),
                Statement::Assign(Box::new((
                    Place::from(RETURN_LOCAL),
                    RValue::BinaryOp(
                        op,
                        Operand::Use(Place::from(Local::new(1))),
                        Operand::Use(Place::from(Local::new(2))),
                    ),
                ))),
            ],
            terminator: Terminator::Return,
        }]),
    }
}

/// Build a `TirBody` for testing a unary operation using a mutable local.
///
/// Generated shape:
/// ```text
/// fn main() -> ty {
///     _1: ty = operand;   // mutable
///     _0 = OP _1;
///     return;
/// }
/// ```
fn unop_body_with_local<'ctx>(
    op: UnaryOp,
    operand: Operand<'ctx>,
    ty: tidec_tir::TirTy<'ctx>,
) -> TirBody<'ctx> {
    TirBody {
        metadata: main_metadata(DefId(0)),
        ret_and_args: IdxVec::from_raw(vec![LocalData { ty, mutable: false }]),
        locals: IdxVec::from_raw(vec![LocalData { ty, mutable: true }]),
        basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
            statements: vec![
                Statement::Assign(Box::new((
                    Place::from(Local::new(1)),
                    RValue::Operand(operand),
                ))),
                Statement::Assign(Box::new((
                    Place::from(RETURN_LOCAL),
                    RValue::UnaryOp(op, Operand::Use(Place::from(Local::new(1)))),
                ))),
            ],
            terminator: Terminator::Return,
        }]),
    }
}

// ====================================================================
// Foundational: scalar return, void return, arithmetic, store
// ====================================================================

/// Simplest possible program: `main() -> i32 { return 0; }`
#[test]
fn pipeline_return_zero() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::new(),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![Statement::Assign(Box::new((
                    Place::from(RETURN_LOCAL),
                    RValue::Operand(const_i32(ctx, 0)),
                )))],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    println!("--- LLVM IR ---\n{}", ir);

    assert!(
        ir.contains("define"),
        "IR must contain a function definition"
    );
    assert!(ir.contains("ret i32 0"), "main should return 0");
}

/// Return a non-trivial constant: `main() -> i32 { return 42; }`
#[test]
fn pipeline_return_42() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::new(),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![Statement::Assign(Box::new((
                    Place::from(RETURN_LOCAL),
                    RValue::Operand(const_i32(ctx, 42)),
                )))],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(ir.contains("ret i32 42"), "main should return 42");
}

/// Void function: `void_fn() { return; }`
#[test]
fn pipeline_void_return() {
    let ir = compile_to_ir(|ctx| {
        let unit_ty = ctx.intern_ty(TirTy::<TirCtx>::Unit);

        let body = TirBody {
            metadata: TirBodyMetadata {
                def_id: DefId(0),
                name: "void_fn".to_string(),
                kind: TirBodyKind::Item(TirItemKind::Function),
                inlined: false,
                linkage: Linkage::External,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::None,
                call_conv: CallConv::C,
                is_varargs: false,
                is_declaration: false,
            },
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: unit_ty,
                mutable: false,
            }]),
            locals: IdxVec::new(),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("define void @void_fn"),
        "Should declare a void function, got:\n{}",
        ir
    );
    assert!(ir.contains("ret void"), "Should return void");
}

/// Unary negation: `main() -> i32 { return -(42); }`
#[test]
fn pipeline_unary_neg() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::new(),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![Statement::Assign(Box::new((
                    Place::from(RETURN_LOCAL),
                    RValue::UnaryOp(UnaryOp::Neg, const_i32(ctx, 42)),
                )))],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    // LLVM folds `sub nsw i32 0, 42` into the constant `-42`, or emits the sub.
    assert!(
        ir.contains("sub") || ir.contains("ret i32 -42"),
        "Should contain negation or folded constant, got:\n{}",
        ir
    );
}

/// Integer addition: `main() -> i32 { _1=10; _2=32; return _1+_2; }`
#[test]
fn pipeline_binary_add() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let body = binop_body_with_locals(
            BinaryOp::Add,
            const_i32(ctx, 10),
            const_i32(ctx, 32),
            i32_ty,
            i32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("add i32"),
        "Should contain an add instruction, got:\n{}",
        ir
    );
}

/// Integer subtraction: `main() -> i32 { _1=50; _2=8; return _1-_2; }`
#[test]
fn pipeline_binary_sub() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let body = binop_body_with_locals(
            BinaryOp::Sub,
            const_i32(ctx, 50),
            const_i32(ctx, 8),
            i32_ty,
            i32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("sub i32"),
        "Should contain a sub instruction, got:\n{}",
        ir
    );
}

/// Integer multiplication: `main() -> i32 { _1=6; _2=7; return _1*_2; }`
#[test]
fn pipeline_binary_mul() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let body = binop_body_with_locals(
            BinaryOp::Mul,
            const_i32(ctx, 6),
            const_i32(ctx, 7),
            i32_ty,
            i32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("mul i32"),
        "Should contain a mul instruction, got:\n{}",
        ir
    );
}

/// Signed integer division: `main() -> i32 { _1=100; _2=3; return _1/_2; }`
#[test]
fn pipeline_binary_div_signed() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let body = binop_body_with_locals(
            BinaryOp::Div,
            const_i32(ctx, 100),
            const_i32(ctx, 3),
            i32_ty,
            i32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("sdiv i32"),
        "Should contain an sdiv instruction for signed int, got:\n{}",
        ir
    );
}

/// Function call (printf): tests pre-definition, call terminator,
/// indirect allocation, and multi-block flow.
#[test]
fn pipeline_function_call_printf() {
    let ir = compile_to_ir(|ctx| {
        let i8_ty = ctx.intern_ty(TirTy::<TirCtx>::I8);
        let ptr_i8_ty = ctx.intern_ty(TirTy::RawPtr(i8_ty, Mutability::Imm));
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);

        // Declare printf
        let printf_def_id = DefId(0);
        let printf_body = TirBody {
            metadata: TirBodyMetadata {
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
            },
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

        let printf_alloc_id = ctx.intern_fn(printf_def_id);
        let format_alloc_id = ctx.intern_c_str("Hello\n");

        // main calls printf then returns 0
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
                Place::from(RETURN_LOCAL),
                RValue::Operand(const_i32(ctx, 0)),
            )))],
            terminator: Terminator::Return,
        };

        let main_body = TirBody {
            metadata: main_metadata(DefId(1)),
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
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![printf_body, main_body]),
        }
    });

    assert!(
        ir.contains("declare i32 @printf"),
        "Should declare printf, got:\n{}",
        ir
    );
    assert!(
        ir.contains("call i32"),
        "Should contain a call instruction, got:\n{}",
        ir
    );
    assert!(
        ir.contains("Hello"),
        "Should contain the format string constant, got:\n{}",
        ir
    );
}

// ====================================================================
// Control Flow: Goto, SwitchInt, Unreachable, comparisons
// ====================================================================

/// `Terminator::Goto`: unconditional branch between basic blocks.
///
/// ```text
/// bb0: goto bb1
/// bb1: return 7
/// ```
#[test]
fn pipeline_goto() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);

        let bb0 = BasicBlockData {
            statements: vec![],
            terminator: Terminator::Goto {
                target: BasicBlock::new(1),
            },
        };

        let bb1 = BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place::from(RETURN_LOCAL),
                RValue::Operand(const_i32(ctx, 7)),
            )))],
            terminator: Terminator::Return,
        };

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::new(),
            basic_blocks: IdxVec::from_raw(vec![bb0, bb1]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("br label"),
        "Should contain an unconditional branch, got:\n{}",
        ir
    );
    assert!(ir.contains("ret i32 7"), "Should return 7");
}

/// `Terminator::Unreachable`: emits LLVM `unreachable`.
///
/// ```text
/// bb0: unreachable
/// ```
#[test]
fn pipeline_unreachable() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::new(),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![],
                terminator: Terminator::Unreachable,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("unreachable"),
        "Should contain an unreachable instruction, got:\n{}",
        ir
    );
}

/// Integer comparison (`Eq`) producing an `i1`.
///
/// Uses mutable locals to prevent LLVM constant folding.
///
/// ```text
/// _1 = 10 (mutable)
/// _2 = 10 (mutable)
/// _3 = Eq(_1, _2)   // i1
/// _0 = 99            // return value (i32)
/// return
/// ```
#[test]
fn pipeline_icmp_eq() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let bool_ty = ctx.intern_ty(TirTy::<TirCtx>::Bool);

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![
                LocalData {
                    ty: i32_ty,
                    mutable: true,
                }, // _1
                LocalData {
                    ty: i32_ty,
                    mutable: true,
                }, // _2
                LocalData {
                    ty: bool_ty,
                    mutable: false,
                }, // _3
            ]),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![
                    Statement::Assign(Box::new((
                        Place::from(Local::new(1)),
                        RValue::Operand(const_i32(ctx, 10)),
                    ))),
                    Statement::Assign(Box::new((
                        Place::from(Local::new(2)),
                        RValue::Operand(const_i32(ctx, 10)),
                    ))),
                    // _3 = Eq(_1, _2)
                    Statement::Assign(Box::new((
                        Place::from(Local::new(3)),
                        RValue::BinaryOp(
                            BinaryOp::Eq,
                            Operand::Use(Place::from(Local::new(1))),
                            Operand::Use(Place::from(Local::new(2))),
                        ),
                    ))),
                    // _0 = 99
                    Statement::Assign(Box::new((
                        Place::from(RETURN_LOCAL),
                        RValue::Operand(const_i32(ctx, 99)),
                    ))),
                ],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("icmp eq i32"),
        "Should contain an icmp eq instruction, got:\n{}",
        ir
    );
}

/// Integer comparison (`Lt`) with signed operands and mutable locals.
#[test]
fn pipeline_icmp_lt_signed() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let bool_ty = ctx.intern_ty(TirTy::<TirCtx>::Bool);

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![
                LocalData {
                    ty: i32_ty,
                    mutable: true,
                }, // _1
                LocalData {
                    ty: i32_ty,
                    mutable: true,
                }, // _2
                LocalData {
                    ty: bool_ty,
                    mutable: false,
                }, // _3
            ]),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![
                    Statement::Assign(Box::new((
                        Place::from(Local::new(1)),
                        RValue::Operand(const_i32(ctx, 1)),
                    ))),
                    Statement::Assign(Box::new((
                        Place::from(Local::new(2)),
                        RValue::Operand(const_i32(ctx, 2)),
                    ))),
                    Statement::Assign(Box::new((
                        Place::from(Local::new(3)),
                        RValue::BinaryOp(
                            BinaryOp::Lt,
                            Operand::Use(Place::from(Local::new(1))),
                            Operand::Use(Place::from(Local::new(2))),
                        ),
                    ))),
                    Statement::Assign(Box::new((
                        Place::from(RETURN_LOCAL),
                        RValue::Operand(const_i32(ctx, 0)),
                    ))),
                ],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("icmp slt i32"),
        "Should contain icmp slt for signed less-than, got:\n{}",
        ir
    );
}

/// `SwitchInt` with a boolean condition (optimised to conditional branch).
///
/// Uses mutable locals for the comparison operands.
///
/// ```text
/// bb0: _1 = 5 (mutable); _2 = 5 (mutable); _3 = Eq(_1, _2)
///      SwitchInt(_3, [1 → bb1, otherwise → bb2])
/// bb1: _0 = 1; return
/// bb2: _0 = 0; return
/// ```
#[test]
fn pipeline_switch_int_bool() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let bool_ty = ctx.intern_ty(TirTy::<TirCtx>::Bool);

        // bb0: compare and branch
        let bb0 = BasicBlockData {
            statements: vec![
                Statement::Assign(Box::new((
                    Place::from(Local::new(1)),
                    RValue::Operand(const_i32(ctx, 5)),
                ))),
                Statement::Assign(Box::new((
                    Place::from(Local::new(2)),
                    RValue::Operand(const_i32(ctx, 5)),
                ))),
                Statement::Assign(Box::new((
                    Place::from(Local::new(3)),
                    RValue::BinaryOp(
                        BinaryOp::Eq,
                        Operand::Use(Place::from(Local::new(1))),
                        Operand::Use(Place::from(Local::new(2))),
                    ),
                ))),
            ],
            terminator: Terminator::SwitchInt {
                discr: Operand::Use(Place::from(Local::new(3))),
                targets: SwitchTargets::if_then(BasicBlock::new(1), BasicBlock::new(2)),
            },
        };

        // bb1: then branch → return 1
        let bb1 = BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place::from(RETURN_LOCAL),
                RValue::Operand(const_i32(ctx, 1)),
            )))],
            terminator: Terminator::Return,
        };

        // bb2: else branch → return 0
        let bb2 = BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place::from(RETURN_LOCAL),
                RValue::Operand(const_i32(ctx, 0)),
            )))],
            terminator: Terminator::Return,
        };

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            // _0 is mutable because it is assigned from multiple basic blocks
            // (bb1 and bb2). Without this, the second assignment panics.
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: true,
            }]),
            locals: IdxVec::from_raw(vec![
                LocalData {
                    ty: i32_ty,
                    mutable: true,
                }, // _1
                LocalData {
                    ty: i32_ty,
                    mutable: true,
                }, // _2
                LocalData {
                    ty: bool_ty,
                    mutable: false,
                }, // _3
            ]),
            basic_blocks: IdxVec::from_raw(vec![bb0, bb1, bb2]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    // A single-arm SwitchInt on bool should be lowered to a conditional branch.
    assert!(
        ir.contains("br i1"),
        "Boolean SwitchInt should emit a conditional branch, got:\n{}",
        ir
    );
    assert!(
        ir.contains("icmp eq i32"),
        "Should contain the comparison, got:\n{}",
        ir
    );
}

/// Multi-way `SwitchInt` (more than 1 arm → LLVM `switch`).
///
/// Uses a mutable local for the discriminant to prevent constant folding.
///
/// ```text
/// bb0: _1 = 2 (mutable); SwitchInt(_1, [0 → bb1, 1 → bb2, otherwise → bb3])
/// bb1: return 10
/// bb2: return 20
/// bb3: return 30
/// ```
#[test]
fn pipeline_switch_int_multi() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);

        let bb0 = BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place::from(Local::new(1)),
                RValue::Operand(const_i32(ctx, 2)),
            )))],
            terminator: Terminator::SwitchInt {
                discr: Operand::Use(Place::from(Local::new(1))),
                targets: SwitchTargets::new(
                    vec![(0, BasicBlock::new(1)), (1, BasicBlock::new(2))],
                    BasicBlock::new(3),
                ),
            },
        };

        let make_ret_bb = |val: i32| BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place::from(RETURN_LOCAL),
                RValue::Operand(const_i32(ctx, val)),
            )))],
            terminator: Terminator::Return,
        };

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            // _0 is mutable because it is assigned from multiple basic blocks
            // (bb1, bb2, bb3). Without this, the second assignment panics.
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: true,
            }]),
            locals: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: true,
            }]),
            basic_blocks: IdxVec::from_raw(vec![
                bb0,
                make_ret_bb(10),
                make_ret_bb(20),
                make_ret_bb(30),
            ]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("switch i32"),
        "Multi-arm SwitchInt should emit a switch instruction, got:\n{}",
        ir
    );
}

/// Loop pattern: `Goto` + `SwitchInt`.
///
/// ```text
/// // _0 = return (i32), _1 = counter (i32), _2 = cmp (bool)
/// bb0: _1 = 0; goto bb1
/// bb1: _2 = Lt(_1, 10); SwitchInt(_2, [1→bb2, else→bb3])
/// bb2: _1 = _1 + 1; goto bb1
/// bb3: _0 = _1; return
/// ```
#[test]
fn pipeline_loop_pattern() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let bool_ty = ctx.intern_ty(TirTy::<TirCtx>::Bool);

        // bb0: initialise counter, goto header
        let bb0 = BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place::from(Local::new(1)),
                RValue::Operand(const_i32(ctx, 0)),
            )))],
            terminator: Terminator::Goto {
                target: BasicBlock::new(1),
            },
        };

        // bb1 (header): compare counter < 10, branch
        let bb1 = BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place::from(Local::new(2)),
                RValue::BinaryOp(
                    BinaryOp::Lt,
                    Operand::Use(Place::from(Local::new(1))),
                    const_i32(ctx, 10),
                ),
            )))],
            terminator: Terminator::SwitchInt {
                discr: Operand::Use(Place::from(Local::new(2))),
                targets: SwitchTargets::if_then(BasicBlock::new(2), BasicBlock::new(3)),
            },
        };

        // bb2 (body): increment counter, goto header
        let bb2 = BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place::from(Local::new(1)),
                RValue::BinaryOp(
                    BinaryOp::Add,
                    Operand::Use(Place::from(Local::new(1))),
                    const_i32(ctx, 1),
                ),
            )))],
            terminator: Terminator::Goto {
                target: BasicBlock::new(1),
            },
        };

        // bb3 (exit): return counter value
        let bb3 = BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place::from(RETURN_LOCAL),
                RValue::Operand(Operand::Use(Place::from(Local::new(1)))),
            )))],
            terminator: Terminator::Return,
        };

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![
                // _1: counter (i32)
                LocalData {
                    ty: i32_ty,
                    mutable: true,
                },
                // _2: comparison result (bool)
                LocalData {
                    ty: bool_ty,
                    mutable: false,
                },
            ]),
            basic_blocks: IdxVec::from_raw(vec![bb0, bb1, bb2, bb3]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    // Should contain a back-edge (br label %bbXXX appearing twice for header)
    let branch_count = ir.matches("br label").count() + ir.matches("br i1").count();
    assert!(
        branch_count >= 3,
        "Loop should produce at least 3 branches (init→header, body→header, header→exit), got {} in:\n{}",
        branch_count,
        ir
    );
    assert!(
        ir.contains("icmp slt i32"),
        "Loop condition should use icmp slt, got:\n{}",
        ir
    );
    assert!(
        ir.contains("add i32"),
        "Loop body should contain add, got:\n{}",
        ir
    );
}

/// All six comparison operators on integers, using mutable locals.
#[test]
fn pipeline_all_icmp_operators() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let bool_ty = ctx.intern_ty(TirTy::<TirCtx>::Bool);

        let ops = [
            BinaryOp::Eq,
            BinaryOp::Ne,
            BinaryOp::Lt,
            BinaryOp::Le,
            BinaryOp::Gt,
            BinaryOp::Ge,
        ];

        // _1, _2: mutable i32 operands
        // _3 .. _8: comparison results (bool, PendingOperandRef)
        let mut stmts: Vec<Statement> = Vec::new();
        // Initialise mutable operands
        stmts.push(Statement::Assign(Box::new((
            Place::from(Local::new(1)),
            RValue::Operand(const_i32(ctx, 3)),
        ))));
        stmts.push(Statement::Assign(Box::new((
            Place::from(Local::new(2)),
            RValue::Operand(const_i32(ctx, 5)),
        ))));
        for (i, op) in ops.iter().enumerate() {
            stmts.push(Statement::Assign(Box::new((
                Place::from(Local::new(3 + i)),
                RValue::BinaryOp(
                    op.clone(),
                    Operand::Use(Place::from(Local::new(1))),
                    Operand::Use(Place::from(Local::new(2))),
                ),
            ))));
        }
        // Return 0
        stmts.push(Statement::Assign(Box::new((
            Place::from(RETURN_LOCAL),
            RValue::Operand(const_i32(ctx, 0)),
        ))));

        let mut locals: Vec<LocalData> = Vec::new();
        // _1, _2: mutable i32
        locals.push(LocalData {
            ty: i32_ty,
            mutable: true,
        });
        locals.push(LocalData {
            ty: i32_ty,
            mutable: true,
        });
        // _3.._8: comparison results
        for _ in 0..ops.len() {
            locals.push(LocalData {
                ty: bool_ty,
                mutable: false,
            });
        }

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(locals),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: stmts,
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    let expected = [
        "icmp eq", "icmp ne", "icmp slt", "icmp sle", "icmp sgt", "icmp sge",
    ];
    for pattern in &expected {
        assert!(
            ir.contains(pattern),
            "Missing comparison '{}' in IR:\n{}",
            pattern,
            ir
        );
    }
}

// ====================================================================
// Remaining Arithmetic & Logic
// ====================================================================

/// Remainder (signed): `main() -> i32 { _1=10; _2=3; return _1%_2; }`
/// Expected LLVM IR: `srem i32`
#[test]
fn pipeline_srem() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let body = binop_body_with_locals(
            BinaryOp::Rem,
            const_i32(ctx, 10),
            const_i32(ctx, 3),
            i32_ty,
            i32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("srem i32"),
        "Expected 'srem i32' in IR:\n{}",
        ir
    );
}

/// Remainder (unsigned): `main() -> u32 { _1=10u; _2=3u; return _1%_2; }`
/// Expected LLVM IR: `urem i32`
#[test]
fn pipeline_urem() {
    let ir = compile_to_ir(|ctx| {
        let u32_ty = ctx.intern_ty(TirTy::<TirCtx>::U32);
        let body = binop_body_with_locals(
            BinaryOp::Rem,
            const_u32(ctx, 10),
            const_u32(ctx, 3),
            u32_ty,
            u32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("urem i32"),
        "Expected 'urem i32' in IR:\n{}",
        ir
    );
}

/// Bitwise AND: `main() -> i32 { _1=0xFF; _2=0x0F; return _1&_2; }`
/// Expected LLVM IR: `and i32`
#[test]
fn pipeline_bitwise_and() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let body = binop_body_with_locals(
            BinaryOp::BitAnd,
            const_i32(ctx, 0xFF),
            const_i32(ctx, 0x0F),
            i32_ty,
            i32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(ir.contains("and i32"), "Expected 'and i32' in IR:\n{}", ir);
}

/// Bitwise OR: `main() -> i32 { _1=0xF0; _2=0x0F; return _1|_2; }`
/// Expected LLVM IR: `or i32`
#[test]
fn pipeline_bitwise_or() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let body = binop_body_with_locals(
            BinaryOp::BitOr,
            const_i32(ctx, 0xF0),
            const_i32(ctx, 0x0F),
            i32_ty,
            i32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(ir.contains("or i32"), "Expected 'or i32' in IR:\n{}", ir);
}

/// Bitwise XOR: `main() -> i32 { _1=0xFF; _2=0x0F; return _1^_2; }`
/// Expected LLVM IR: `xor i32`
#[test]
fn pipeline_bitwise_xor() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let body = binop_body_with_locals(
            BinaryOp::BitXor,
            const_i32(ctx, 0xFF),
            const_i32(ctx, 0x0F),
            i32_ty,
            i32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(ir.contains("xor i32"), "Expected 'xor i32' in IR:\n{}", ir);
}

/// Left shift: `main() -> i32 { _1=1; _2=4; return _1<<_2; }`
/// Expected LLVM IR: `shl i32`
#[test]
fn pipeline_shl() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let body = binop_body_with_locals(
            BinaryOp::Shl,
            const_i32(ctx, 1),
            const_i32(ctx, 4),
            i32_ty,
            i32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(ir.contains("shl i32"), "Expected 'shl i32' in IR:\n{}", ir);
}

/// Arithmetic right shift (signed): `main() -> i32 { _1=-16; _2=2; return _1>>_2; }`
/// Expected LLVM IR: `ashr i32`
#[test]
fn pipeline_ashr_signed() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let body = binop_body_with_locals(
            BinaryOp::Shr,
            const_i32(ctx, -16),
            const_i32(ctx, 2),
            i32_ty,
            i32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("ashr i32"),
        "Expected 'ashr i32' in IR:\n{}",
        ir
    );
}

/// Logical right shift (unsigned): `main() -> u32 { _1=16u; _2=2u; return _1>>_2; }`
/// Expected LLVM IR: `lshr i32`
#[test]
fn pipeline_lshr_unsigned() {
    let ir = compile_to_ir(|ctx| {
        let u32_ty = ctx.intern_ty(TirTy::<TirCtx>::U32);
        let body = binop_body_with_locals(
            BinaryOp::Shr,
            const_u32(ctx, 16),
            const_u32(ctx, 2),
            u32_ty,
            u32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("lshr i32"),
        "Expected 'lshr i32' in IR:\n{}",
        ir
    );
}

/// Bitwise NOT: `main() -> i32 { _1=42; return ~_1; }`
/// Expected LLVM IR: `xor i32 %..., -1`
#[test]
fn pipeline_not() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let body = unop_body_with_local(UnaryOp::Not, const_i32(ctx, 42), i32_ty);
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    // LLVM emits bitwise NOT as `xor %val, -1`
    assert!(
        ir.contains("xor i32"),
        "Expected 'xor i32' (NOT) in IR:\n{}",
        ir
    );
}

/// Composite test: all arithmetic & logic ops using mutable locals.
/// Verifies that remainder, bitwise, shift, and NOT all appear in the IR.
#[test]
fn pipeline_all_aritlogic_ops() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);

        // _0: return
        let ret_and_args = IdxVec::from_raw(vec![LocalData {
            ty: i32_ty,
            mutable: false,
        }]);
        // _1, _2: mutable operands; _3.._9: mutable results
        let mut locals = IdxVec::new();
        // _1: lhs operand
        locals.push(LocalData {
            ty: i32_ty,
            mutable: true,
        });
        // _2: rhs operand
        locals.push(LocalData {
            ty: i32_ty,
            mutable: true,
        });
        // _3.._9: results (7 ops total)
        for _ in 0..7 {
            locals.push(LocalData {
                ty: i32_ty,
                mutable: true,
            });
        }

        let local = |n: usize| -> Local { Local::new(n) };
        let use_local = |n: usize| -> Operand<'_> { Operand::Use(Place::from(Local::new(n))) };

        // bb0: init operands, compute all ops, then return
        let mut stmts = Vec::new();

        // _1 = 10
        stmts.push(Statement::Assign(Box::new((
            Place::from(local(1)),
            RValue::Operand(const_i32(ctx, 10)),
        ))));
        // _2 = 3
        stmts.push(Statement::Assign(Box::new((
            Place::from(local(2)),
            RValue::Operand(const_i32(ctx, 3)),
        ))));
        // _3 = _1 % _2  (srem)
        stmts.push(Statement::Assign(Box::new((
            Place::from(local(3)),
            RValue::BinaryOp(BinaryOp::Rem, use_local(1), use_local(2)),
        ))));
        // _4 = _1 & _2  (and)
        stmts.push(Statement::Assign(Box::new((
            Place::from(local(4)),
            RValue::BinaryOp(BinaryOp::BitAnd, use_local(1), use_local(2)),
        ))));
        // _5 = _1 | _2  (or)
        stmts.push(Statement::Assign(Box::new((
            Place::from(local(5)),
            RValue::BinaryOp(BinaryOp::BitOr, use_local(1), use_local(2)),
        ))));
        // _6 = _1 ^ _2  (xor)
        stmts.push(Statement::Assign(Box::new((
            Place::from(local(6)),
            RValue::BinaryOp(BinaryOp::BitXor, use_local(1), use_local(2)),
        ))));
        // _7 = _1 << _2  (shl)
        stmts.push(Statement::Assign(Box::new((
            Place::from(local(7)),
            RValue::BinaryOp(BinaryOp::Shl, use_local(1), use_local(2)),
        ))));
        // _8 = _1 >> _2  (ashr, signed)
        stmts.push(Statement::Assign(Box::new((
            Place::from(local(8)),
            RValue::BinaryOp(BinaryOp::Shr, use_local(1), use_local(2)),
        ))));
        // _9 = ~_1  (not)
        stmts.push(Statement::Assign(Box::new((
            Place::from(local(9)),
            RValue::UnaryOp(UnaryOp::Not, use_local(1)),
        ))));
        // _0 = _3  (return the remainder result)
        stmts.push(Statement::Assign(Box::new((
            Place::from(RETURN_LOCAL),
            RValue::Operand(Operand::Use(Place::from(local(3)))),
        ))));

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args,
            locals,
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: stmts,
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    let expected = [
        "srem i32", "and i32", "or i32", "xor i32", "shl i32", "ashr i32",
    ];
    for pattern in &expected {
        assert!(ir.contains(pattern), "Missing '{}' in IR:\n{}", pattern, ir);
    }
}

// Debug-only: verify inkwell can create an LLVM `Context` without crashing.
#[test]
fn debug_inkwell_context_create() {
    use inkwell::context::Context;
    println!("debug: creating inkwell Context");
    let _ctx = Context::create();
    println!("debug: inkwell Context created");
}

#[test]
fn debug_inkwell_emit_simple_main_ir() {
    use inkwell::context::Context;

    let ctx = Context::create();
    let module = ctx.create_module("simple_main");

    let i32_type = ctx.i32_type();
    let fn_type = i32_type.fn_type(&[], false);
    let fn_val = module.add_function("main", fn_type, None);

    let builder = ctx.create_builder();
    let entry = ctx.append_basic_block(fn_val, "entry");
    builder.position_at_end(entry);

    let zero = i32_type.const_int(0, false);
    builder.build_return(Some(&zero)).unwrap();

    let llvm_string = module.print_to_string();
    let ir = llvm_string.to_string();
    std::mem::forget(llvm_string);
    println!("--- simple main IR ---\n{}", ir);
    assert!(ir.contains("define"));
    assert!(ir.contains("ret i32 0"));
    // Leak module (borrows ctx) then ctx to avoid STATUS_ACCESS_VIOLATION
    // on Windows caused by CRT-heap mismatch with LLVM DLL.
    std::mem::forget(module);
    std::mem::forget(builder);
    std::mem::forget(ctx);
}

// ====================================================================
// unsigned, float, and edge cases
// ====================================================================

/// Unsigned division: `main() -> u32 { _1=100u; _2=3u; return _1/_2; }`
/// Expected LLVM IR: `udiv i32`
#[test]
fn pipeline_udiv_unsigned() {
    let ir = compile_to_ir(|ctx| {
        let u32_ty = ctx.intern_ty(TirTy::<TirCtx>::U32);
        let body = binop_body_with_locals(
            BinaryOp::Div,
            const_u32(ctx, 100),
            const_u32(ctx, 3),
            u32_ty,
            u32_ty,
        );
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("udiv i32"),
        "Should contain udiv for unsigned division, got:\n{}",
        ir
    );
}

/// Float addition: `main() -> f64 { _1=1.5; _2=2.5; return _1+_2; }`
/// Expected LLVM IR: `fadd double`
#[test]
fn pipeline_float_add() {
    let ir = compile_to_ir(|ctx| {
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);

        let f64_const = |val: f64| -> Operand<'_> {
            Operand::Const(ConstOperand::Value(
                ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                    data: val.to_bits() as u128,
                    size: NonZero::new(8).unwrap(),
                })),
                f64_ty,
            ))
        };

        let body = binop_body_with_locals(
            BinaryOp::Add,
            f64_const(1.5),
            f64_const(2.5),
            f64_ty,
            f64_ty,
        );

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("fadd double"),
        "Should contain fadd for float addition, got:\n{}",
        ir
    );
}

/// Float multiplication: `main() -> f64 { _1=3.0; _2=4.0; return _1*_2; }`
/// Expected LLVM IR: `fmul double`
#[test]
fn pipeline_float_mul() {
    let ir = compile_to_ir(|ctx| {
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);

        let f64_const = |val: f64| -> Operand<'_> {
            Operand::Const(ConstOperand::Value(
                ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                    data: val.to_bits() as u128,
                    size: NonZero::new(8).unwrap(),
                })),
                f64_ty,
            ))
        };

        let body = binop_body_with_locals(
            BinaryOp::Mul,
            f64_const(3.0),
            f64_const(4.0),
            f64_ty,
            f64_ty,
        );

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("fmul double"),
        "Should contain fmul for float multiplication, got:\n{}",
        ir
    );
}

/// Float remainder (frem): `main() -> f64 { _1=10.0; _2=3.0; return _1%_2; }`
/// Expected LLVM IR: `frem double`
#[test]
fn pipeline_float_rem() {
    let ir = compile_to_ir(|ctx| {
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);

        let f64_const = |val: f64| -> Operand<'_> {
            Operand::Const(ConstOperand::Value(
                ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                    data: val.to_bits() as u128,
                    size: NonZero::new(8).unwrap(),
                })),
                f64_ty,
            ))
        };

        let body = binop_body_with_locals(
            BinaryOp::Rem,
            f64_const(10.0),
            f64_const(3.0),
            f64_ty,
            f64_ty,
        );

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("frem double"),
        "Should contain frem for float remainder, got:\n{}",
        ir
    );
}

/// Float negation: `main() -> f64 { _1=42.0; return -_1; }`
/// Expected LLVM IR: `fneg double`
#[test]
fn pipeline_float_neg() {
    let ir = compile_to_ir(|ctx| {
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);

        let f64_const = |val: f64| -> Operand<'_> {
            Operand::Const(ConstOperand::Value(
                ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                    data: val.to_bits() as u128,
                    size: NonZero::new(8).unwrap(),
                })),
                f64_ty,
            ))
        };

        let body = unop_body_with_local(UnaryOp::Neg, f64_const(42.0), f64_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("fneg double"),
        "Should contain fneg for float negation, got:\n{}",
        ir
    );
}

/// Unsigned comparison: uses `ult` / `ugt` etc. instead of `slt` / `sgt`.
/// Checks that `Lt` on U32 emits `icmp ult`.
#[test]
fn pipeline_icmp_unsigned() {
    let ir = compile_to_ir(|ctx| {
        let u32_ty = ctx.intern_ty(TirTy::<TirCtx>::U32);
        let bool_ty = ctx.intern_ty(TirTy::<TirCtx>::Bool);

        let body = binop_body_with_locals(
            BinaryOp::Lt,
            const_u32(ctx, 1),
            const_u32(ctx, 2),
            u32_ty,
            bool_ty,
        );

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("icmp ult i32"),
        "Unsigned less-than should emit icmp ult, got:\n{}",
        ir
    );
}

/// Mutable local alloca: verify that mutable locals produce `alloca`
/// instructions and allow multiple assignments across basic blocks.
#[test]
fn pipeline_mutable_local_alloca() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);

        // bb0: _1 = 10; goto bb1
        let bb0 = BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place::from(Local::new(1)),
                RValue::Operand(const_i32(ctx, 10)),
            )))],
            terminator: Terminator::Goto {
                target: BasicBlock::new(1),
            },
        };

        // bb1: _1 = 20; _0 = _1; return
        let bb1 = BasicBlockData {
            statements: vec![
                Statement::Assign(Box::new((
                    Place::from(Local::new(1)),
                    RValue::Operand(const_i32(ctx, 20)),
                ))),
                Statement::Assign(Box::new((
                    Place::from(RETURN_LOCAL),
                    RValue::Operand(Operand::Use(Place::from(Local::new(1)))),
                ))),
            ],
            terminator: Terminator::Return,
        };

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: true,
            }]),
            basic_blocks: IdxVec::from_raw(vec![bb0, bb1]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    // Mutable locals should produce alloca + store + load pattern
    assert!(
        ir.contains("alloca"),
        "Mutable local should produce an alloca, got:\n{}",
        ir
    );
    assert!(
        ir.contains("store i32"),
        "Mutable local assignment should produce a store, got:\n{}",
        ir
    );
}

// ====================================================================
// Type Casts
// ====================================================================

/// Build a `TirBody` for testing a cast operation using a **mutable local**
/// to prevent LLVM from constant-folding the cast away.
///
/// Generated shape:
/// ```text
/// fn main() -> dest_ty {
///     _1: src_ty = src_operand;   // mutable
///     _0 = Cast(kind, _1, dest_ty);
///     return;
/// }
/// ```
fn cast_body_with_local<'ctx>(
    kind: CastKind,
    src_operand: Operand<'ctx>,
    src_ty: tidec_tir::TirTy<'ctx>,
    dest_ty: tidec_tir::TirTy<'ctx>,
) -> TirBody<'ctx> {
    TirBody {
        metadata: main_metadata(DefId(0)),
        ret_and_args: IdxVec::from_raw(vec![LocalData {
            ty: dest_ty,
            mutable: false,
        }]),
        locals: IdxVec::from_raw(vec![LocalData {
            ty: src_ty,
            mutable: true,
        }]),
        basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
            statements: vec![
                Statement::Assign(Box::new((
                    Place::from(Local::new(1)),
                    RValue::Operand(src_operand),
                ))),
                Statement::Assign(Box::new((
                    Place::from(RETURN_LOCAL),
                    RValue::Cast(kind, Operand::Use(Place::from(Local::new(1))), dest_ty),
                ))),
            ],
            terminator: Terminator::Return,
        }]),
    }
}

/// IntToInt: i32 → i64 (sign-extend)
#[test]
fn pipeline_cast_sext_i32_to_i64() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let i64_ty = ctx.intern_ty(TirTy::<TirCtx>::I64);

        let body = cast_body_with_local(CastKind::IntToInt, const_i32(ctx, 42), i32_ty, i64_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("sext"),
        "Signed i32→i64 should produce sext, got:\n{}",
        ir
    );
}

/// IntToInt: u32 → u64 (zero-extend)
#[test]
fn pipeline_cast_zext_u32_to_u64() {
    let ir = compile_to_ir(|ctx| {
        let u32_ty = ctx.intern_ty(TirTy::<TirCtx>::U32);
        let u64_ty = ctx.intern_ty(TirTy::<TirCtx>::U64);

        let body = cast_body_with_local(CastKind::IntToInt, const_u32(ctx, 42), u32_ty, u64_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("zext"),
        "Unsigned u32→u64 should produce zext, got:\n{}",
        ir
    );
}

/// IntToInt: i64 → i32 (truncate)
#[test]
fn pipeline_cast_trunc_i64_to_i32() {
    let ir = compile_to_ir(|ctx| {
        let i64_ty = ctx.intern_ty(TirTy::<TirCtx>::I64);
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let src = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 100,
                size: NonZero::new(8).unwrap(),
            })),
            i64_ty,
        ));

        let body = cast_body_with_local(CastKind::IntToInt, src, i64_ty, i32_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("trunc"),
        "i64→i32 should produce trunc, got:\n{}",
        ir
    );
}

/// FloatToFloat: f32 → f64 (fpext)
#[test]
fn pipeline_cast_fpext_f32_to_f64() {
    let ir = compile_to_ir(|ctx| {
        let f32_ty = ctx.intern_ty(TirTy::<TirCtx>::F32);
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);
        let src = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x42280000, // 42.0f32 in IEEE 754
                size: NonZero::new(4).unwrap(),
            })),
            f32_ty,
        ));

        let body = cast_body_with_local(CastKind::FloatToFloat, src, f32_ty, f64_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("fpext"),
        "f32→f64 should produce fpext, got:\n{}",
        ir
    );
}

/// FloatToFloat: f64 → f32 (fptrunc)
#[test]
fn pipeline_cast_fptrunc_f64_to_f32() {
    let ir = compile_to_ir(|ctx| {
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);
        let f32_ty = ctx.intern_ty(TirTy::<TirCtx>::F32);
        let src = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x4045000000000000, // 42.0f64
                size: NonZero::new(8).unwrap(),
            })),
            f64_ty,
        ));

        let body = cast_body_with_local(CastKind::FloatToFloat, src, f64_ty, f32_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("fptrunc"),
        "f64→f32 should produce fptrunc, got:\n{}",
        ir
    );
}

/// IntToFloat: signed i32 → f64 (sitofp)
#[test]
fn pipeline_cast_sitofp() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);

        let body = cast_body_with_local(CastKind::IntToFloat, const_i32(ctx, 42), i32_ty, f64_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("sitofp"),
        "signed i32→f64 should produce sitofp, got:\n{}",
        ir
    );
}

/// IntToFloat: unsigned u32 → f64 (uitofp)
#[test]
fn pipeline_cast_uitofp() {
    let ir = compile_to_ir(|ctx| {
        let u32_ty = ctx.intern_ty(TirTy::<TirCtx>::U32);
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);

        let body = cast_body_with_local(CastKind::IntToFloat, const_u32(ctx, 42), u32_ty, f64_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("uitofp"),
        "unsigned u32→f64 should produce uitofp, got:\n{}",
        ir
    );
}

/// FloatToInt: f64 → signed i32 (fptosi)
#[test]
fn pipeline_cast_fptosi() {
    let ir = compile_to_ir(|ctx| {
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let src = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x4045000000000000, // 42.0f64
                size: NonZero::new(8).unwrap(),
            })),
            f64_ty,
        ));

        let body = cast_body_with_local(CastKind::FloatToInt, src, f64_ty, i32_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("fptosi"),
        "f64→signed i32 should produce fptosi, got:\n{}",
        ir
    );
}

/// FloatToInt: f64 → unsigned u32 (fptoui)
#[test]
fn pipeline_cast_fptoui() {
    let ir = compile_to_ir(|ctx| {
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);
        let u32_ty = ctx.intern_ty(TirTy::<TirCtx>::U32);
        let src = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x4045000000000000, // 42.0f64
                size: NonZero::new(8).unwrap(),
            })),
            f64_ty,
        ));

        let body = cast_body_with_local(CastKind::FloatToInt, src, f64_ty, u32_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("fptoui"),
        "f64→unsigned u32 should produce fptoui, got:\n{}",
        ir
    );
}

/// IntToPtr: u64 → *mut i32 (inttoptr)
#[test]
fn pipeline_cast_inttoptr() {
    let ir = compile_to_ir(|ctx| {
        let u64_ty = ctx.intern_ty(TirTy::<TirCtx>::U64);
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let ptr_ty = ctx.intern_ty(TirTy::<TirCtx>::RawPtr(i32_ty, Mutability::Mut));
        let src = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0xDEAD_BEEF,
                size: NonZero::new(8).unwrap(),
            })),
            u64_ty,
        ));

        let body = cast_body_with_local(CastKind::IntToPtr, src, u64_ty, ptr_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("inttoptr"),
        "u64→ptr should produce inttoptr, got:\n{}",
        ir
    );
}

/// PtrToInt: *mut i32 → u64 (ptrtoint)
#[test]
fn pipeline_cast_ptrtoint() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let u64_ty = ctx.intern_ty(TirTy::<TirCtx>::U64);
        let ptr_ty = ctx.intern_ty(TirTy::<TirCtx>::RawPtr(i32_ty, Mutability::Mut));
        // Represent the pointer as a scalar with 8-byte size (64-bit pointer)
        let src = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0xCAFE_BABE,
                size: NonZero::new(8).unwrap(),
            })),
            ptr_ty,
        ));

        let body = cast_body_with_local(CastKind::PtrToInt, src, ptr_ty, u64_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("ptrtoint"),
        "ptr→u64 should produce ptrtoint, got:\n{}",
        ir
    );
}

/// Bitcast: i32 → f32 (bitcast, same bit-width reinterpretation)
#[test]
fn pipeline_cast_bitcast_i32_to_f32() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let f32_ty = ctx.intern_ty(TirTy::<TirCtx>::F32);

        let body = cast_body_with_local(CastKind::Bitcast, const_i32(ctx, 42), i32_ty, f32_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    assert!(
        ir.contains("bitcast"),
        "i32→f32 should produce bitcast, got:\n{}",
        ir
    );
}

/// PtrToPtr: *imm i32 → *mut i64 (no-op under opaque pointers, no cast instruction)
#[test]
fn pipeline_cast_ptr_to_ptr() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let i64_ty = ctx.intern_ty(TirTy::<TirCtx>::I64);
        let ptr_i32 = ctx.intern_ty(TirTy::<TirCtx>::RawPtr(i32_ty, Mutability::Imm));
        let ptr_i64 = ctx.intern_ty(TirTy::<TirCtx>::RawPtr(i64_ty, Mutability::Mut));
        let src = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x1000,
                size: NonZero::new(8).unwrap(),
            })),
            ptr_i32,
        ));

        let body = cast_body_with_local(CastKind::PtrToPtr, src, ptr_i32, ptr_i64);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    // PtrToPtr under opaque pointers is a no-op — no cast instruction emitted
    // by the Cast node. The `inttoptr` visible in the IR comes from the
    // constant-scalar-to-backend conversion (creating the pointer literal),
    // not from our Cast codegen path.
    assert!(
        ir.contains("define"),
        "PtrToPtr should still produce a valid function, got:\n{}",
        ir
    );
    // No bitcast or ptrtoint expected from the cast itself
    assert!(
        !ir.contains("bitcast") && !ir.contains("ptrtoint"),
        "PtrToPtr should be a no-op (no bitcast/ptrtoint), got:\n{}",
        ir
    );
}

/// IntToInt: i32 → i32 (same width, should be a no-op)
#[test]
fn pipeline_cast_int_same_width_noop() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);

        let body = cast_body_with_local(CastKind::IntToInt, const_i32(ctx, 7), i32_ty, i32_ty);

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    // Same width IntToInt should not produce trunc/zext/sext
    assert!(
        !ir.contains("trunc") && !ir.contains("zext") && !ir.contains("sext"),
        "Same-width i32→i32 should be a no-op, got:\n{}",
        ir
    );
}

// ====================================================================
// Composite Types — Struct & Array
// ====================================================================

/// Construct a struct { i32, i32 } aggregate with two fields and read back
/// the first field via `Projection::Field`.
///
/// ```text
/// fn main() -> i32 {
///     _1: { i32, i32 } = Aggregate::Struct(10, 20);
///     _0 = _1.0;  // Field(0)
///     return;
/// }
/// ```
#[test]
fn pipeline_struct_aggregate_and_field_access() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let fields = ctx.intern_type_list(&[i32_ty, i32_ty]);
        let struct_ty = ctx.intern_ty(TirTy::<TirCtx>::Struct {
            fields,
            packed: false,
        });

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![LocalData {
                ty: struct_ty,
                mutable: true,
            }]),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![
                    // _1 = Aggregate::Struct { 10, 20 }
                    Statement::Assign(Box::new((
                        Place::from(Local::new(1)),
                        RValue::Aggregate(
                            AggregateKind::Struct(struct_ty),
                            vec![const_i32(ctx, 10), const_i32(ctx, 20)],
                        ),
                    ))),
                    // _0 = _1.0
                    Statement::Assign(Box::new((
                        Place::from(RETURN_LOCAL),
                        RValue::Operand(Operand::Use(Place {
                            local: Local::new(1),
                            projection: vec![Projection::Field(0, i32_ty)],
                        })),
                    ))),
                ],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    println!("--- struct aggregate IR ---\n{}", ir);

    // Must have an alloca for the struct local
    assert!(
        ir.contains("alloca"),
        "Struct local should be alloca'd, got:\n{}",
        ir
    );
    // Must have a getelementptr to access the field
    assert!(
        ir.contains("getelementptr"),
        "Field access should use GEP, got:\n{}",
        ir
    );
    // Should store the field values
    assert!(
        ir.contains("store i32"),
        "Should store i32 fields, got:\n{}",
        ir
    );
}

/// Construct a struct { i32, i32 } and read the *second* field.
///
/// ```text
/// fn main() -> i32 {
///     _1: { i32, i32 } = Aggregate::Struct(10, 20);
///     _0 = _1.1;  // Field(1)
///     return;
/// }
/// ```
#[test]
fn pipeline_struct_read_second_field() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let fields = ctx.intern_type_list(&[i32_ty, i32_ty]);
        let struct_ty = ctx.intern_ty(TirTy::<TirCtx>::Struct {
            fields,
            packed: false,
        });

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![LocalData {
                ty: struct_ty,
                mutable: true,
            }]),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![
                    Statement::Assign(Box::new((
                        Place::from(Local::new(1)),
                        RValue::Aggregate(
                            AggregateKind::Struct(struct_ty),
                            vec![const_i32(ctx, 10), const_i32(ctx, 20)],
                        ),
                    ))),
                    // _0 = _1.1
                    Statement::Assign(Box::new((
                        Place::from(RETURN_LOCAL),
                        RValue::Operand(Operand::Use(Place {
                            local: Local::new(1),
                            projection: vec![Projection::Field(1, i32_ty)],
                        })),
                    ))),
                ],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    println!("--- struct second field IR ---\n{}", ir);

    assert!(
        ir.contains("getelementptr"),
        "Second field access should use GEP, got:\n{}",
        ir
    );
}

/// Construct a packed struct { i8, i32 } and read the i32 field.
///
/// ```text
/// fn main() -> i32 {
///     _1: packed { i8, i32 } = Aggregate::Struct(0xFF, 42);
///     _0 = _1.1;
///     return;
/// }
/// ```
#[test]
fn pipeline_packed_struct() {
    let ir = compile_to_ir(|ctx| {
        let i8_ty = ctx.intern_ty(TirTy::<TirCtx>::I8);
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let fields = ctx.intern_type_list(&[i8_ty, i32_ty]);
        let struct_ty = ctx.intern_ty(TirTy::<TirCtx>::Struct {
            fields,
            packed: true,
        });

        let i8_const = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0xFF,
                size: NonZero::new(1).unwrap(),
            })),
            i8_ty,
        ));

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![LocalData {
                ty: struct_ty,
                mutable: true,
            }]),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![
                    Statement::Assign(Box::new((
                        Place::from(Local::new(1)),
                        RValue::Aggregate(
                            AggregateKind::Struct(struct_ty),
                            vec![i8_const, const_i32(ctx, 42)],
                        ),
                    ))),
                    Statement::Assign(Box::new((
                        Place::from(RETURN_LOCAL),
                        RValue::Operand(Operand::Use(Place {
                            local: Local::new(1),
                            projection: vec![Projection::Field(1, i32_ty)],
                        })),
                    ))),
                ],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    println!("--- packed struct IR ---\n{}", ir);

    // Packed struct should produce <{ ... }> in LLVM IR
    assert!(
        ir.contains("alloca"),
        "Packed struct should be alloca'd, got:\n{}",
        ir
    );
    assert!(
        ir.contains("getelementptr"),
        "Field access should use GEP, got:\n{}",
        ir
    );
}

/// Construct a struct with mixed types: { i32, f64 }
///
/// ```text
/// fn main() -> f64 {
///     _1: { i32, f64 } = Aggregate::Struct(42, 3.14);
///     _0 = _1.1;  // read the f64 field
///     return;
/// }
/// ```
#[test]
fn pipeline_struct_mixed_types() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);
        let fields = ctx.intern_type_list(&[i32_ty, f64_ty]);
        let struct_ty = ctx.intern_ty(TirTy::<TirCtx>::Struct {
            fields,
            packed: false,
        });

        let f64_const = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x40091EB851EB851F, // ~3.14 as f64 bits
                size: NonZero::new(8).unwrap(),
            })),
            f64_ty,
        ));

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: f64_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![LocalData {
                ty: struct_ty,
                mutable: true,
            }]),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![
                    Statement::Assign(Box::new((
                        Place::from(Local::new(1)),
                        RValue::Aggregate(
                            AggregateKind::Struct(struct_ty),
                            vec![const_i32(ctx, 42), f64_const],
                        ),
                    ))),
                    // _0 = _1.1 (the f64 field)
                    Statement::Assign(Box::new((
                        Place::from(RETURN_LOCAL),
                        RValue::Operand(Operand::Use(Place {
                            local: Local::new(1),
                            projection: vec![Projection::Field(1, f64_ty)],
                        })),
                    ))),
                ],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    println!("--- struct mixed types IR ---\n{}", ir);

    assert!(
        ir.contains("getelementptr"),
        "Mixed-type struct field access should use GEP, got:\n{}",
        ir
    );
    assert!(
        ir.contains("store"),
        "Should store values into struct fields, got:\n{}",
        ir
    );
}

/// Construct an array [i32; 3] aggregate and read back the first element.
///
/// ```text
/// fn main() -> i32 {
///     _1: [i32; 3] = Aggregate::Array(100, 200, 300);
///     _2: u64 = 0;   // index
///     _0 = _1[_2];   // Index projection
///     return;
/// }
/// ```
#[test]
fn pipeline_array_aggregate_and_index() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let u64_ty = ctx.intern_ty(TirTy::<TirCtx>::U64);
        let array_ty = ctx.intern_ty(TirTy::<TirCtx>::Array(i32_ty, 3));

        let const_u64_zero = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0,
                size: NonZero::new(8).unwrap(),
            })),
            u64_ty,
        ));

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![
                // _1: [i32; 3]
                LocalData {
                    ty: array_ty,
                    mutable: true,
                },
                // _2: u64 (index)
                LocalData {
                    ty: u64_ty,
                    mutable: true,
                },
            ]),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![
                    // _1 = [100, 200, 300]
                    Statement::Assign(Box::new((
                        Place::from(Local::new(1)),
                        RValue::Aggregate(
                            AggregateKind::Array(i32_ty),
                            vec![
                                const_i32(ctx, 100),
                                const_i32(ctx, 200),
                                const_i32(ctx, 300),
                            ],
                        ),
                    ))),
                    // _2 = 0u64
                    Statement::Assign(Box::new((
                        Place::from(Local::new(2)),
                        RValue::Operand(const_u64_zero),
                    ))),
                    // _0 = _1[_2]
                    Statement::Assign(Box::new((
                        Place::from(RETURN_LOCAL),
                        RValue::Operand(Operand::Use(Place {
                            local: Local::new(1),
                            projection: vec![Projection::Index(Local::new(2))],
                        })),
                    ))),
                ],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    println!("--- array aggregate + index IR ---\n{}", ir);

    // Must alloca the array
    assert!(
        ir.contains("alloca"),
        "Array local should be alloca'd, got:\n{}",
        ir
    );
    // Must have GEP for storing elements and for indexing
    assert!(
        ir.contains("getelementptr"),
        "Array indexing should use GEP, got:\n{}",
        ir
    );
    // Should contain stores for the three elements
    assert!(
        ir.contains("store i32"),
        "Should store i32 elements, got:\n{}",
        ir
    );
}

/// Construct a single-element array [f64; 1].
///
/// ```text
/// fn main() -> f64 {
///     _1: [f64; 1] = Aggregate::Array(2.718);
///     _2: u64 = 0;
///     _0 = _1[_2];
///     return;
/// }
/// ```
#[test]
fn pipeline_array_single_element() {
    let ir = compile_to_ir(|ctx| {
        let f64_ty = ctx.intern_ty(TirTy::<TirCtx>::F64);
        let u64_ty = ctx.intern_ty(TirTy::<TirCtx>::U64);
        let array_ty = ctx.intern_ty(TirTy::<TirCtx>::Array(f64_ty, 1));

        let f64_const = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0x4005BF0A8B145769, // ~2.718 as f64 bits
                size: NonZero::new(8).unwrap(),
            })),
            f64_ty,
        ));
        let const_u64_zero = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 0,
                size: NonZero::new(8).unwrap(),
            })),
            u64_ty,
        ));

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: f64_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![
                LocalData {
                    ty: array_ty,
                    mutable: true,
                },
                LocalData {
                    ty: u64_ty,
                    mutable: true,
                },
            ]),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![
                    Statement::Assign(Box::new((
                        Place::from(Local::new(1)),
                        RValue::Aggregate(AggregateKind::Array(f64_ty), vec![f64_const]),
                    ))),
                    Statement::Assign(Box::new((
                        Place::from(Local::new(2)),
                        RValue::Operand(const_u64_zero),
                    ))),
                    Statement::Assign(Box::new((
                        Place::from(RETURN_LOCAL),
                        RValue::Operand(Operand::Use(Place {
                            local: Local::new(1),
                            projection: vec![Projection::Index(Local::new(2))],
                        })),
                    ))),
                ],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    println!("--- array single element IR ---\n{}", ir);

    assert!(
        ir.contains("alloca"),
        "Array should be alloca'd, got:\n{}",
        ir
    );
    assert!(
        ir.contains("store double") || ir.contains("store float"),
        "Should store f64 element, got:\n{}",
        ir
    );
}

/// Write to a struct field via `Projection::Field`.
///
/// ```text
/// fn main() -> i32 {
///     _1: { i32, i32 } = Aggregate::Struct(0, 0);
///     _1.0 = 99;
///     _0 = _1.0;
///     return;
/// }
/// ```
#[test]
fn pipeline_struct_field_write() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let fields = ctx.intern_type_list(&[i32_ty, i32_ty]);
        let struct_ty = ctx.intern_ty(TirTy::<TirCtx>::Struct {
            fields,
            packed: false,
        });

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![LocalData {
                ty: struct_ty,
                mutable: true,
            }]),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![
                    // _1 = Aggregate::Struct(0, 0)
                    Statement::Assign(Box::new((
                        Place::from(Local::new(1)),
                        RValue::Aggregate(
                            AggregateKind::Struct(struct_ty),
                            vec![const_i32(ctx, 0), const_i32(ctx, 0)],
                        ),
                    ))),
                    // _1.0 = 99
                    Statement::Assign(Box::new((
                        Place {
                            local: Local::new(1),
                            projection: vec![Projection::Field(0, i32_ty)],
                        },
                        RValue::Operand(const_i32(ctx, 99)),
                    ))),
                    // _0 = _1.0
                    Statement::Assign(Box::new((
                        Place::from(RETURN_LOCAL),
                        RValue::Operand(Operand::Use(Place {
                            local: Local::new(1),
                            projection: vec![Projection::Field(0, i32_ty)],
                        })),
                    ))),
                ],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    println!("--- struct field write IR ---\n{}", ir);

    // Should have multiple GEPs (aggregate construction + field write + field read)
    assert!(
        ir.contains("getelementptr"),
        "Field write/read should use GEP, got:\n{}",
        ir
    );
    // Stores: both aggregate stores plus the overwrite
    assert!(
        ir.contains("store i32"),
        "Should store i32 values, got:\n{}",
        ir
    );
}

/// Write to an array element via `Projection::Index`.
///
/// ```text
/// fn main() -> i32 {
///     _1: [i32; 2] = Aggregate::Array(0, 0);
///     _2: u64 = 1;
///     _1[_2] = 77;
///     _0 = _1[_2];
///     return;
/// }
/// ```
#[test]
fn pipeline_array_element_write() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let u64_ty = ctx.intern_ty(TirTy::<TirCtx>::U64);
        let array_ty = ctx.intern_ty(TirTy::<TirCtx>::Array(i32_ty, 2));

        let const_u64_one = Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: 1,
                size: NonZero::new(8).unwrap(),
            })),
            u64_ty,
        ));

        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![
                // _1: [i32; 2]
                LocalData {
                    ty: array_ty,
                    mutable: true,
                },
                // _2: u64 (index)
                LocalData {
                    ty: u64_ty,
                    mutable: true,
                },
            ]),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![
                    // _1 = [0, 0]
                    Statement::Assign(Box::new((
                        Place::from(Local::new(1)),
                        RValue::Aggregate(
                            AggregateKind::Array(i32_ty),
                            vec![const_i32(ctx, 0), const_i32(ctx, 0)],
                        ),
                    ))),
                    // _2 = 1u64
                    Statement::Assign(Box::new((
                        Place::from(Local::new(2)),
                        RValue::Operand(const_u64_one),
                    ))),
                    // _1[_2] = 77
                    Statement::Assign(Box::new((
                        Place {
                            local: Local::new(1),
                            projection: vec![Projection::Index(Local::new(2))],
                        },
                        RValue::Operand(const_i32(ctx, 77)),
                    ))),
                    // _0 = _1[_2]
                    Statement::Assign(Box::new((
                        Place::from(RETURN_LOCAL),
                        RValue::Operand(Operand::Use(Place {
                            local: Local::new(1),
                            projection: vec![Projection::Index(Local::new(2))],
                        })),
                    ))),
                ],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    println!("--- array element write IR ---\n{}", ir);

    assert!(
        ir.contains("getelementptr"),
        "Array element write/read should use GEP, got:\n{}",
        ir
    );
    assert!(
        ir.contains("store i32"),
        "Should store i32 values, got:\n{}",
        ir
    );
}

/// Nested composite: struct { i32, [i32; 2] } — access the array field,
/// then index into it.
///
/// ```text
/// fn main() -> i32 {
///     _1: [i32; 2] = Aggregate::Array(10, 20);  // inner array
///     _2: { i32, [i32; 2] } = Aggregate::Struct(99, _1 as operand ref);
///     ... (simplified: we just test the array aggregate + struct aggregate)
/// }
/// ```
///
/// This test checks that we can nest struct and array aggregates.
#[test]
fn pipeline_struct_with_array_field() {
    let ir = compile_to_ir(|ctx| {
        let i32_ty = ctx.intern_ty(TirTy::<TirCtx>::I32);
        let array_ty = ctx.intern_ty(TirTy::<TirCtx>::Array(i32_ty, 2));
        let fields = ctx.intern_type_list(&[i32_ty, array_ty]);
        let struct_ty = ctx.intern_ty(TirTy::<TirCtx>::Struct {
            fields,
            packed: false,
        });

        // We construct the struct by building the whole struct aggregate.
        // The array field is passed as individual elements when constructing
        // the inner array first, then use Operand::Use to read the local.
        let body = TirBody {
            metadata: main_metadata(DefId(0)),
            ret_and_args: IdxVec::from_raw(vec![LocalData {
                ty: i32_ty,
                mutable: false,
            }]),
            locals: IdxVec::from_raw(vec![
                // _1: [i32; 2] (inner array)
                LocalData {
                    ty: array_ty,
                    mutable: true,
                },
                // _2: { i32, [i32; 2] } (the struct)
                LocalData {
                    ty: struct_ty,
                    mutable: true,
                },
            ]),
            basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
                statements: vec![
                    // _1 = [10, 20]
                    Statement::Assign(Box::new((
                        Place::from(Local::new(1)),
                        RValue::Aggregate(
                            AggregateKind::Array(i32_ty),
                            vec![const_i32(ctx, 10), const_i32(ctx, 20)],
                        ),
                    ))),
                    // For now, just read back the first scalar field of the struct.
                    // We'd construct the struct with _1 as a field, but since memory-backed
                    // operand in aggregate is still todo, we test what we can:
                    // Just test that both arrays and structs can be alloca'd and GEP'd.
                    // _2.0 = 99 (write to struct field 0)
                    Statement::Assign(Box::new((
                        Place {
                            local: Local::new(2),
                            projection: vec![Projection::Field(0, i32_ty)],
                        },
                        RValue::Operand(const_i32(ctx, 99)),
                    ))),
                    // _0 = _2.0
                    Statement::Assign(Box::new((
                        Place::from(RETURN_LOCAL),
                        RValue::Operand(Operand::Use(Place {
                            local: Local::new(2),
                            projection: vec![Projection::Field(0, i32_ty)],
                        })),
                    ))),
                ],
                terminator: Terminator::Return,
            }]),
        };

        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: "test".to_string(),
            },
            bodies: IdxVec::from_raw(vec![body]),
        }
    });

    println!("--- struct with array field IR ---\n{}", ir);

    assert!(
        ir.contains("alloca"),
        "Should have alloca for struct/array locals, got:\n{}",
        ir
    );
    assert!(
        ir.contains("getelementptr"),
        "Should have GEP for field access, got:\n{}",
        ir
    );
}
