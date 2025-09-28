use std::num::NonZero;
// #[macro_use] extern crate tidec_utils;
//
use tidec_abi::target::BackendKind;
use tidec_codegen_llvm::entry::llvm_codegen_lir_unit;
use tidec_tir::basic_blocks::BasicBlockData;
use tidec_tir::syntax::{
    ConstOperand, ConstScalar, ConstValue, LocalData, Place, RValue, RawScalarValue, Statement,
    Terminator, TirTy, RETURN_LOCAL,
};
use tidec_tir::tir::{
    CallConv, DefId, EmitKind, Linkage, TirBody, TirBodyKind, TirBodyMetadata, TirCtx, TirItemKind,
    TirUnit, TirUnitMetadata, UnnamedAddress, Visibility,
};
use tidec_utils::index_vec::IdxVec;
use tracing::debug;

// TIDEC_LOG=debug cargo run; cc main.o -o a.out; ./a.out; echo $?
fn main() {
    init_tidec_logger();
    debug!("Logging initialized");

    // TODO: check valitiy of TideArgs
    let lir_ctx = TirCtx::new(BackendKind::Llvm, EmitKind::LlvmIr);

    // Create a simple main function that returns 0.
    // ```c
    // int main() {
    //   return 0;
    // }
    // ```
    let lir_body_metadata = TirBodyMetadata {
        def_id: DefId(0),
        name: "main".to_string(),
        kind: TirBodyKind::Item(TirItemKind::Function),
        inlined: false,
        linkage: Linkage::External, // TODO(bruzzone): Check the correct linkage
        visibility: Visibility::Default,
        unnamed_address: UnnamedAddress::None,
        call_conv: CallConv::C,
    };
    let lir_bodies = IdxVec::from_raw(vec![TirBody {
        metadata: lir_body_metadata,
        ret_and_args: IdxVec::from_raw(vec![LocalData {
            // ty: TirTy::F32,
            ty: TirTy::I32,
            mutable: false,
        }]),
        locals: IdxVec::new(),
        basic_blocks: IdxVec::from_raw(vec![BasicBlockData {
            statements: vec![Statement::Assign(Box::new((
                Place {
                    local: RETURN_LOCAL,
                    projection: vec![],
                },
                RValue::Const(ConstOperand::Value(
                    // ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                    //     data: 7.7f32.to_bits() as u128,
                    //     size: NonZero::new(4).unwrap(), // 4 bytes for f32
                    // })),
                    // TirTy::F32,
                    ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                        data: 7u128,
                        size: NonZero::new(4).unwrap(), // 4 bytes for i32
                    })),
                    TirTy::I32,
                )),
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

    codegen_lir_unit(lir_ctx, lir_unit);
}

pub fn codegen_lir_unit(lir_ctx: TirCtx, lir_unit: TirUnit) {
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

// TIDEC_LOG=debug cargo run; clang main.ll -o main; ./main; echo $?
//
// Create a simple main function that returns the value stored in the first place.
// ```c
// int main() {
//    int _0 = 5; // The first place
//    return _0;
// }
// ```
// fn main2() {
//     init_tidec_logger();
//     debug!("Logging initialized");
//
//     let lir_ctx = TirTyCtx::new(BackendKind::Llvm);
//
//     let context = Context::create();
//     let module = context.create_module("main");
//     // let builder = context.create_builder();
//     let code_gen_ctx = CodegenCtx::new(lir_ctx, &context, module);
//     let codegen = CodegenBuilder::with_ctx(&code_gen_ctx);
//
//     let i32_type = codegen.ctx().ll_context.i32_type();
//     let fn_type = i32_type.fn_type(&[], false);
//     let function = codegen.ctx().ll_module.add_function("main", fn_type, None);
//     let basic_block = codegen.ctx().ll_context.append_basic_block(function, "entry");
//     // It is important to set the position at the end of the basic block, which in this case is the
//     // start of the entry block.
//     codegen.ll_builder.position_at_end(basic_block);
//
//     // Declare an integer variable
//     let _0 = codegen.ll_builder.build_alloca(i32_type, "_0").unwrap();
//     // Store the 5 in the first_place
//     let i32_five = i32_type.const_int(5, false);
//     let _ = codegen.ll_builder.build_store(_0, i32_five).unwrap();
//
//     // codegen.builder.build_return(Some(&i64_type.const_int(0, false))).unwrap(); // Reutrn 0
//     // Dereference the _0 and return it
//     let deref_0 = codegen.ll_builder.build_load(i32_type, _0, "_0").unwrap();
//     codegen.ll_builder.build_return(Some(&deref_0)).unwrap();
//
//     codegen
//         .ctx()
//         .ll_module
//         .print_to_file(Path::new("main.ll"))
//         .unwrap();
//     // module.print_to_stderr();
//
//     // =========================
//     // ========= TESTS =========
//     // =========================
//
//     let int_value = TirTy::I8.into_basic_type(codegen.ctx()).size_of().unwrap();
//     let align = int_value.get_type().get_alignment();
//     println!("Size of i8: {}", int_value);
//     println!("Alignment of i8: {}", align);
// }
