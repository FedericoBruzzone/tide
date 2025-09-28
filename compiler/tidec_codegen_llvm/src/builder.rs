use std::ops::Deref;

use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue};
use inkwell::{basic_block::BasicBlock, builder::Builder};
use tidec_abi::layout::{BackendRepr, Primitive, TyAndLayout};
use tidec_abi::size_and_align::{Align, Size};
use tidec_codegen_ssa::tir::{OperandRef, PlaceRef};
use tidec_codegen_ssa::traits::{BuilderMethods, CodegenBackendTypes};
use tidec_tir::syntax::{ConstScalar, TirTy};
use tracing::instrument;

use crate::context::CodegenCtx;
use crate::tir::tir_ty::BasicTypesUtils;

/// A builder for generating LLVM IR code.
///
/// This struct wraps the `inkwell::builder::Builder` and provides
/// additional methods for code generation.
pub struct CodegenBuilder<'a, 'll> {
    pub ll_builder: Builder<'ll>,
    ctx: &'a CodegenCtx<'ll>,
}

impl<'ll> Deref for CodegenBuilder<'_, 'll> {
    type Target = CodegenCtx<'ll>;

    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}

impl<'ll> CodegenBackendTypes for CodegenBuilder<'_, 'll> {
    type BasicBlock = <CodegenCtx<'ll> as CodegenBackendTypes>::BasicBlock;
    type Type = <CodegenCtx<'ll> as CodegenBackendTypes>::Type;
    type Value = <CodegenCtx<'ll> as CodegenBackendTypes>::Value;
    type FunctionType = <CodegenCtx<'ll> as CodegenBackendTypes>::FunctionType;
    type FunctionValue = <CodegenCtx<'ll> as CodegenBackendTypes>::FunctionValue;
    type MetadataType = <CodegenCtx<'ll> as CodegenBackendTypes>::MetadataType;
    type MetadataValue = <CodegenCtx<'ll> as CodegenBackendTypes>::MetadataValue;
}

impl<'a, 'll> CodegenBuilder<'a, 'll> {
    #[instrument(skip(ctx))]
    pub fn with_ctx(ctx: &'a CodegenCtx<'ll>) -> Self {
        let ll_builder = ctx.ll_context.create_builder();
        CodegenBuilder { ll_builder, ctx }
    }
}

impl<'a, 'll> BuilderMethods<'a, 'll> for CodegenBuilder<'a, 'll> {
    type CodegenCtx = CodegenCtx<'ll>;

    fn ctx(&self) -> &Self::CodegenCtx {
        self.ctx
    }

    #[instrument(skip(ctx, llbb))]
    /// Create a new CodeGenBuilder from a CodeGenCtx and a BasicBlock.
    /// The builder is positioned at the end of the BasicBlock.
    fn build(ctx: &'a CodegenCtx<'ll>, llbb: BasicBlock) -> Self {
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
        ctx: &'a CodegenCtx<'ll>,
        fn_value: FunctionValue<'ll>,
        name: &str,
    ) -> BasicBlock<'ll> {
        ctx.ll_context.append_basic_block(fn_value, name)
    }

    #[instrument(level = "trace", skip(self))]
    fn load_operand(&mut self, place_ref: &PlaceRef<Self::Value>) -> OperandRef<Self::Value> {
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
        self.ll_builder.build_float_neg(val.into_float_value(), "fneg").unwrap().into()
    }

    fn build_neg(&self, val: Self::Value) -> Self::Value {
        assert!(val.get_type().is_int_type());
        self.ll_builder.build_int_neg(val.into_int_value(), "neg").unwrap().into()
    }

    fn const_scalar_to_backend_value(
        &self,
        const_scalar: ConstScalar,
        ty_layout: TyAndLayout<TirTy>,
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
}
