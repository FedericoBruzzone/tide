use std::num::NonZero;

use tidec_abi::size_and_align::Size;
use tidec_builder::body::{
    CallConv, DefId, Linkage, TirBody, TirBodyKind, TirBodyMetadata, TirItemKind, TirUnit,
    TirUnitMetadata, UnnamedAddress, Visibility,
};
use tidec_builder::syntax::{
    BasicBlock, BasicBlockData, ConstOperand, ConstScalar, ConstValue, Local, LocalData, Operand,
    Place, RValue, RawScalarValue, Statement, Terminator, UnaryOp, RETURN_LOCAL,
};
use tidec_builder::BuilderCtx;
use tidec_driver::{compile_unit, init_tidec_logger, BackendKind, CompileConfig, EmitKind};
use tidec_tir::ctx::TirCtx;
use tidec_utils::idx::Idx;
use tidec_utils::index_vec::IdxVec;
use tracing::debug;

// ─── Examples ────────────────────────────────────────────────────────────────

/// Example: `int main() { return 10; }`
fn build_example_return10<'a>(tir_ctx: &TirCtx<'a>) -> TirUnit<'a> {
    let builder_ctx = BuilderCtx::new(*tir_ctx);
    let i32_ty = builder_ctx.i32();

    let metadata = TirBodyMetadata {
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

    let bodies = IdxVec::from_raw(vec![TirBody {
        metadata,
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
                            data: 10u128,
                            size: NonZero::new(4).unwrap(),
                        })),
                        i32_ty,
                    )),
                ),
            )))],
            terminator: Terminator::Return,
        }]),
    }]);

    TirUnit {
        metadata: TirUnitMetadata {
            unit_name: "main".to_string(),
        },
        globals: IdxVec::new(),
        bodies,
    }
}

/// Example: `printf("Hello, World! %d\n", 42); return 0;`
fn build_example_printf<'a>(tir_ctx: &TirCtx<'a>) -> TirUnit<'a> {
    let builder_ctx = BuilderCtx::new(*tir_ctx);
    let i8_ty = builder_ctx.i8();
    let ptr_i8_ty = builder_ctx.ptr_imm(i8_ty);
    let i32_ty = builder_ctx.i32();

    // Declare printf (external, variadic)
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

    let printf_alloc_id = tir_ctx.intern_fn(printf_def_id);
    let format_alloc_id = tir_ctx.intern_c_str("Hello, World! %d\n");

    // Define main
    let main_body = TirBody {
        metadata: TirBodyMetadata {
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
        },
        ret_and_args: IdxVec::from_raw(vec![LocalData {
            ty: i32_ty,
            mutable: false,
        }]),
        locals: IdxVec::from_raw(vec![LocalData {
            ty: i32_ty,
            mutable: false,
        }]),
        basic_blocks: IdxVec::from_raw(vec![
            // bb0: call printf, then jump to bb1
            BasicBlockData {
                statements: vec![],
                terminator: Terminator::Call {
                    func: Operand::Const(ConstOperand::Value(
                        ConstValue::Indirect {
                            alloc_id: printf_alloc_id,
                            offset: Size::ZERO,
                        },
                        ptr_i8_ty,
                    )),
                    args: vec![
                        Operand::Const(ConstOperand::Value(
                            ConstValue::Indirect {
                                alloc_id: format_alloc_id,
                                offset: Size::ZERO,
                            },
                            ptr_i8_ty,
                        )),
                        Operand::Const(ConstOperand::Value(
                            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                                data: 42u128,
                                size: NonZero::new(4).unwrap(),
                            })),
                            i32_ty,
                        )),
                    ],
                    destination: Place {
                        local: Local::new(1),
                        projection: vec![],
                    },
                    target: BasicBlock::new(1),
                },
            },
            // bb1: return 0
            BasicBlockData {
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
            },
        ]),
    };

    TirUnit {
        metadata: TirUnitMetadata {
            unit_name: "main".to_string(),
        },
        globals: IdxVec::new(),
        bodies: IdxVec::from_raw(vec![printf_body, main_body]),
    }
}

// ─── CLI ─────────────────────────────────────────────────────────────────────

/// Tiny argument parser for the tidec demo CLI.
///
/// Usage:
///   tidec [--emit=object|assembly|llvm-ir|llvm-bc|exe] [--example=printf|return10]
fn parse_args() -> (CompileConfig, &'static str) {
    let mut config = CompileConfig::default();
    let mut example = "printf";

    for arg in std::env::args().skip(1) {
        if let Some(value) = arg.strip_prefix("--emit=") {
            config.emit = match value {
                "object" | "obj" | "o" => EmitKind::Object,
                "assembly" | "asm" | "s" => EmitKind::Assembly,
                "llvm-ir" | "ir" | "ll" => EmitKind::LlvmIr,
                "llvm-bc" | "bc" => EmitKind::LlvmBitcode,
                "exe" | "executable" => EmitKind::Executable,
                other => {
                    eprintln!("Unknown emit kind: {other}");
                    eprintln!("Valid options: object, assembly, llvm-ir, llvm-bc, exe");
                    std::process::exit(1);
                }
            };
        } else if let Some(value) = arg.strip_prefix("--backend=") {
            config.backend = match value {
                "llvm" => BackendKind::Llvm,
                "cranelift" => BackendKind::Cranelift,
                "gcc" => BackendKind::Gcc,
                other => {
                    eprintln!("Unknown backend: {other}");
                    eprintln!("Valid options: llvm, cranelift, gcc");
                    std::process::exit(1);
                }
            };
        } else if let Some(value) = arg.strip_prefix("--example=") {
            example = match value {
                "printf" => "printf",
                "return10" | "return_10" | "simple" => "return10",
                other => {
                    eprintln!("Unknown example: {other}");
                    eprintln!("Valid options: printf, return10");
                    std::process::exit(1);
                }
            };
        } else if arg == "--help" || arg == "-h" {
            println!("tidec — Tide compiler demo CLI");
            println!();
            println!("Usage:");
            println!("  tidec [OPTIONS]");
            println!();
            println!("Options:");
            println!("  --emit=<kind>       Output kind: object (default), assembly, llvm-ir, llvm-bc, exe");
            println!("  --backend=<name>    Backend: llvm (default), cranelift, gcc");
            println!("  --example=<name>    Example program: printf (default), return10");
            println!("  -h, --help          Show this help message");
            std::process::exit(0);
        } else {
            eprintln!("Unknown argument: {arg}");
            eprintln!("Run with --help for usage information.");
            std::process::exit(1);
        }
    }

    (config, example)
}

// ─── Main ────────────────────────────────────────────────────────────────────

/// TIDEC_LOG=debug cargo run -- --emit=object --example=printf; \
///   cc main.o -o a.out; ./a.out; echo $?
fn main() {
    init_tidec_logger();
    debug!("Logging initialized");

    let (config, example) = parse_args();

    let result = compile_unit(&config, |tir_ctx| match example {
        "printf" => build_example_printf(tir_ctx),
        "return10" => build_example_return10(tir_ctx),
        _ => unreachable!(),
    });

    match result {
        Ok(output) => {
            debug!("Compilation succeeded: emit_kind={:?}", output.emit_kind);
            if let Some(ref ir) = output.ir_string {
                println!("{ir}");
            }
        }
        Err(err) => {
            eprintln!("Compilation failed: {err}");
            std::process::exit(1);
        }
    }
}
