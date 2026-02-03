use std::num::NonZero;
// #[macro_use] extern crate tidec_utils;
//
use tidec_abi::size_and_align::Size;
use tidec_abi::target::{BackendKind, TirTarget};
use tidec_codegen_llvm::entry::llvm_codegen_lir_unit;

use tidec_tir::body::{
    CallConv, DefId, Linkage, TirBody, TirBodyKind, TirBodyMetadata, TirItemKind, TirUnit,
    TirUnitMetadata, UnnamedAddress, Visibility,
};
use tidec_tir::ctx::{EmitKind, InternCtx, TirArena, TirArgs, TirCtx};
use tidec_tir::syntax::{
    BasicBlock, BasicBlockData, ConstOperand, ConstScalar, ConstValue, Local, LocalData, Operand,
    Place, RValue, RawScalarValue, Statement, Terminator, UnaryOp, RETURN_LOCAL,
};
use tidec_tir::ty::{Mutability, TirTy};
use tidec_utils::idx::Idx;
use tidec_utils::index_vec::IdxVec;
use tracing::debug;

/// Example with printf that prints "Hello, World! 42\n" and returns 0.
/// ```c
/// #include <stdio.h>
/// int main() {
///     printf("Hello, World! %d\n", 42);
///     return 0;
/// }
/// ```
///
/// TIDEC_LOG=debug cargo run; \
///   ld main.o -o a.out -lSystem -syslibroot `xcrun --show-sdk-path` \
///   ./a.out; echo $?
fn example_printf<'a>(tir_ctx: &TirCtx<'a>) -> TirUnit<'a> {
    // Intern the pointer type for i8 (for printf's char* parameter)
    let i8_ty = tir_ctx.intern_ty(TirTy::<TirCtx>::I8);
    let ptr_i8_ty = tir_ctx.intern_ty(TirTy::RawPtr(i8_ty, Mutability::Imm));
    let i32_ty = tir_ctx.intern_ty(TirTy::<TirCtx>::I32);

    // Declare printf as an external function
    // int printf(const char* format, ...)
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
        is_varargs: true,     // printf is variadic
        is_declaration: true, // This is just a declaration, no body
    };

    let printf_body = TirBody {
        metadata: printf_metadata,
        ret_and_args: IdxVec::from_raw(vec![
            // Return type: i32
            LocalData {
                ty: i32_ty,
                mutable: false,
            },
            // First parameter: const char* (pointer to i8)
            LocalData {
                ty: ptr_i8_ty,
                mutable: false,
            },
        ]),
        locals: IdxVec::new(),
        basic_blocks: IdxVec::new(), // No basic blocks for external declaration
    };

    // Register printf as a function allocation using TirCtx
    let printf_alloc_id = tir_ctx.intern_fn(printf_def_id);

    // Register the format string as a memory allocation using TirCtx
    let format_alloc_id = tir_ctx.intern_c_str("Hello, World! %d\n");

    // Create the main function that calls printf
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

    // Local 0: return value (i32)
    // Local 1: result of printf call (i32) - we'll store this but not use it
    let main_ret_and_args = IdxVec::from_raw(vec![LocalData {
        ty: i32_ty,
        mutable: false,
    }]);

    let main_locals = IdxVec::from_raw(vec![LocalData {
        ty: i32_ty, // For storing printf return value
        mutable: false,
    }]);

    // Basic block 0: entry - call printf and jump to bb1
    // Basic block 1: set return value and return
    let bb0 = BasicBlockData {
        statements: vec![],
        terminator: Terminator::Call {
            // Function reference via Indirect allocation
            func: Operand::Const(ConstOperand::Value(
                ConstValue::Indirect {
                    alloc_id: printf_alloc_id,
                    offset: Size::ZERO,
                },
                ptr_i8_ty, // Function pointer type
            )),
            args: vec![
                // Format string via Indirect allocation
                Operand::Const(ConstOperand::Value(
                    ConstValue::Indirect {
                        alloc_id: format_alloc_id,
                        offset: Size::ZERO,
                    },
                    ptr_i8_ty,
                )),
                // Integer argument: 42
                Operand::Const(ConstOperand::Value(
                    ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                        data: 42u128,
                        size: NonZero::new(4).unwrap(),
                    })),
                    i32_ty,
                )),
            ],
            destination: Place {
                local: Local::new(1), // Store result in local 1 (first non-return local)
                projection: vec![],
            },
            target: BasicBlock::new(1), // Continue to bb1
        },
    };

    let bb1 = BasicBlockData {
        statements: vec![
            // Set return value to 0
            Statement::Assign(Box::new((
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
            ))),
        ],
        terminator: Terminator::Return,
    };

    let main_body = TirBody {
        metadata: main_metadata,
        ret_and_args: main_ret_and_args,
        locals: main_locals,
        basic_blocks: IdxVec::from_raw(vec![bb0, bb1]),
    };

    let unit_metadata = TirUnitMetadata {
        unit_name: "main".to_string(),
    };

    TirUnit {
        metadata: unit_metadata,
        bodies: IdxVec::from_raw(vec![printf_body, main_body]),
    }
}

// TIDEC_LOG=debug cargo run; cc main.o -o a.out; ./a.out; echo $?
fn main() {
    init_tidec_logger();
    debug!("Logging initialized");

    let target = TirTarget::new(BackendKind::Llvm);
    let arguments = TirArgs {
        emit_kind: EmitKind::Object,
    };
    let tir_arena = TirArena::default();
    let intern_ctx = InternCtx::new(&tir_arena);

    // TODO: check validity of TideArgs
    let tir_ctx = TirCtx::new(&target, &arguments, &intern_ctx);

    // Use example_printf to demonstrate external function calls
    let lir_unit = example_printf(&tir_ctx);
    // Alternatively, use example1 for the simple case:
    // let lir_unit = example1(&tir_ctx);

    codegen_lir_unit(tir_ctx, lir_unit);
}

pub fn codegen_lir_unit<'ctx>(lir_ctx: TirCtx<'ctx>, lir_unit: TirUnit<'ctx>) {
    match lir_ctx.backend_kind() {
        BackendKind::Llvm => llvm_codegen_lir_unit(lir_ctx, lir_unit),
        BackendKind::Cranelift => todo!(),
        BackendKind::Gcc => todo!(),
    }
}

/// Initialize the logger for the tidec project.
fn init_tidec_logger() {
    if let Err(err) = tidec_log::Logger::init_logger(
        tidec_log::LoggerConfig::from_prefix("TIDEC").unwrap(),
        tidec_log::FallbackDefaultEnv::No,
    ) {
        eprintln!("Error initializing logger: {:?}", err);
        std::process::exit(1);
    }
}

// Keep example1 available for testing simple case
// TIDEC_LOG=debug cargo run; cc main.o -o a.out; ./a.out; echo $?
#[allow(dead_code)]
fn run_example1() {
    init_tidec_logger();
    debug!("Logging initialized");

    let target = TirTarget::new(BackendKind::Llvm);
    let arguments = TirArgs {
        emit_kind: EmitKind::Object,
    };
    let tir_arena = TirArena::default();
    let intern_ctx = InternCtx::new(&tir_arena);

    let tir_ctx = TirCtx::new(&target, &arguments, &intern_ctx);
    let lir_unit = example1(&tir_ctx);

    codegen_lir_unit(tir_ctx, lir_unit);
}

fn example1<'a>(tir_ctx: &TirCtx<'a>) -> TirUnit<'a> {
    // Create a simple main function that returns 10.
    // ```c
    // int main() {
    //   return 10;
    // }
    // ```
    let lir_body_metadata = TirBodyMetadata {
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
    let lir_bodies = IdxVec::from_raw(vec![TirBody {
        metadata: lir_body_metadata,
        ret_and_args: IdxVec::from_raw(vec![LocalData {
            ty: tir_ctx.intern_ty(TirTy::<TirCtx>::I32),
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
                            data: 10u128,
                            size: NonZero::new(4).unwrap(), // 4 bytes for i32
                        })),
                        tir_ctx.intern_ty(TirTy::<TirCtx>::I32),
                    )),
                ),
            )))],
            terminator: Terminator::Return,
        }]),
    }]);
    let lit_unit_metadata = TirUnitMetadata {
        unit_name: "main".to_string(),
    };

    let lir_unit: TirUnit = TirUnit {
        metadata: lit_unit_metadata,
        bodies: lir_bodies,
    };

    lir_unit
}
