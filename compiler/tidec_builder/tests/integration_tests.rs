//! Integration tests for `tidec_builder`.
//!
//! Each test constructs a complete TIR module end-to-end using the builder API
//! and then asserts on the resulting structure.

use std::num::NonZero;

use tidec_abi::target::{BackendKind, TirTarget};
use tidec_builder::{BasicBlockBuilder, FunctionBuilder, UnitBuilder};
use tidec_tir::body::*;
use tidec_tir::ctx::{EmitKind, InternCtx, TirArena, TirArgs, TirCtx};
use tidec_tir::syntax::*;
use tidec_tir::ty;
use tidec_utils::idx::Idx;

/// Helper to run a closure with a fresh `TirCtx`.
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

fn make_metadata(name: &str) -> TirBodyMetadata {
    TirBodyMetadata {
        def_id: DefId(0),
        name: name.to_string(),
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

// ---------------------------------------------------------------------------
// Test: build a simple `add(a, b) -> a + b` function inside a module.
// ---------------------------------------------------------------------------

#[test]
fn build_add_function_module() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);

        // -- Build the function body: i32 add(i32 %a, i32 %b) { return %a + %b; }
        let mut fb = FunctionBuilder::new(make_metadata("add"));

        // _0: i32 (return place)
        let ret = fb.declare_ret(i32_ty, false);
        assert_eq!(ret, RETURN_LOCAL);

        // _1: i32 (first arg)
        let arg_a = fb.declare_arg(i32_ty, false);
        // _2: i32 (second arg)
        let arg_b = fb.declare_arg(i32_ty, false);

        assert_eq!(fb.num_args(), 2);
        assert_eq!(fb.num_locals(), 3); // ret + 2 args

        // Create entry block
        let entry = fb.create_block();

        // entry:
        //   _0 = Add(_1, _2)
        //   return
        fb.push_assign(
            entry,
            Place::from(RETURN_LOCAL),
            RValue::BinaryOp(
                BinaryOp::Add,
                Operand::Use(Place::from(arg_a)),
                Operand::Use(Place::from(arg_b)),
            ),
        );
        fb.set_terminator(entry, Terminator::Return);

        let body = fb.build();

        // Verify the body structure.
        assert_eq!(body.metadata.name, "add");
        assert_eq!(body.ret_and_args.len(), 3); // ret + 2 args
        assert!(body.locals.is_empty()); // no extra locals
        assert_eq!(body.basic_blocks.len(), 1);

        let bb0 = &body.basic_blocks[BasicBlock::new(0)];
        assert_eq!(bb0.statements.len(), 1);
        assert!(matches!(bb0.terminator, Terminator::Return));

        // -- Wrap the body in a module.
        let mut unit = UnitBuilder::new("add_module");
        let body_id = unit.add_body(body);
        assert!(body_id.idx() == 0);
        assert_eq!(unit.num_bodies(), 1);

        let tir_unit = unit.build();
        assert_eq!(tir_unit.metadata.unit_name, "add_module");
        assert_eq!(tir_unit.bodies.len(), 1);
        assert!(tir_unit.globals.is_empty());
    });
}

// ---------------------------------------------------------------------------
// Test: build a function with a global variable and a branch.
//
//   global counter: i32 = 0
//
//   fn maybe_increment(cond: bool) -> i32 {
//       if cond { counter + 1 } else { counter }
//   }
// ---------------------------------------------------------------------------

#[test]
fn build_module_with_global_and_branch() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let bool_ty = ctx.intern_ty(ty::TirTy::Bool);

        // -- Global: `counter = 0`
        let scalar_zero = ConstScalar::Value(RawScalarValue {
            data: 0,
            size: NonZero::new(4).unwrap(),
        });
        let global = TirGlobal {
            name: "counter".to_string(),
            ty: i32_ty,
            initializer: Some(ConstValue::Scalar(scalar_zero)),
            mutable: true,
            linkage: Linkage::Internal,
            visibility: Visibility::Default,
            unnamed_address: UnnamedAddress::None,
        };

        // -- Function: maybe_increment
        let mut fb = FunctionBuilder::new(make_metadata("maybe_increment"));
        let _ret = fb.declare_ret(i32_ty, false);
        let cond = fb.declare_arg(bool_ty, false);
        let counter_local = fb.declare_local(i32_ty, false); // _3, holds loaded counter
        let tmp = fb.declare_local(i32_ty, true); // _4, holds counter+1

        // entry: switch on cond -> then_bb / else_bb
        let entry = fb.create_block();
        let then_bb = fb.create_block();
        let else_bb = fb.create_block();
        let merge_bb = fb.create_block();

        assert_eq!(fb.num_blocks(), 4);

        // entry: switchInt(cond) [1 -> then_bb, otherwise -> else_bb]
        fb.set_terminator(
            entry,
            Terminator::SwitchInt {
                discr: Operand::Use(Place::from(cond)),
                targets: SwitchTargets::if_then(then_bb, else_bb),
            },
        );

        // then_bb: _4 = _3 + 1; _0 = _4; goto merge
        {
            let one = ConstOperand::Value(
                ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                    data: 1,
                    size: NonZero::new(4).unwrap(),
                })),
                i32_ty,
            );
            let mut bb = BasicBlockBuilder::new();
            bb.push_assign_binary_op(
                Place::from(tmp),
                BinaryOp::Add,
                Operand::Use(Place::from(counter_local)),
                Operand::Const(one),
            );
            bb.push_assign_operand(Place::from(RETURN_LOCAL), Operand::Use(Place::from(tmp)));
            let data = bb.build(Terminator::Goto { target: merge_bb });
            fb.apply_block_builder(then_bb, data);
        }

        // else_bb: _0 = _3; goto merge
        fb.push_assign(
            else_bb,
            Place::from(RETURN_LOCAL),
            RValue::Operand(Operand::Use(Place::from(counter_local))),
        );
        fb.set_terminator(else_bb, Terminator::Goto { target: merge_bb });

        // merge_bb: return
        fb.set_terminator(merge_bb, Terminator::Return);

        let body = fb.build();

        // Verify function structure.
        assert_eq!(body.ret_and_args.len(), 2); // ret + cond
        assert_eq!(body.locals.len(), 2); // counter_local + tmp
        assert_eq!(body.basic_blocks.len(), 4);

        // Verify then_bb has 2 statements (add + assign).
        let then_data = &body.basic_blocks[then_bb];
        assert_eq!(then_data.statements.len(), 2);
        assert!(matches!(
            then_data.terminator,
            Terminator::Goto { target } if target == merge_bb
        ));

        // Verify else_bb has 1 statement (assign).
        let else_data = &body.basic_blocks[else_bb];
        assert_eq!(else_data.statements.len(), 1);

        // Verify merge_bb has no statements, just return.
        let merge_data = &body.basic_blocks[merge_bb];
        assert!(merge_data.statements.is_empty());
        assert!(matches!(merge_data.terminator, Terminator::Return));

        // -- Assemble the module.
        let mut unit = UnitBuilder::new("branch_module");
        let gid = unit.add_global(global);
        let bid = unit.add_body(body);

        assert_eq!(unit.num_globals(), 1);
        assert_eq!(unit.num_bodies(), 1);
        assert_eq!(unit.get_global(gid).name, "counter");
        assert_eq!(unit.get_body(&bid).metadata.name, "maybe_increment");

        let tir_unit = unit.build();
        assert_eq!(tir_unit.metadata.unit_name, "branch_module");
        assert_eq!(tir_unit.globals.len(), 1);
        assert_eq!(tir_unit.bodies.len(), 1);
    });
}

// ---------------------------------------------------------------------------
// Test: build a declaration-only (extern) function and a calling function.
//
//   declare i32 @ext_fn(i32)
//
//   define i32 @caller(i32 %x) {
//     _2 = call @ext_fn(%x)
//     return _2
//   }
// ---------------------------------------------------------------------------

#[test]
fn build_module_with_declaration_and_call() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);

        // -- External declaration: i32 ext_fn(i32)
        let mut ext_meta = make_metadata("ext_fn");
        ext_meta.is_declaration = true;
        ext_meta.def_id = DefId(0);
        let mut ext_fb = FunctionBuilder::new(ext_meta);
        ext_fb.declare_ret(i32_ty, false);
        ext_fb.declare_arg(i32_ty, false);
        // Declarations don't need blocks.
        // We add a dummy unreachable block so the builder doesn't complain.
        let ext_entry = ext_fb.create_block();
        ext_fb.set_terminator(ext_entry, Terminator::Unreachable);
        let ext_body = ext_fb.build();

        assert!(ext_body.metadata.is_declaration);
        assert_eq!(ext_body.metadata.name, "ext_fn");

        // -- Caller function: i32 caller(i32 %x) { return ext_fn(%x); }
        let mut caller_meta = make_metadata("caller");
        caller_meta.def_id = DefId(1);
        let mut caller_fb = FunctionBuilder::new(caller_meta);

        let _ret = caller_fb.declare_ret(i32_ty, false);
        let x = caller_fb.declare_arg(i32_ty, false);
        let dest = caller_fb.declare_local(i32_ty, true);

        let entry = caller_fb.create_block();
        let cont = caller_fb.create_block();

        // entry: _2 = call ext_fn(_1); goto cont
        // We use _1 as a stand-in operand for the function pointer (simplified).
        caller_fb.set_terminator(
            entry,
            Terminator::Call {
                func: Operand::Use(Place::from(x)), // placeholder
                args: vec![Operand::Use(Place::from(x))],
                destination: Place::from(dest),
                target: cont,
            },
        );

        // cont: _0 = _2; return
        caller_fb.push_assign(
            cont,
            Place::from(RETURN_LOCAL),
            RValue::Operand(Operand::Use(Place::from(dest))),
        );
        caller_fb.set_terminator(cont, Terminator::Return);

        let caller_body = caller_fb.build();

        assert_eq!(caller_body.basic_blocks.len(), 2);
        assert!(matches!(
            caller_body.basic_blocks[BasicBlock::new(0)].terminator,
            Terminator::Call { .. }
        ));

        // -- Assemble the module.
        let mut unit = UnitBuilder::new("call_module");
        unit.add_body(ext_body);
        unit.add_body(caller_body);

        assert_eq!(unit.num_bodies(), 2);

        let tir_unit = unit.build();
        assert_eq!(tir_unit.bodies.len(), 2);
        assert_eq!(tir_unit.bodies.raw[0].metadata.name, "ext_fn");
        assert_eq!(tir_unit.bodies.raw[1].metadata.name, "caller");
    });
}

// ---------------------------------------------------------------------------
// Test: build a module with struct aggregate construction.
//
//   struct Pair { i32, f64 }
//
//   define Pair @make_pair(i32 %a, f64 %b) {
//       _3 = Aggregate::Struct(Pair, [%a, %b])
//       _0 = _3
//       return
//   }
// ---------------------------------------------------------------------------

#[test]
fn build_module_with_struct_aggregate() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);

        let fields = ctx.intern_type_list(&[i32_ty, f64_ty]);
        let pair_ty = ctx.intern_ty(ty::TirTy::Struct {
            fields,
            packed: false,
        });

        let mut fb = FunctionBuilder::new(make_metadata("make_pair"));
        fb.declare_ret(pair_ty, false);
        let a = fb.declare_arg(i32_ty, false);
        let b = fb.declare_arg(f64_ty, false);
        let tmp = fb.declare_local(pair_ty, false);

        let entry = fb.create_block();

        // _3 = Aggregate::Struct(pair_ty, [_1, _2])
        fb.push_assign(
            entry,
            Place::from(tmp),
            RValue::Aggregate(
                AggregateKind::Struct(pair_ty),
                vec![Operand::Use(Place::from(a)), Operand::Use(Place::from(b))],
            ),
        );
        // _0 = _3
        fb.push_assign(
            entry,
            Place::from(RETURN_LOCAL),
            RValue::Operand(Operand::Use(Place::from(tmp))),
        );
        fb.set_terminator(entry, Terminator::Return);

        let body = fb.build();
        assert_eq!(body.basic_blocks[BasicBlock::new(0)].statements.len(), 2);

        let mut unit = UnitBuilder::new("struct_module");
        unit.add_body(body);
        let tir_unit = unit.build();

        assert_eq!(tir_unit.bodies.len(), 1);
        assert_eq!(tir_unit.bodies.raw[0].metadata.name, "make_pair");
    });
}

// ---------------------------------------------------------------------------
// Test: build a module with a cast (int → float).
//
//   define f64 @int_to_float(i32 %x) {
//       _0 = IntToFloat(_1) as f64
//       return
//   }
// ---------------------------------------------------------------------------

#[test]
fn build_module_with_cast() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let f64_ty = ctx.intern_ty(ty::TirTy::F64);

        let mut fb = FunctionBuilder::new(make_metadata("int_to_float"));
        fb.declare_ret(f64_ty, false);
        let x = fb.declare_arg(i32_ty, false);

        let entry = fb.create_block();

        // Use BasicBlockBuilder for variety.
        let mut bb = BasicBlockBuilder::new();
        bb.push_assign_cast(
            Place::from(RETURN_LOCAL),
            CastKind::IntToFloat,
            Operand::Use(Place::from(x)),
            f64_ty,
        );
        let data = bb.build(Terminator::Return);

        fb.apply_block_builder(entry, data);

        let body = fb.build();
        assert_eq!(body.basic_blocks.len(), 1);
        assert_eq!(body.basic_blocks[BasicBlock::new(0)].statements.len(), 1);

        let mut unit = UnitBuilder::new("cast_module");
        unit.add_body(body);
        let tir_unit = unit.build();

        assert_eq!(tir_unit.metadata.unit_name, "cast_module");
        assert_eq!(tir_unit.bodies.len(), 1);
    });
}

// ---------------------------------------------------------------------------
// Test: build a module with an address-of operation.
//
//   define *imm i32 @take_addr(i32 %x) {
//       _0 = &imm _1
//       return
//   }
// ---------------------------------------------------------------------------

#[test]
fn build_module_with_address_of() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let ptr_ty = ctx.intern_ty(ty::TirTy::RawPtr(i32_ty, ty::Mutability::Imm));

        let mut fb = FunctionBuilder::new(make_metadata("take_addr"));
        fb.declare_ret(ptr_ty, false);
        let x = fb.declare_arg(i32_ty, false);

        let entry = fb.create_block();

        let mut bb = BasicBlockBuilder::new();
        bb.push_assign_address_of(
            Place::from(RETURN_LOCAL),
            ty::Mutability::Imm,
            Place::from(x),
        );
        fb.apply_block_builder(entry, bb.build(Terminator::Return));

        let body = fb.build();

        let mut unit = UnitBuilder::new("addr_module");
        unit.add_body(body);
        let tir_unit = unit.build();

        assert_eq!(tir_unit.bodies.len(), 1);
        assert_eq!(tir_unit.bodies.raw[0].metadata.name, "take_addr");
    });
}

// ---------------------------------------------------------------------------
// Test: build a module with multiple globals and multiple functions.
// ---------------------------------------------------------------------------

#[test]
fn build_large_module() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);
        let unit_ty = ctx.intern_ty(ty::TirTy::Unit);

        let mut unit = UnitBuilder::new("large_module");

        // Add 5 globals.
        for i in 0..5 {
            let global = TirGlobal {
                name: format!("g{}", i),
                ty: i32_ty,
                initializer: Some(ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                    data: i as u128,
                    size: NonZero::new(4).unwrap(),
                }))),
                mutable: i % 2 == 0,
                linkage: Linkage::External,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::None,
            };
            unit.add_global(global);
        }

        // Add 3 trivial functions.
        for i in 0..3 {
            let ret_ty = if i == 0 { unit_ty } else { i32_ty };
            let mut fb = FunctionBuilder::new(make_metadata(&format!("fn_{}", i)));
            fb.declare_ret(ret_ty, false);
            let entry = fb.create_block();
            fb.set_terminator(entry, Terminator::Return);
            unit.add_body(fb.build());
        }

        assert_eq!(unit.num_globals(), 5);
        assert_eq!(unit.num_bodies(), 3);

        let tir_unit = unit.build();
        assert_eq!(tir_unit.metadata.unit_name, "large_module");
        assert_eq!(tir_unit.globals.len(), 5);
        assert_eq!(tir_unit.bodies.len(), 3);

        // Verify global names.
        for i in 0..5 {
            assert_eq!(tir_unit.globals.raw[i].name, format!("g{}", i));
        }

        // Verify function names.
        for i in 0..3 {
            assert_eq!(tir_unit.bodies.raw[i].metadata.name, format!("fn_{}", i));
        }
    });
}

// ---------------------------------------------------------------------------
// Test: using BasicBlockBuilder chaining API with unary operations.
// ---------------------------------------------------------------------------

#[test]
fn chaining_basic_block_builder_in_function() {
    with_ctx(|ctx| {
        let i32_ty = ctx.intern_ty(ty::TirTy::I32);

        let mut fb = FunctionBuilder::new(make_metadata("neg_and_not"));
        fb.declare_ret(i32_ty, false);
        let x = fb.declare_arg(i32_ty, false);
        let neg_tmp = fb.declare_local(i32_ty, true);

        let entry = fb.create_block();

        let mut bb = BasicBlockBuilder::new();
        // _2 = Neg(_1)
        // _0 = Not(_2)
        bb.push_assign_unary_op(
            Place::from(neg_tmp),
            UnaryOp::Neg,
            Operand::Use(Place::from(x)),
        )
        .push_assign_unary_op(
            Place::from(RETURN_LOCAL),
            UnaryOp::Not,
            Operand::Use(Place::from(neg_tmp)),
        );

        assert_eq!(bb.len(), 2);
        fb.apply_block_builder(entry, bb.build(Terminator::Return));

        let body = fb.build();
        assert_eq!(body.basic_blocks[BasicBlock::new(0)].statements.len(), 2);

        let unit = UnitBuilder::new("chain_mod");
        // We intentionally leave the module with zero bodies, just to show the
        // builder allows it.
        let tir_unit = unit.build();
        assert!(tir_unit.bodies.is_empty());
    });
}
