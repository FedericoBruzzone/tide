//! Integration tests for `tidec_builder`.
//!
//! Each test constructs a complete TIR module end-to-end using the builder API
//! and then asserts on the resulting structure.
//!
//! These tests use the new `BuilderCtx` API which handles interning automatically.

use std::num::NonZero;

use tidec_builder::{BasicBlockBuilder, BuildError, BuilderCtx};
use tidec_tir::body::*;
use tidec_tir::syntax::*;
use tidec_tir::ty::Mutability;
use tidec_utils::idx::Idx;

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
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();

        // -- Build the function body: i32 add(i32 %a, i32 %b) { return %a + %b; }
        let mut fb = ctx.function_builder(make_metadata("add"));

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
        let mut unit = ctx.unit_builder("add_module");
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
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let bool_ty = ctx.bool();

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
        let mut fb = ctx.function_builder(make_metadata("maybe_increment"));
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
        let mut unit = ctx.unit_builder("branch_module");
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
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();

        // -- External declaration: i32 ext_fn(i32)
        let mut ext_meta = make_metadata("ext_fn");
        ext_meta.is_declaration = true;
        ext_meta.def_id = DefId(0);
        let mut ext_fb = ctx.function_builder(ext_meta);
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
        let mut caller_fb = ctx.function_builder(caller_meta);

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
        let mut unit = ctx.unit_builder("call_module");
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
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let f64_ty = ctx.f64();

        // Create struct type with automatic interning
        let pair_ty = ctx.struct_ty(&[i32_ty, f64_ty], false);

        let mut fb = ctx.function_builder(make_metadata("make_pair"));
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

        let mut unit = ctx.unit_builder("struct_module");
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
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let f64_ty = ctx.f64();

        let mut fb = ctx.function_builder(make_metadata("int_to_float"));
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

        let mut unit = ctx.unit_builder("cast_module");
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
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        // Create pointer type with automatic interning
        let ptr_ty = ctx.ptr_imm(i32_ty);

        let mut fb = ctx.function_builder(make_metadata("take_addr"));
        fb.declare_ret(ptr_ty, false);
        let x = fb.declare_arg(i32_ty, false);

        let entry = fb.create_block();

        let mut bb = BasicBlockBuilder::new();
        bb.push_assign_address_of(Place::from(RETURN_LOCAL), Mutability::Imm, Place::from(x));
        fb.apply_block_builder(entry, bb.build(Terminator::Return));

        let body = fb.build();

        let mut unit = ctx.unit_builder("addr_module");
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
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let unit_ty = ctx.unit();

        let mut unit = ctx.unit_builder("large_module");

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
            let mut fb = ctx.function_builder(make_metadata(&format!("fn_{}", i)));
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
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();

        let mut fb = ctx.function_builder(make_metadata("neg_and_not"));
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

        let unit = ctx.unit_builder("chain_mod");
        // We intentionally leave the module with zero bodies, just to show the
        // builder allows it.
        let tir_unit = unit.build();
        assert!(tir_unit.bodies.is_empty());
    });
}

// ---------------------------------------------------------------------------
// Test: verify type interning works correctly through BuilderCtx.
// ---------------------------------------------------------------------------

#[test]
fn type_interning_through_builder_ctx() {
    BuilderCtx::with_default(|ctx| {
        // Same type created multiple times should be deduplicated
        let i32_a = ctx.i32();
        let i32_b = ctx.i32();
        assert_eq!(i32_a, i32_b);

        // Different types should be different
        let f64_ty = ctx.f64();
        assert_ne!(i32_a, f64_ty);

        // Pointer types should be deduplicated
        let ptr1 = ctx.ptr_imm(i32_a);
        let ptr2 = ctx.ptr_imm(i32_b);
        assert_eq!(ptr1, ptr2);

        // Mutable vs immutable pointers should differ
        let ptr_mut = ctx.ptr_mut(i32_a);
        assert_ne!(ptr1, ptr_mut);

        // Struct types should work correctly
        let struct1 = ctx.struct_ty(&[i32_a, f64_ty], false);
        let struct2 = ctx.struct_ty(&[i32_b, f64_ty], false);
        // Note: struct types may not be deduplicated at the type list level,
        // but the internal types should be the same
        assert!(struct1.is_struct());
        assert!(struct2.is_struct());

        // Array types
        let arr = ctx.array(i32_a, 10);
        assert!(arr.is_array());
    });
}

// ---------------------------------------------------------------------------
// Test: verify allocation interning through BuilderCtx.
// ---------------------------------------------------------------------------

#[test]
fn allocation_interning_through_builder_ctx() {
    BuilderCtx::with_default(|ctx| {
        // Intern C strings
        let str1 = ctx.intern_c_str("hello");
        let str2 = ctx.intern_c_str("world");
        assert_ne!(str1, str2);

        // Intern bytes
        let bytes1 = ctx.intern_bytes(&[1, 2, 3, 4]);
        let bytes2 = ctx.intern_bytes(&[5, 6, 7, 8]);
        assert_ne!(bytes1, bytes2);

        // Intern functions
        let fn1 = ctx.intern_fn(DefId(0));
        let fn2 = ctx.intern_fn(DefId(1));
        assert_ne!(fn1, fn2);

        // Intern statics
        let static1 = ctx.intern_static(GlobalId::new(0));
        let static2 = ctx.intern_static(GlobalId::new(1));
        assert_ne!(static1, static2);
    });
}

// ---------------------------------------------------------------------------
// Test: build a module with arrays.
// ---------------------------------------------------------------------------

#[test]
fn build_module_with_array_type() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let arr_ty = ctx.array(i32_ty, 4);

        // Global array initialized to zeros
        let global = TirGlobal {
            name: "arr".to_string(),
            ty: arr_ty,
            initializer: Some(ConstValue::ZST), // placeholder
            mutable: true,
            linkage: Linkage::External,
            visibility: Visibility::Default,
            unnamed_address: UnnamedAddress::None,
        };

        let mut unit = ctx.unit_builder("array_module");
        unit.add_global(global);

        let tir_unit = unit.build();
        assert_eq!(tir_unit.globals.len(), 1);
        assert!(tir_unit.globals.raw[0].ty.is_array());
    });
}

// ---------------------------------------------------------------------------
// Test: using layout computation through BuilderCtx.
// ---------------------------------------------------------------------------

#[test]
fn layout_computation_through_builder_ctx() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let layout = ctx.layout_of(i32_ty);
        assert_eq!(layout.layout.size.bytes(), 4);

        let i64_ty = ctx.i64();
        let layout = ctx.layout_of(i64_ty);
        assert_eq!(layout.layout.size.bytes(), 8);

        let unit_ty = ctx.unit();
        let layout = ctx.layout_of(unit_ty);
        assert_eq!(layout.layout.size.bytes(), 0);
    });
}

// ===========================================================================
// Tests for feature #1: DefId allocator
// ===========================================================================

#[test]
fn fresh_def_id_is_monotonic() {
    BuilderCtx::with_default(|ctx| {
        let id0 = ctx.fresh_def_id();
        let id1 = ctx.fresh_def_id();
        let id2 = ctx.fresh_def_id();

        assert_eq!(id0, DefId(0));
        assert_eq!(id1, DefId(1));
        assert_eq!(id2, DefId(2));
    });
}

#[test]
fn fresh_def_id_used_in_metadata_factory() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();

        let id_a = ctx.fresh_def_id();
        let id_b = ctx.fresh_def_id();

        let meta_a = TirBodyMetadata::function(id_a, "fn_a");
        let meta_b = TirBodyMetadata::function(id_b, "fn_b");

        let mut fb_a = ctx.function_builder(meta_a);
        fb_a.declare_ret(i32_ty, false);
        let entry = fb_a.create_block();
        fb_a.set_terminator(entry, Terminator::Return);

        let mut fb_b = ctx.function_builder(meta_b);
        fb_b.declare_ret(i32_ty, false);
        let entry = fb_b.create_block();
        fb_b.set_terminator(entry, Terminator::Return);

        let body_a = fb_a.build();
        let body_b = fb_b.build();

        assert_eq!(body_a.metadata.def_id, DefId(0));
        assert_eq!(body_b.metadata.def_id, DefId(1));
        assert_eq!(body_a.metadata.name, "fn_a");
        assert_eq!(body_b.metadata.name, "fn_b");
    });
}

// ===========================================================================
// Tests for feature #2: TirBodyMetadata::function factory
// ===========================================================================

#[test]
fn metadata_factory_has_sensible_defaults() {
    let meta = TirBodyMetadata::function(DefId(42), "my_func");

    assert_eq!(meta.def_id, DefId(42));
    assert_eq!(meta.name, "my_func");
    assert!(matches!(
        meta.kind,
        TirBodyKind::Item(TirItemKind::Function)
    ));
    assert!(!meta.inlined);
    assert!(matches!(meta.linkage, Linkage::External));
    assert!(matches!(meta.visibility, Visibility::Default));
    assert!(matches!(meta.unnamed_address, UnnamedAddress::None));
    assert!(matches!(meta.call_conv, CallConv::C));
    assert!(!meta.is_varargs);
    assert!(!meta.is_declaration);
}

// ===========================================================================
// Tests for feature #3: Constant constructors
// ===========================================================================

#[test]
fn const_i32_produces_correct_operand() {
    BuilderCtx::with_default(|ctx| {
        let op = ctx.const_i32(42);
        if let Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(raw)),
            ty,
        )) = op
        {
            let data = raw.data;
            let size = raw.size.get();
            assert_eq!(data, 42);
            assert_eq!(size, 4);
            assert!(ty.is_integer());
        } else {
            panic!("expected scalar constant");
        }
    });
}

#[test]
fn const_i64_produces_correct_operand() {
    BuilderCtx::with_default(|ctx| {
        let op = ctx.const_i64(-1);
        if let Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(raw)),
            ty,
        )) = op
        {
            // -1i64 as u64 = u64::MAX
            let data = raw.data;
            let size = raw.size.get();
            assert_eq!(data, u64::MAX as u128);
            assert_eq!(size, 8);
            assert!(ty.is_integer());
        } else {
            panic!("expected scalar constant");
        }
    });
}

#[test]
fn const_bool_produces_correct_operand() {
    BuilderCtx::with_default(|ctx| {
        let op_true = ctx.const_bool(true);
        let op_false = ctx.const_bool(false);

        if let Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(raw)),
            ty,
        )) = op_true
        {
            let data = raw.data;
            let size = raw.size.get();
            assert_eq!(data, 1);
            assert_eq!(size, 1);
            assert!(ty.is_bool());
        } else {
            panic!("expected scalar constant for true");
        }

        if let Operand::Const(ConstOperand::Value(ConstValue::Scalar(ConstScalar::Value(raw)), _)) =
            op_false
        {
            let data = raw.data;
            assert_eq!(data, 0);
        } else {
            panic!("expected scalar constant for false");
        }
    });
}

#[test]
fn const_f64_produces_correct_operand() {
    BuilderCtx::with_default(|ctx| {
        let op = ctx.const_f64(3.14);
        if let Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(raw)),
            ty,
        )) = op
        {
            let data = raw.data;
            let size = raw.size.get();
            assert_eq!(data, 3.14f64.to_bits() as u128);
            assert_eq!(size, 8);
            assert!(ty.is_floating_point());
        } else {
            panic!("expected scalar constant");
        }
    });
}

#[test]
fn const_f32_produces_correct_operand() {
    BuilderCtx::with_default(|ctx| {
        let op = ctx.const_f32(2.5);
        if let Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(raw)),
            ty,
        )) = op
        {
            let data = raw.data;
            let size = raw.size.get();
            assert_eq!(data, 2.5f32.to_bits() as u128);
            assert_eq!(size, 4);
            assert!(ty.is_floating_point());
        } else {
            panic!("expected scalar constant");
        }
    });
}

#[test]
fn const_unsigned_types() {
    BuilderCtx::with_default(|ctx| {
        let op_u8 = ctx.const_u8(255);
        let op_u16 = ctx.const_u16(65535);
        let op_u32 = ctx.const_u32(0xDEAD_BEEF);
        let op_u64 = ctx.const_u64(0xCAFE_BABE_DEAD_BEEF);

        if let Operand::Const(ConstOperand::Value(ConstValue::Scalar(ConstScalar::Value(raw)), _)) =
            op_u8
        {
            let data = raw.data;
            let size = raw.size.get();
            assert_eq!(data, 255);
            assert_eq!(size, 1);
        } else {
            panic!("expected u8 constant");
        }

        if let Operand::Const(ConstOperand::Value(ConstValue::Scalar(ConstScalar::Value(raw)), _)) =
            op_u16
        {
            let data = raw.data;
            let size = raw.size.get();
            assert_eq!(data, 65535);
            assert_eq!(size, 2);
        } else {
            panic!("expected u16 constant");
        }

        if let Operand::Const(ConstOperand::Value(ConstValue::Scalar(ConstScalar::Value(raw)), _)) =
            op_u32
        {
            let data = raw.data;
            let size = raw.size.get();
            assert_eq!(data, 0xDEAD_BEEF);
            assert_eq!(size, 4);
        } else {
            panic!("expected u32 constant");
        }

        if let Operand::Const(ConstOperand::Value(ConstValue::Scalar(ConstScalar::Value(raw)), _)) =
            op_u64
        {
            let data = raw.data;
            let size = raw.size.get();
            assert_eq!(data, 0xCAFE_BABE_DEAD_BEEF);
            assert_eq!(size, 8);
        } else {
            panic!("expected u64 constant");
        }
    });
}

// ===========================================================================
// Tests for feature #4: Extern declaration support
// ===========================================================================

#[test]
fn extern_declaration_with_set_declaration() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let ptr_ty = ctx.ptr_imm(ctx.i8());

        // Build an extern printf-like declaration using the new API.
        let def_id = ctx.fresh_def_id();
        let mut fb = ctx.function_builder(TirBodyMetadata::function(def_id, "printf"));
        fb.set_declaration().set_varargs();
        fb.declare_ret(i32_ty, false);
        fb.declare_arg(ptr_ty, false);

        // Declarations still need a dummy block.
        let entry = fb.create_block();
        fb.set_terminator(entry, Terminator::Unreachable);
        let body = fb.build();

        assert!(body.metadata.is_declaration);
        assert!(body.metadata.is_varargs);
        assert_eq!(body.metadata.name, "printf");
    });
}

// ===========================================================================
// Tests for feature #5: Metadata modifiers
// ===========================================================================

#[test]
fn metadata_modifiers_chain() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let def_id = ctx.fresh_def_id();
        let mut fb = ctx.function_builder(TirBodyMetadata::function(def_id, "fast_fn"));

        fb.set_call_conv(CallConv::Fast)
            .set_linkage(Linkage::Internal);

        fb.declare_ret(i32_ty, false);
        let entry = fb.create_block();
        fb.set_terminator(entry, Terminator::Return);
        let body = fb.build();

        assert!(matches!(body.metadata.call_conv, CallConv::Fast));
        assert!(matches!(body.metadata.linkage, Linkage::Internal));
    });
}

#[test]
fn metadata_and_metadata_mut_access() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let def_id = ctx.fresh_def_id();
        let mut fb = ctx.function_builder(TirBodyMetadata::function(def_id, "test_fn"));

        // Read access
        assert_eq!(fb.metadata().name, "test_fn");

        // Write access
        fb.metadata_mut().inlined = true;

        fb.declare_ret(i32_ty, false);
        let entry = fb.create_block();
        fb.set_terminator(entry, Terminator::Return);
        let body = fb.build();

        assert!(body.metadata.inlined);
    });
}

// ===========================================================================
// Tests for feature #6: Statement::assign & Operand::use_local
// ===========================================================================

#[test]
fn statement_assign_helper() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();

        let mut fb = ctx.function_builder(TirBodyMetadata::function(ctx.fresh_def_id(), "test"));
        fb.declare_ret(i32_ty, false);
        let arg = fb.declare_arg(i32_ty, false);

        let entry = fb.create_block();

        // Use the new Statement::assign helper instead of manual boxing.
        fb.push_statement(
            entry,
            Statement::assign(
                Place::from(RETURN_LOCAL),
                RValue::Operand(Operand::use_local(arg)),
            ),
        );
        fb.set_terminator(entry, Terminator::Return);

        let body = fb.build();
        assert_eq!(body.basic_blocks[BasicBlock::new(0)].statements.len(), 1);
    });
}

#[test]
fn operand_use_local_shorthand() {
    let op = Operand::<'_>::use_local(Local::new(3));
    if let Operand::Use(place) = &op {
        assert_eq!(place.local, Local::new(3));
        assert!(place.projection.is_empty());
    } else {
        panic!("expected Use variant");
    }
}

// ===========================================================================
// Tests for feature #7: Function operand helper
// ===========================================================================

#[test]
fn fn_operand_creates_indirect_const() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let fn_ty = ctx.ptr_imm(i32_ty); // placeholder fn type
        let def_id = ctx.fresh_def_id();

        let op = ctx.fn_operand(def_id, fn_ty);

        if let Operand::Const(ConstOperand::Value(ConstValue::Indirect { offset, .. }, ty)) = op {
            assert_eq!(offset.bytes(), 0);
            assert_eq!(ty, fn_ty);
        } else {
            panic!("expected indirect constant");
        }
    });
}

#[test]
fn fn_operand_used_in_call() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let fn_ty = ctx.ptr_imm(i32_ty);

        // Build a callee
        let callee_id = ctx.fresh_def_id();
        let callee_meta = TirBodyMetadata::function(callee_id, "callee");
        let mut callee_fb = ctx.function_builder(callee_meta);
        callee_fb.declare_ret(i32_ty, false);
        callee_fb.declare_arg(i32_ty, false);
        callee_fb.set_declaration();
        let entry = callee_fb.create_block();
        callee_fb.set_terminator(entry, Terminator::Unreachable);
        let callee_body = callee_fb.build();

        // Build a caller that uses fn_operand
        let caller_id = ctx.fresh_def_id();
        let mut caller = ctx.function_builder(TirBodyMetadata::function(caller_id, "caller"));
        caller.declare_ret(i32_ty, false);
        let dest = caller.declare_local(i32_ty, true);

        let entry = caller.create_block();
        let cont = caller.create_block();

        let fn_op = ctx.fn_operand(callee_id, fn_ty);
        caller.set_terminator(
            entry,
            Terminator::Call {
                func: fn_op,
                args: vec![ctx.const_i32(10)],
                destination: Place::from(dest),
                target: cont,
            },
        );
        caller.push_assign(
            cont,
            Place::from(RETURN_LOCAL),
            RValue::Operand(Operand::use_local(dest)),
        );
        caller.set_terminator(cont, Terminator::Return);

        let caller_body = caller.build();

        let mut unit = ctx.unit_builder("call_module");
        unit.add_body(callee_body);
        unit.add_body(caller_body);

        let tir_unit = unit.build();
        assert_eq!(tir_unit.bodies.len(), 2);
        assert!(tir_unit.bodies.raw[0].metadata.is_declaration);
        assert!(!tir_unit.bodies.raw[1].metadata.is_declaration);
    });
}

// ===========================================================================
// Tests for feature #8: FunctionBuilder holds TirCtx (convenience methods)
// ===========================================================================

#[test]
fn function_builder_const_methods() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();

        let mut fb = ctx.function_builder(TirBodyMetadata::function(ctx.fresh_def_id(), "test"));
        fb.declare_ret(i32_ty, false);
        let tmp = fb.declare_local(i32_ty, true);

        let entry = fb.create_block();

        // Use fb.const_i32() instead of ctx.const_i32()
        fb.push_assign(
            entry,
            Place::from(tmp),
            RValue::BinaryOp(
                BinaryOp::Add,
                Operand::use_local(RETURN_LOCAL),
                fb.const_i32(42),
            ),
        );
        fb.set_terminator(entry, Terminator::Return);

        let body = fb.build();
        assert_eq!(body.basic_blocks[BasicBlock::new(0)].statements.len(), 1);
    });
}

#[test]
fn function_builder_tir_ctx_access() {
    BuilderCtx::with_default(|ctx| {
        let fb = ctx.function_builder(TirBodyMetadata::function(ctx.fresh_def_id(), "test"));
        // Created via BuilderCtx, so tir_ctx should be present.
        assert!(fb.tir_ctx().is_some());
    });
}

#[test]
fn function_builder_new_has_no_tir_ctx() {
    let fb = tidec_builder::FunctionBuilder::<'_>::new(TirBodyMetadata::function(
        DefId(0),
        "standalone",
    ));
    assert!(fb.tir_ctx().is_none());
}

// ===========================================================================
// Tests for feature #9: Error variants instead of panics
// ===========================================================================

#[test]
fn try_build_missing_ret_returns_error() {
    let fb =
        tidec_builder::FunctionBuilder::<'_>::new(TirBodyMetadata::function(DefId(0), "no_ret"));
    let result = fb.try_build();
    assert!(matches!(result, Err(BuildError::MissingReturnLocal)));
}

#[test]
fn try_build_missing_terminator_returns_error() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let mut fb = ctx.function_builder(TirBodyMetadata::function(ctx.fresh_def_id(), "test"));
        fb.declare_ret(i32_ty, false);
        fb.create_block(); // no terminator set

        let result = fb.try_build();
        assert!(matches!(
            result,
            Err(BuildError::MissingTerminator { block: 0 })
        ));
    });
}

#[test]
fn try_build_success() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let mut fb = ctx.function_builder(TirBodyMetadata::function(ctx.fresh_def_id(), "ok_fn"));
        fb.declare_ret(i32_ty, false);
        let entry = fb.create_block();
        fb.set_terminator(entry, Terminator::Return);

        let result = fb.try_build();
        assert!(result.is_ok());
        let body = result.unwrap();
        assert_eq!(body.metadata.name, "ok_fn");
    });
}

#[test]
fn build_error_display() {
    let err = BuildError::MissingReturnLocal;
    assert!(err.to_string().contains("return local"));

    let err = BuildError::MissingTerminator { block: 3 };
    assert!(err.to_string().contains("3"));
    assert!(err.to_string().contains("terminator"));
}

// ===========================================================================
// End-to-end: build a multi-function module with the new API.
//
//   declare i32 @ext_pow(i32, i32)   -- extern, declaration
//
//   define i32 @square(i32 %x) {
//       _2 = call @ext_pow(%x, 2)
//       _0 = _2
//       return
//   }
// ===========================================================================

#[test]
fn end_to_end_multi_function_with_new_api() {
    BuilderCtx::with_default(|ctx| {
        let i32_ty = ctx.i32();
        let fn_ty = ctx.ptr_imm(i32_ty);

        // -- extern i32 ext_pow(i32, i32)
        let pow_id = ctx.fresh_def_id();
        let mut pow_fb = ctx.function_builder(TirBodyMetadata::function(pow_id, "ext_pow"));
        pow_fb.set_declaration();
        pow_fb.declare_ret(i32_ty, false);
        pow_fb.declare_arg(i32_ty, false);
        pow_fb.declare_arg(i32_ty, false);
        let entry = pow_fb.create_block();
        pow_fb.set_terminator(entry, Terminator::Unreachable);
        let pow_body = pow_fb.build();

        // -- i32 square(i32 %x)
        let square_id = ctx.fresh_def_id();
        let mut sq_fb = ctx.function_builder(TirBodyMetadata::function(square_id, "square"));
        sq_fb.declare_ret(i32_ty, false);
        let x = sq_fb.declare_arg(i32_ty, false);
        let call_dest = sq_fb.declare_local(i32_ty, true);

        let entry = sq_fb.create_block();
        let cont = sq_fb.create_block();

        // entry: _2 = ext_pow(_1, 2)
        let pow_op = ctx.fn_operand(pow_id, fn_ty);
        sq_fb.set_terminator(
            entry,
            Terminator::Call {
                func: pow_op,
                args: vec![Operand::use_local(x), ctx.const_i32(2)],
                destination: Place::from(call_dest),
                target: cont,
            },
        );

        // cont: _0 = _2; return
        sq_fb.push_statement(
            cont,
            Statement::assign(
                Place::from(RETURN_LOCAL),
                RValue::Operand(Operand::use_local(call_dest)),
            ),
        );
        sq_fb.set_terminator(cont, Terminator::Return);

        let sq_body = sq_fb.build();

        // -- Assemble module
        let mut unit = ctx.unit_builder("math_module");
        unit.add_body(pow_body);
        unit.add_body(sq_body);

        let tir_unit = unit.build();
        assert_eq!(tir_unit.bodies.len(), 2);
        assert!(tir_unit.bodies.raw[0].metadata.is_declaration);
        assert_eq!(tir_unit.bodies.raw[0].metadata.name, "ext_pow");
        assert!(!tir_unit.bodies.raw[1].metadata.is_declaration);
        assert_eq!(tir_unit.bodies.raw[1].metadata.name, "square");

        // Verify def_ids are sequential
        assert_eq!(tir_unit.bodies.raw[0].metadata.def_id, DefId(0));
        assert_eq!(tir_unit.bodies.raw[1].metadata.def_id, DefId(1));
    });
}
