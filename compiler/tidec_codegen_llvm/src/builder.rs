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

    fn fn_to_ptr(&mut self, fn_value: Self::FunctionValue) -> Self::Value {
        // In LLVM's opaque pointer model, function values can be used directly as pointers
        fn_value.as_global_value().as_pointer_value().into()
    }
}
