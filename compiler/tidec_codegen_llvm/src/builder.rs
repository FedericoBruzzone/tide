use std::ops::Deref;

use crate::context::CodegenCtx;
use crate::tir::tir_ty::BasicTypesUtils;
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, ValueKind};
use inkwell::{basic_block::BasicBlock, builder::Builder};
use tidec_abi::layout::{BackendRepr, Primitive, TyAndLayout};
use tidec_abi::size_and_align::{Align, Size};
use tidec_codegen_ssa::tir::{OperandRef, PlaceRef};
use tidec_codegen_ssa::traits::{BuilderMethods, CodegenBackendTypes};
use tidec_tir::alloc::Allocation;
use tidec_tir::syntax::ConstScalar;
use tidec_tir::TirTy;
use tracing::instrument;

/// Macro for generating arithmetic operation methods
macro_rules! impl_arithmetic_ops {
    // Integer operations
    (int, $method_name:ident, $llvm_method:ident, $op_name:literal, $doc:literal) => {
        #[doc = $doc]
        fn $method_name(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value {
            assert!(lhs.get_type().is_int_type() && rhs.get_type().is_int_type());
            self.ll_builder
                .$llvm_method(lhs.into_int_value(), rhs.into_int_value(), $op_name)
                .unwrap()
                .into()
        }
    };

    // Float operations
    (float, $method_name:ident, $llvm_method:ident, $op_name:literal, $doc:literal) => {
        #[doc = $doc]
        fn $method_name(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value {
            assert!(lhs.get_type().is_float_type() && rhs.get_type().is_float_type());
            self.ll_builder
                .$llvm_method(lhs.into_float_value(), rhs.into_float_value(), $op_name)
                .unwrap()
                .into()
        }
    };

    // Integer operations with overflow flags (nsw/nuw) and documentation
    (int_overflow, $method_name:ident, $llvm_method:ident, $op_name:literal, $doc:literal) => {
        #[doc = $doc]
        fn $method_name(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value {
            assert!(lhs.get_type().is_int_type() && rhs.get_type().is_int_type());
            self.ll_builder
                .$llvm_method(lhs.into_int_value(), rhs.into_int_value(), $op_name)
                .unwrap()
                .into()
        }
    };
}

/// A builder for generating LLVM IR code.
///
/// This struct wraps the `inkwell::builder::Builder` and provides
/// additional methods for code generation.
pub struct CodegenBuilder<'a, 'll, 'ctx> {
    pub ll_builder: Builder<'ll>,
    ctx: &'a CodegenCtx<'ctx, 'll>,
}

impl<'ll, 'ctx> Deref for CodegenBuilder<'_, 'll, 'ctx> {
    type Target = CodegenCtx<'ctx, 'll>;

    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}

impl<'ll, 'ctx> CodegenBackendTypes for CodegenBuilder<'_, 'll, 'ctx> {
    type BasicBlock = <CodegenCtx<'ctx, 'll> as CodegenBackendTypes>::BasicBlock;
    type Type = <CodegenCtx<'ctx, 'll> as CodegenBackendTypes>::Type;
    type Value = <CodegenCtx<'ctx, 'll> as CodegenBackendTypes>::Value;
    type FunctionType = <CodegenCtx<'ctx, 'll> as CodegenBackendTypes>::FunctionType;
    type FunctionValue = <CodegenCtx<'ctx, 'll> as CodegenBackendTypes>::FunctionValue;
    type MetadataType = <CodegenCtx<'ctx, 'll> as CodegenBackendTypes>::MetadataType;
    type MetadataValue = <CodegenCtx<'ctx, 'll> as CodegenBackendTypes>::MetadataValue;
}

impl<'a, 'll, 'ctx> CodegenBuilder<'a, 'll, 'ctx> {
    #[instrument(skip(ctx))]
    pub fn with_ctx(ctx: &'a CodegenCtx<'ctx, 'll>) -> Self {
        let ll_builder = ctx.ll_context.create_builder();
        CodegenBuilder { ll_builder, ctx }
    }
}

impl<'a, 'll, 'ctx> BuilderMethods<'a, 'ctx> for CodegenBuilder<'a, 'll, 'ctx> {
    type CodegenCtx = CodegenCtx<'ctx, 'll>;

    fn ctx(&self) -> &Self::CodegenCtx {
        self.ctx
    }

    #[instrument(skip(ctx, llbb))]
    /// Create a new CodeGenBuilder from a CodeGenCtx and a BasicBlock.
    /// The builder is positioned at the end of the BasicBlock.
    fn build(ctx: &'a CodegenCtx<'ctx, 'll>, llbb: BasicBlock) -> Self {
        let builder = CodegenBuilder::with_ctx(ctx);
        builder.ll_builder.position_at_end(llbb);
        builder
    }

    #[instrument(skip(self))]
    /// Allocate memory for a value of the given size and alignment.
    ///
    /// We do not track the first basic block, so the caller should ensure
    /// that the allocation is done at the beginning of the function.
    fn alloca(&self, size: Size, align: Align) -> Self::Value {
        let builder = self;
        let ty = self
            .ctx
            .ll_context
            .i8_type()
            .array_type(size.bytes() as u32);
        let name = ""; // Generate a unique name for the alloca

        match builder.ll_builder.build_alloca(ty, name) {
            Ok(pointer_value) => {
                if let Err(err) = pointer_value
                    .as_instruction()
                    .unwrap()
                    .set_alignment(align.bytes() as u32)
                {
                    panic!("Failed to set alignment: {}", err);
                }
                pointer_value.into()
            }
            Err(err) => {
                panic!("Failed to allocate memory: {}", err);
            }
        }
    }

    /// Append a new basic block to the function.
    ///
    /// # Panic
    ///
    /// Panics if the function is not a valid function value.
    fn append_basic_block(
        ctx: &'a CodegenCtx<'ctx, 'll>,
        fn_value: FunctionValue<'ll>,
        name: &str,
    ) -> BasicBlock<'ll> {
        ctx.ll_context.append_basic_block(fn_value, name)
    }

    #[instrument(level = "trace", skip(self))]
    fn load_operand(
        &mut self,
        place_ref: &PlaceRef<'ctx, Self::Value>,
    ) -> OperandRef<'ctx, Self::Value> {
        if place_ref.ty_layout.is_zst() {
            return OperandRef::new_zst(place_ref.ty_layout);
        }

        if place_ref.ty_layout.is_immediate() {
            let mut ll_global_const: Option<BasicValueEnum> = None;
            let llty = place_ref.ty_layout.ty.into_basic_type(self.ctx);

            // ```rust
            // unsafe {
            //     let llval = LLVMIsAGlobalVariable(place_ref.place_val.value.as_value_ref());
            //     if !llval.is_null() && LLVMIsGlobalConstant(llval) == LLVMBool::from(1) {
            //         let global_val = GlobalValue::new(llval);
            //         let loaded_val = global_val.get_initializer().unwrap();
            //         assert_eq!(loaded_val.get_type(), llty);
            //         ll_global_const = Some(loaded_val);
            //     }
            // }
            // ```
            let global_val = self
                .ll_module
                .get_global(place_ref.place_val.value.get_name().to_str().unwrap());
            if let Some(gv) = global_val {
                if gv.is_constant() {
                    let loaded_val = gv.get_initializer().unwrap();
                    assert_eq!(loaded_val.get_type(), llty);
                    ll_global_const = Some(loaded_val);
                }
            }

            let llval = ll_global_const.unwrap_or_else(|| {
                // TODO: Here we should call:
                //
                // 1) scalar_load_metadata(...)
                // Attaches LLVM metadata to the load instruction (the one that just pulled load from memory).
                // This metadata guides LLVM optimizations and correctness:
                // e.g. alignment info, nonnull if it’s a pointer, range for integers, noalias hints, etc.
                // So if you load an &T, the compiler may add metadata saying “this pointer is non-null”.
                //
                // 2) self.to_immediate_scalar(load, scalar)
                // Converts the loaded LLVM value (load) into an immediate scalar representation in Tide’s codegen world.
                // Why? Because some scalars (e.g., booleans) need normalization: Tide booleans are guaranteed to be 0 or 1,
                // but LLVM might treat them as any non-zero integer. to_immediate_scalar ensures consistency with Tide’s semantics.
                self.build_load(llty, place_ref.place_val.value, place_ref.place_val.align)
            });

            OperandRef::new_immediate(llval, place_ref.ty_layout)
        } else {
            todo!("Handle non-immediate types — when the layout is, for example, `Memory`");
        }
    }

    /// Build a return instruction for the given builder.
    /// If the return value is `None`, it means that the function returns `void`,
    /// otherwise it returns the given value.
    fn build_return(&mut self, ret_val: Option<Self::Value>) {
        match ret_val {
            None => {
                let _ = self.ll_builder.build_return(None);
            }
            Some(val) => {
                let _ = self.ll_builder.build_return(Some(&val));
            }
        }
    }

    /// Build a load instruction to load a value from the given pointer. It also creates
    /// a new variable to hold the loaded value.
    fn build_load(&mut self, ty: Self::Type, ptr: Self::Value, align: Align) -> Self::Value {
        let load_inst = match self.ll_builder.build_load(ty, ptr.into_pointer_value(), "") {
            Ok(v) => v,
            Err(err) => panic!("Failed to build load instruction: {}", err),
        };

        load_inst
            .as_instruction_value()
            .unwrap()
            .set_alignment(align.bytes() as u32)
            .expect("Failed to set alignment");

        load_inst
    }

    /// Build a store instruction to write a value into memory at the given pointer.
    ///
    /// Emits an LLVM `store` instruction with the specified alignment. The pointer
    /// must point to a memory location large enough to hold the value.
    fn build_store(&mut self, val: Self::Value, ptr: Self::Value, align: Align) {
        let store_inst = self
            .ll_builder
            .build_store(ptr.into_pointer_value(), val)
            .expect("Failed to build store instruction");

        store_inst
            .set_alignment(align.bytes() as u32)
            .expect("Failed to set alignment on store");
    }

    fn build_fneg(&mut self, val: Self::Value) -> Self::Value {
        assert!(val.get_type().is_float_type());
        self.ll_builder
            .build_float_neg(val.into_float_value(), "fneg")
            .unwrap()
            .into()
    }

    fn build_neg(&self, val: Self::Value) -> Self::Value {
        assert!(val.get_type().is_int_type());
        self.ll_builder
            .build_int_neg(val.into_int_value(), "neg")
            .unwrap()
            .into()
    }

    // Basic integer arithmetic operations
    impl_arithmetic_ops!(int, build_add, build_int_add, "add",
        "Integer addition.\n\n`build_int_add` is a helper on an LLVM IR builder wrapper that generates an integer addition instruction.");
    impl_arithmetic_ops!(int, build_sub, build_int_sub, "sub",
        "Integer subtraction.\n\n`build_int_sub` is a helper on an LLVM IR builder wrapper that generates an integer subtraction instruction.");
    impl_arithmetic_ops!(int, build_mul, build_int_mul, "mul",
        "Integer multiplication.\n\n`build_int_mul` is a helper on an LLVM IR builder wrapper that generates an integer multiplication instruction.");
    impl_arithmetic_ops!(int, build_sdiv, build_int_signed_div, "sdiv",
        "Signed integer division.\n\n`build_int_signed_div` is a helper on an LLVM IR builder wrapper that generates a signed integer division instruction.");
    impl_arithmetic_ops!(int, build_udiv, build_int_unsigned_div, "udiv",
        "Unsigned integer division.\n\n`build_int_unsigned_div` is a helper on an LLVM IR builder wrapper that generates an unsigned integer division instruction.");

    // Remainder / modulo operations
    impl_arithmetic_ops!(int, build_srem, build_int_signed_rem, "srem",
        "Signed integer remainder.\n\n`build_int_signed_rem` is a helper on an LLVM IR builder wrapper that generates a signed integer remainder instruction.");
    impl_arithmetic_ops!(int, build_urem, build_int_unsigned_rem, "urem",
        "Unsigned integer remainder.\n\n`build_int_unsigned_rem` is a helper on an LLVM IR builder wrapper that generates an unsigned integer remainder instruction.");
    impl_arithmetic_ops!(float, build_frem, build_float_rem, "frem",
        "Floating-point remainder.\n\n`build_float_rem` is a helper on an LLVM IR builder wrapper that generates a floating-point remainder instruction.");

    // Bitwise operations
    impl_arithmetic_ops!(int, build_and, build_and, "and",
        "Bitwise AND.\n\n`build_and` is a helper on an LLVM IR builder wrapper that generates a bitwise AND instruction.");
    impl_arithmetic_ops!(int, build_or, build_or, "or",
        "Bitwise OR.\n\n`build_or` is a helper on an LLVM IR builder wrapper that generates a bitwise OR instruction.");
    impl_arithmetic_ops!(int, build_xor, build_xor, "xor",
        "Bitwise XOR.\n\n`build_xor` is a helper on an LLVM IR builder wrapper that generates a bitwise XOR instruction.");

    // Shift operations
    impl_arithmetic_ops!(int, build_shl, build_left_shift, "shl",
        "Left shift.\n\n`build_left_shift` is a helper on an LLVM IR builder wrapper that generates a left shift instruction.");

    /// Logical (unsigned) right shift.
    ///
    /// Uses `build_right_shift` with `sign_extend = false` to produce a
    /// logical shift that fills vacated bits with zeros.
    fn build_lshr(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value {
        assert!(lhs.get_type().is_int_type() && rhs.get_type().is_int_type());
        self.ll_builder
            .build_right_shift(
                lhs.into_int_value(),
                rhs.into_int_value(),
                false, // sign_extend = false → logical shift
                "lshr",
            )
            .unwrap()
            .into()
    }

    /// Arithmetic (signed) right shift.
    ///
    /// Uses `build_right_shift` with `sign_extend = true` to produce an
    /// arithmetic shift that preserves the sign bit.
    fn build_ashr(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value {
        assert!(lhs.get_type().is_int_type() && rhs.get_type().is_int_type());
        self.ll_builder
            .build_right_shift(
                lhs.into_int_value(),
                rhs.into_int_value(),
                true, // sign_extend = true → arithmetic shift
                "ashr",
            )
            .unwrap()
            .into()
    }

    /// Bitwise NOT (complement).
    ///
    /// Emits an LLVM `xor %val, -1` instruction, which flips every bit.
    fn build_not(&mut self, val: Self::Value) -> Self::Value {
        assert!(val.get_type().is_int_type());
        self.ll_builder
            .build_not(val.into_int_value(), "not")
            .unwrap()
            .into()
    }

    // ── Cast / conversion instructions ───────────────────────────

    /// Truncate an integer to a narrower integer type.
    fn build_trunc(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_int_truncate(val.into_int_value(), dest_ty.into_int_type(), "trunc")
            .unwrap()
            .into()
    }

    /// Zero-extend an integer to a wider integer type.
    fn build_zext(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_int_z_extend(val.into_int_value(), dest_ty.into_int_type(), "zext")
            .unwrap()
            .into()
    }

    /// Sign-extend an integer to a wider integer type.
    fn build_sext(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_int_s_extend(val.into_int_value(), dest_ty.into_int_type(), "sext")
            .unwrap()
            .into()
    }

    /// Truncate a float to a narrower float type.
    fn build_fptrunc(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_float_trunc(val.into_float_value(), dest_ty.into_float_type(), "fptrunc")
            .unwrap()
            .into()
    }

    /// Extend a float to a wider float type.
    fn build_fpext(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_float_ext(val.into_float_value(), dest_ty.into_float_type(), "fpext")
            .unwrap()
            .into()
    }

    /// Signed integer → float.
    fn build_sitofp(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_signed_int_to_float(val.into_int_value(), dest_ty.into_float_type(), "sitofp")
            .unwrap()
            .into()
    }

    /// Unsigned integer → float.
    fn build_uitofp(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_unsigned_int_to_float(val.into_int_value(), dest_ty.into_float_type(), "uitofp")
            .unwrap()
            .into()
    }

    /// Float → signed integer.
    fn build_fptosi(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_float_to_signed_int(val.into_float_value(), dest_ty.into_int_type(), "fptosi")
            .unwrap()
            .into()
    }

    /// Float → unsigned integer.
    fn build_fptoui(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_float_to_unsigned_int(val.into_float_value(), dest_ty.into_int_type(), "fptoui")
            .unwrap()
            .into()
    }

    /// Integer → pointer.
    fn build_inttoptr(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_int_to_ptr(
                val.into_int_value(),
                dest_ty.into_pointer_type(),
                "inttoptr",
            )
            .unwrap()
            .into()
    }

    /// Pointer → integer.
    fn build_ptrtoint(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_ptr_to_int(
                val.into_pointer_value(),
                dest_ty.into_int_type(),
                "ptrtoint",
            )
            .unwrap()
            .into()
    }

    /// Bitcast (reinterpret bits).
    fn build_bitcast(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value {
        self.ll_builder
            .build_bit_cast(val, dest_ty, "bitcast")
            .unwrap()
    }

    // Float arithmetic operations
    impl_arithmetic_ops!(float, build_fadd, build_float_add, "fadd",
        "Floating-point addition.\n\n`build_float_add` is a helper on an LLVM IR builder wrapper that generates a floating-point addition instruction.");
    impl_arithmetic_ops!(float, build_fsub, build_float_sub, "fsub",
        "Floating-point subtraction.\n\n`build_float_sub` is a helper on an LLVM IR builder wrapper that generates a floating-point subtraction instruction.");
    impl_arithmetic_ops!(float, build_fmul, build_float_mul, "fmul",
        "Floating-point multiplication.\n\n`build_float_mul` is a helper on an LLVM IR builder wrapper that generates a floating-point multiplication instruction.");
    impl_arithmetic_ops!(float, build_fdiv, build_float_div, "fdiv",
        "Floating-point division.\n\n`build_float_div` is a helper on an LLVM IR builder wrapper that generates a floating-point division instruction.");

    impl_arithmetic_ops!(int_overflow, build_sadd_unchecked, build_int_nsw_add, "sadd",
        "Signed addition with UB on overflow.\n\n`build_int_nsw_add` is a helper on an LLVM IR builder wrapper that generates a signed integer addition instruction with the nsw flag, ensuring the operation is UB (undefined behavior) if signed overflow occurs.");
    impl_arithmetic_ops!(int_overflow, build_uadd_unchecked, build_int_nuw_add, "uadd",
        "Unsigned addition with UB on overflow.\n\n`build_int_nuw_add` is a helper on an LLVM IR builder wrapper that generates an unsigned integer addition instruction with the nuw flag, ensuring the operation is UB (undefined behavior) if unsigned overflow occurs.");
    impl_arithmetic_ops!(int_overflow, build_ssub_unchecked, build_int_nsw_sub, "ssub",
        "Signed subtraction with UB on overflow.\n\n`build_int_nsw_sub` is a helper on an LLVM IR builder wrapper that generates a signed integer subtraction instruction with the nsw flag, ensuring the operation is UB (undefined behavior) if signed overflow occurs.");
    impl_arithmetic_ops!(int_overflow, build_usub_unchecked, build_int_nuw_sub, "usub",
        "Unsigned subtraction with UB on overflow.\n\n`build_int_nuw_sub` is a helper on an LLVM IR builder wrapper that generates an unsigned integer subtraction instruction with the nuw flag, ensuring the operation is UB (undefined behavior) if unsigned overflow occurs.");
    impl_arithmetic_ops!(int_overflow, build_smul_unchecked, build_int_nsw_mul, "smul",
        "Signed multiplication with UB on overflow.\n\n`build_int_nsw_mul` is a helper on an LLVM IR builder wrapper that generates a signed integer multiplication instruction with the nsw flag, ensuring the operation is UB (undefined behavior) if signed overflow occurs.");
    impl_arithmetic_ops!(int_overflow, build_umul_unchecked, build_int_nuw_mul, "umul",
        "Unsigned multiplication with UB on overflow.\n\n`build_int_nuw_mul` is a helper on an LLVM IR builder wrapper that generates an unsigned integer multiplication instruction with the nuw flag, ensuring the operation is UB (undefined behavior) if unsigned overflow occurs.");

    fn const_scalar_to_backend_value(
        &self,
        const_scalar: ConstScalar,
        ty_layout: TyAndLayout<TirTy<'ctx>>,
    ) -> Self::Value {
        assert!(matches!(ty_layout.backend_repr, BackendRepr::Scalar(_)));
        let llty = ty_layout.ty.into_basic_type(self.ctx);
        let be_repr = ty_layout.backend_repr.to_primitive();
        let bitsize = if ty_layout.is_bool() {
            1
        } else {
            ty_layout.size.bits()
        };

        match const_scalar {
            /* TODO: ConstScalar::Ptr(...) */
            ConstScalar::Value(raw_scalar_value) => {
                let bits = raw_scalar_value.to_bits(ty_layout.size);
                // Create an LLVM integer type with the appropriate bit size.
                // Here, even if the llty is a pointer, we create an integer type to hold the bits
                // and then cast it to a pointer if needed. The same applies to floats.
                //
                // Note that, we cannot create an integer of size 128 (e.g., `self.ctx().ll_context.i128_type()`)
                // because it cannot be cast to a float 32; LLVM rejects such casts because of
                // "invalid cast opcode". Consequently, the `llval.const_truncate_or_bit_cast(llty.into_int_type()).into()` method
                // also would fail due to llty being a float type.
                let base_int = self.ctx.ll_context.custom_width_int_type(bitsize as u32);

                // Split the 128-bit integer into two 64-bit words for LLVM
                let words = [(bits & u64::MAX as u128) as u64, (bits >> 64) as u64];
                let llval = base_int.const_int_arbitrary_precision(&words);

                if let Primitive::Pointer(_) = be_repr {
                    llval.const_to_pointer(llty.into_pointer_type()).into()
                } else {
                    self.ll_builder.build_bit_cast(llval, llty, "").unwrap()
                }
            }
        }
    }

    fn const_data_from_alloc(&mut self, alloc: &Allocation) -> Self::Value {
        // Create a global constant from the allocation bytes
        let bytes = alloc.bytes();
        let i8_type = self.ctx.ll_context.i8_type();
        let array_type = i8_type.array_type(bytes.len() as u32);

        // Create constant values for each byte
        let byte_values: Vec<_> = bytes
            .iter()
            .map(|&b| i8_type.const_int(b as u64, false))
            .collect();
        let const_array = i8_type.const_array(&byte_values);

        // Create a global variable with the constant array
        let global = self
            .ctx
            .ll_module
            .add_global(array_type, None, "const_data");
        global.set_initializer(&const_array);
        global.set_constant(true);
        global.set_linkage(inkwell::module::Linkage::Private);
        global.set_unnamed_addr(true);

        // Return a pointer to the global
        global.as_pointer_value().into()
    }

    fn build_call(
        &mut self,
        fn_value: Self::FunctionValue,
        args: &[Self::MetadataValue],
        name: &str,
    ) -> Option<Self::Value> {
        let call_site = self
            .ll_builder
            .build_call(fn_value, args, name)
            .expect("Failed to build call instruction");

        // Try to get the return value. If the function returns void, this will be None.
        // inkwell returns a ValueKind enum with Basic/Instruction variants
        match call_site.try_as_basic_value() {
            ValueKind::Basic(val) => Some(val),
            ValueKind::Instruction(_) => None,
        }
    }

    fn build_unconditional_br(&mut self, target: Self::BasicBlock) {
        self.ll_builder
            .build_unconditional_branch(target)
            .expect("Failed to build unconditional branch");
    }

    fn build_conditional_br(
        &mut self,
        cond: Self::Value,
        then_bb: Self::BasicBlock,
        else_bb: Self::BasicBlock,
    ) {
        self.ll_builder
            .build_conditional_branch(cond.into_int_value(), then_bb, else_bb)
            .expect("Failed to build conditional branch");
    }

    fn build_switch(
        &mut self,
        discr: Self::Value,
        otherwise: Self::BasicBlock,
        cases: &[(u128, Self::BasicBlock)],
    ) {
        let int_val = discr.into_int_value();
        let int_ty = int_val.get_type();
        let ll_cases: Vec<_> = cases
            .iter()
            .map(|&(val, bb)| (int_ty.const_int(val as u64, false), bb))
            .collect();
        self.ll_builder
            .build_switch(int_val, otherwise, &ll_cases)
            .expect("Failed to build switch instruction");
    }

    fn build_unreachable(&mut self) {
        self.ll_builder
            .build_unreachable()
            .expect("Failed to build unreachable");
    }

    fn build_icmp(
        &mut self,
        op: tidec_tir::syntax::BinaryOp,
        lhs: Self::Value,
        rhs: Self::Value,
        signed: bool,
    ) -> Self::Value {
        use inkwell::IntPredicate;
        use tidec_tir::syntax::BinaryOp;

        let pred = match op {
            BinaryOp::Eq => IntPredicate::EQ,
            BinaryOp::Ne => IntPredicate::NE,
            BinaryOp::Lt => {
                if signed {
                    IntPredicate::SLT
                } else {
                    IntPredicate::ULT
                }
            }
            BinaryOp::Le => {
                if signed {
                    IntPredicate::SLE
                } else {
                    IntPredicate::ULE
                }
            }
            BinaryOp::Gt => {
                if signed {
                    IntPredicate::SGT
                } else {
                    IntPredicate::UGT
                }
            }
            BinaryOp::Ge => {
                if signed {
                    IntPredicate::SGE
                } else {
                    IntPredicate::UGE
                }
            }
            _ => panic!("build_icmp called with non-comparison op: {:?}", op),
        };

        self.ll_builder
            .build_int_compare(pred, lhs.into_int_value(), rhs.into_int_value(), "icmp")
            .expect("Failed to build integer comparison")
            .into()
    }

    fn build_fcmp(
        &mut self,
        op: tidec_tir::syntax::BinaryOp,
        lhs: Self::Value,
        rhs: Self::Value,
    ) -> Self::Value {
        use inkwell::FloatPredicate;
        use tidec_tir::syntax::BinaryOp;

        let pred = match op {
            BinaryOp::Eq => FloatPredicate::OEQ,
            BinaryOp::Ne => FloatPredicate::ONE,
            BinaryOp::Lt => FloatPredicate::OLT,
            BinaryOp::Le => FloatPredicate::OLE,
            BinaryOp::Gt => FloatPredicate::OGT,
            BinaryOp::Ge => FloatPredicate::OGE,
            _ => panic!("build_fcmp called with non-comparison op: {:?}", op),
        };

        self.ll_builder
            .build_float_compare(pred, lhs.into_float_value(), rhs.into_float_value(), "fcmp")
            .expect("Failed to build float comparison")
            .into()
    }

    /// Build a GEP (GetElementPtr) instruction for accessing a struct field.
    ///
    /// Emits an LLVM `getelementptr inbounds` instruction to compute the
    /// address of a struct field. The `ty` is the LLVM struct type, `ptr` points
    /// to the struct in memory, and `field_idx` is the zero-based field index.
    fn build_struct_gep(
        &mut self,
        ty: Self::Type,
        ptr: Self::Value,
        field_idx: u32,
        name: &str,
    ) -> Self::Value {
        self.ll_builder
            .build_struct_gep(ty, ptr.into_pointer_value(), field_idx, name)
            .expect("Failed to build struct GEP")
            .into()
    }

    fn fn_to_ptr(&mut self, fn_value: Self::FunctionValue) -> Self::Value {
        // In LLVM's opaque pointer model, function values can be used directly as pointers
        fn_value.as_global_value().as_pointer_value().into()
    }
}
