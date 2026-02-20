use tidec_abi::{
    calling_convention::function::FnAbi,
    layout::TyAndLayout,
    size_and_align::{Align, Size},
};
use tidec_tir::{
    TirTy,
    alloc::{AllocId, Allocation, GlobalAlloc},
    body::{TirBody, TirBodyMetadata, TirUnit},
    ctx::TirCtx,
    syntax::{ConstScalar, Local, LocalData},
};
use tidec_utils::index_vec::IdxVec;

use crate::tir::{OperandRef, PlaceRef};

/// This trait is used to get the layout of a type.
/// It is used to get the layout of a type in the codegen backend.
pub trait LayoutOf<'ctx> {
    /// Returns the layout of the given type.
    fn layout_of(&self, ty: TirTy<'ctx>) -> TyAndLayout<'ctx, TirTy<'ctx>>;
}

/// This trait converts a TIR type to the backend's type representation.
///
/// This is needed in the generic codegen layer (e.g., for GEP instructions
/// in `codegen_place`) where the backend type is required but the specific
/// backend is not known.
pub trait BackendTypeOf<'ctx>: CodegenBackendTypes {
    /// Convert a TIR type to the corresponding backend type.
    ///
    /// # Panics
    ///
    /// Panics if the type cannot be represented as a backend value type
    /// (e.g., `TirTy::Unit` / void, which is not a value type).
    fn backend_type_of(&self, ty: TirTy<'ctx>) -> Self::Type;
}

pub trait FnAbiOf<'ctx> {
    /// Returns the function ABI for the given return type and argument types.
    fn fn_abi_of(&self, ret_and_args: &IdxVec<Local, LocalData<'ctx>>) -> FnAbi<'ctx, TirTy<'ctx>>;
}

/// This trait is used to define the types used in the codegen backend.
/// It is used to define the types used in the codegen backend.
// FIXME(bruzzone): when `trait alias` is stable, we can use it to alias the `CodegenObject` trait
// pub trait CodegenObject = Copy + PartialEq + std::fmt::Debug;
pub trait CodegenBackendTypes {
    /// A `BasicBlock` is a basic block in the codegen backend.
    type BasicBlock: Copy + PartialEq + std::fmt::Debug;
    /// A `Type` is a type in the codegen backend.
    type Type: Copy + PartialEq + std::fmt::Debug;
    /// A `Value` is an instance of a type in the codegen backend.
    /// E.g., an instruction, constant, argument, or a function value.
    type Value: Copy + PartialEq + std::fmt::Debug;
    /// A `Function` is a function type in the codegen backend.
    type FunctionType: Copy + PartialEq + std::fmt::Debug;
    /// A `FunctionValue` is a function value in the codegen backend.
    type FunctionValue: Copy + PartialEq + std::fmt::Debug;
    /// A `MetadataType` is a metadata type in the codegen backend.
    type MetadataType: Copy + PartialEq + std::fmt::Debug;
    /// A `MetadataValue` is a metadata value in the codegen backend.
    /// E.g., a debug info node or TBAA (Type-Based Alias Analysis) node.
    type MetadataValue: Copy + PartialEq + std::fmt::Debug + From<Self::Value>;
}

/// The codegen backend trait.
/// It is used to define the methods used in the codegen backend.
/// The associated types are used to define the types used in the codegen backend.
pub trait CodegenBackend: Sized + CodegenBackendTypes {
    /// The associated codegen module type.
    // FIXME(bruzzone): add constraints to ensure that the module is compatible with the codegen backend.
    type Module;

    /// The associated codegen context type.
    // FIXME(bruzzone): add constraints to ensure that the context is compatible with the codegen backend.
    type Context;
}

/// The pre-definition methods for the codegen backend. It is used to pre-define functions.
/// After pre-defining all functions, the bodies should be defined (see `DefineCodegenMethods`).
pub trait PreDefineCodegenMethods<'ctx>: Sized + CodegenBackendTypes {
    fn predefine_body(
        &self,
        lir_body_metadata: &TirBodyMetadata,
        lir_body_ret_and_args: &IdxVec<Local, LocalData<'ctx>>,
    );
}

/// The definition methods for the codegen backend. It is used to define (compile) function bodies.
/// The definition should be done after pre-defining all functions (see `PreDefineCodegenMethods`).
pub trait DefineCodegenMethods<'ctx>: Sized + CodegenBackendTypes {
    fn define_body(&self, lir_body: TirBody<'ctx>);
}

/// The codegen backend methods.
pub trait CodegenMethods<'ctx>:
    Sized
    + LayoutOf<'ctx>
    + FnAbiOf<'ctx>
    + BackendTypeOf<'ctx>
    + CodegenBackendTypes
    + CodegenBackend
    + PreDefineCodegenMethods<'ctx>
    + DefineCodegenMethods<'ctx>
{
    /// Return the TIR type context associated with this codegen context.
    fn tir_ctx(&self) -> TirCtx<'ctx>;

    /// Compile the given TIR unit.
    fn compile_tir_unit<'be, B: BuilderMethods<'be, 'ctx>>(&self, lir_unit: TirUnit<'ctx>);

    /// Emit the output of the codegen backend.
    /// This could be writing to a file ASM, object file, or JIT execution.
    /// The output format is backend-specific.
    fn emit_output(&self);

    /// Returns the function value for the given TIR body if it exists.
    fn get_fn(&self, lir_body_metadata: &TirBodyMetadata) -> Option<Self::FunctionValue>;

    /// Returns the function value for the given TIR body or defines it if it does not exist.
    fn get_or_define_fn(
        &self,
        lir_fn_metadata: &TirBodyMetadata,
        lir_fn_ret_and_args: &IdxVec<Local, LocalData<'ctx>>,
    ) -> Self::FunctionValue;

    /// Returns the function value for the given function name if it exists.
    fn get_fn_by_name(&self, name: &str) -> Option<Self::FunctionValue>;

    /// Get a global allocation by its ID.
    fn global_alloc(&self, alloc_id: AllocId) -> GlobalAlloc<'ctx>;

    /// Create a global constant from an allocation and return a pointer to it.
    fn const_alloc_to_value(&self, alloc: &Allocation) -> Self::Value;

    /// Get the function value for a function allocation.
    fn get_fn_from_alloc(&self, alloc_id: AllocId) -> Self::FunctionValue;
}

/// The builder methods for the codegen backend.
/// This trait is used to define the methods used in the codegen backend.
pub trait BuilderMethods<'a, 'ctx>: Sized + CodegenBackendTypes {
    /// The associated codegen context type.
    /// This ensures that the codegen context is compatible with the codegen backend types.
    type CodegenCtx: CodegenMethods<
            'ctx,
            BasicBlock = Self::BasicBlock,
            Type = Self::Type,
            Value = Self::Value,
            FunctionType = Self::FunctionType,
            FunctionValue = Self::FunctionValue,
            MetadataType = Self::MetadataType,
            MetadataValue = Self::MetadataValue,
        >;

    /// Returns a reference to the codegen context.
    fn ctx(&self) -> &Self::CodegenCtx;

    /// Allocate memory for a value of the given size and alignment.
    /// For instance, in LLVM this corresponds to the `alloca` instruction.
    fn alloca(&self, size: Size, align: Align) -> Self::Value;

    /// Create a new builder for the given codegen context and basic block.
    /// The builder is positioned at the end of the basic block.
    fn build(ctx: &'a Self::CodegenCtx, bb: Self::BasicBlock) -> Self;

    /// Append a new basic block to the given function value with the given name.
    /// The name can be empty, in which case a unique name will be generated.
    /// The function value is assumed to be valid and belong to the same context as the codegen context.
    fn append_basic_block(
        ctx: &'a Self::CodegenCtx,
        fn_value: Self::FunctionValue,
        name: &str,
    ) -> Self::BasicBlock;

    /// Build a return instruction for the given builder.
    /// If the return value is `None`, it means that the function returns `void`,
    /// the return value is ignored, or it is `Indirect` (see `PassMode` in `tidec_abi`).
    /// For instance, it could be `Indirect` if the return value is a large struct:
    /// ```rust
    /// struct LargeStruct { a: [u8; 1024] }
    /// fn foo() -> LargeStruct { unimplemented!() }
    /// ```
    fn build_return(&mut self, return_value: Option<Self::Value>);

    /// Load an operand from the given place reference.
    /// This is used to load a value from memory.
    fn load_operand(
        &mut self,
        place_ref: &PlaceRef<'ctx, Self::Value>,
    ) -> OperandRef<'ctx, Self::Value>;

    /// Build a floating-point negation instruction for the given value.
    fn build_fneg(&mut self, val: Self::Value) -> Self::Value;
    /// Build an integer negation instruction for the given value.
    fn build_neg(&self, val: Self::Value) -> Self::Value;
    /// Build a floating-point addition instruction for the given values.
    fn build_fadd(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build an integer addition instruction for the given values.
    fn build_add(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build a signed integer addition instruction for the given values, with undefined behavior on overflow.
    fn build_sadd_unchecked(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build an unsigned integer addition instruction for the given values, with undefined behavior on overflow.
    fn build_uadd_unchecked(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build a floating-point subtraction instruction for the given values.
    fn build_fsub(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build an integer subtraction instruction for the given values.
    fn build_sub(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build a signed integer subtraction instruction for the given values, with undefined behavior on overflow.
    fn build_ssub_unchecked(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build an unsigned integer subtraction instruction for the given values, with undefined behavior on overflow.
    fn build_usub_unchecked(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build a floating-point multiplication instruction for the given values.
    fn build_fmul(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build an integer multiplication instruction for the given values.
    fn build_mul(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build a signed integer multiplication instruction for the given values, with undefined behavior on overflow.
    fn build_smul_unchecked(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build an unsigned integer multiplication instruction for the given values, with undefined behavior on overflow.
    fn build_umul_unchecked(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build a floating-point division instruction for the given values.
    fn build_fdiv(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build a signed integer division instruction for the given values.
    fn build_sdiv(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build an unsigned integer division instruction for the given values.
    fn build_udiv(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;

    /// Build a signed integer remainder instruction for the given values.
    fn build_srem(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build an unsigned integer remainder instruction for the given values.
    fn build_urem(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build a floating-point remainder instruction for the given values.
    fn build_frem(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;

    /// Build a bitwise AND instruction for the given values.
    fn build_and(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build a bitwise OR instruction for the given values.
    fn build_or(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build a bitwise XOR instruction for the given values.
    fn build_xor(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;

    /// Build a left shift instruction for the given values.
    fn build_shl(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build a logical (unsigned) right shift instruction for the given values.
    fn build_lshr(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;
    /// Build an arithmetic (signed) right shift instruction for the given values.
    fn build_ashr(&mut self, lhs: Self::Value, rhs: Self::Value) -> Self::Value;

    /// Build a bitwise NOT instruction for the given value.
    ///
    /// For integers this is the bitwise complement (XOR with all-ones).
    /// For booleans this is a logical negation (XOR with `1`).
    fn build_not(&mut self, val: Self::Value) -> Self::Value;

    // ── Cast / conversion instructions ───────────────────────────

    /// Truncate an integer value to a narrower integer type.
    ///
    /// The destination type must be strictly narrower than the source.
    /// Maps to LLVM `trunc`.
    fn build_trunc(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Zero-extend an integer value to a wider integer type.
    ///
    /// The destination type must be strictly wider than the source.
    /// High bits are filled with zeros. Maps to LLVM `zext`.
    fn build_zext(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Sign-extend an integer value to a wider integer type.
    ///
    /// The destination type must be strictly wider than the source.
    /// High bits are filled with copies of the sign bit. Maps to LLVM `sext`.
    fn build_sext(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Truncate a floating-point value to a narrower float type.
    ///
    /// E.g. `f64` → `f32`. Maps to LLVM `fptrunc`.
    fn build_fptrunc(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Extend a floating-point value to a wider float type.
    ///
    /// E.g. `f32` → `f64`. Maps to LLVM `fpext`.
    fn build_fpext(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Convert a signed integer to a floating-point value.
    ///
    /// Maps to LLVM `sitofp`.
    fn build_sitofp(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Convert an unsigned integer to a floating-point value.
    ///
    /// Maps to LLVM `uitofp`.
    fn build_uitofp(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Convert a floating-point value to a signed integer.
    ///
    /// Maps to LLVM `fptosi`.
    fn build_fptosi(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Convert a floating-point value to an unsigned integer.
    ///
    /// Maps to LLVM `fptoui`.
    fn build_fptoui(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Convert an integer value to a pointer.
    ///
    /// Maps to LLVM `inttoptr`.
    fn build_inttoptr(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Convert a pointer to an integer value.
    ///
    /// Maps to LLVM `ptrtoint`.
    fn build_ptrtoint(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Reinterpret the bits of a value as a different type (same bit width).
    ///
    /// Maps to LLVM `bitcast`.
    fn build_bitcast(&mut self, val: Self::Value, dest_ty: Self::Type) -> Self::Value;

    /// Build a store instruction to store the given value to the given place reference.
    /// This is used to store a value to memory.
    /// The value is assumed to be of the same type as the place reference.
    /// The alignment is the alignment of the place reference.
    fn build_load(&mut self, ty: Self::Type, ptr: Self::Value, align: Align) -> Self::Value;

    /// Build a store instruction to write a value into memory at the given pointer.
    ///
    /// This is the counterpart of `build_load`. It writes the value `val` into the
    /// memory location pointed to by `ptr`, respecting the given alignment.
    ///
    /// # Parameters
    /// - `val`: The value to store.
    /// - `ptr`: The pointer to the memory location where the value will be stored.
    /// - `align`: The alignment of the memory location.
    fn build_store(&mut self, val: Self::Value, ptr: Self::Value, align: Align);

    /// Construct a backend value from a constant scalar and its TIR type.
    /// This is used to create constant values in the backend.
    ///
    /// For instance, in LLVM this could correspond to `LLVMConstInt` or `LLVMConstReal`.
    fn const_scalar_to_backend_value(
        &self,
        const_scalar: ConstScalar,
        ty_layout: TyAndLayout<TirTy<'ctx>>,
    ) -> Self::Value;

    /// Build a global constant from an allocation.
    /// This creates a global constant and returns a pointer to it.
    fn const_data_from_alloc(&mut self, alloc: &Allocation) -> Self::Value;

    /// Build a function call instruction.
    /// Returns the return value of the call (or a placeholder for void returns).
    fn build_call(
        &mut self,
        fn_value: Self::FunctionValue,
        args: &[Self::MetadataValue],
        name: &str,
    ) -> Option<Self::Value>;

    /// Build an unconditional branch to the given basic block.
    fn build_unconditional_br(&mut self, target: Self::BasicBlock);

    /// Build a conditional branch based on a boolean value.
    ///
    /// If `cond` is true (non-zero), execution continues at `then_bb`;
    /// otherwise it continues at `else_bb`.
    ///
    /// The `cond` value must be an `i1` (boolean) in the backend.
    fn build_conditional_br(
        &mut self,
        cond: Self::Value,
        then_bb: Self::BasicBlock,
        else_bb: Self::BasicBlock,
    );

    /// Build a multi-way switch instruction.
    ///
    /// The discriminant `discr` is compared against each `(value, block)` pair.
    /// If a match is found, control transfers to the corresponding block.
    /// Otherwise, control transfers to `otherwise`.
    fn build_switch(
        &mut self,
        discr: Self::Value,
        otherwise: Self::BasicBlock,
        cases: &[(u128, Self::BasicBlock)],
    );

    /// Build an `unreachable` instruction.
    ///
    /// This signals to the backend that control flow never reaches this
    /// point. If it does at runtime, the behaviour is undefined.
    fn build_unreachable(&mut self);

    /// Build an integer comparison instruction.
    ///
    /// Compares `lhs` and `rhs` using the given predicate string and returns
    /// an `i1` (boolean) result.
    ///
    /// The `signed` flag indicates whether the comparison is signed or unsigned,
    /// which matters for `Lt`, `Le`, `Gt`, `Ge` (but not for `Eq`/`Ne`).
    fn build_icmp(
        &mut self,
        op: tidec_tir::syntax::BinaryOp,
        lhs: Self::Value,
        rhs: Self::Value,
        signed: bool,
    ) -> Self::Value;

    /// Build a floating-point comparison instruction.
    ///
    /// Compares `lhs` and `rhs` using the given predicate and returns
    /// an `i1` (boolean) result. Uses ordered comparisons (NaN → false).
    fn build_fcmp(
        &mut self,
        op: tidec_tir::syntax::BinaryOp,
        lhs: Self::Value,
        rhs: Self::Value,
    ) -> Self::Value;

    /// Build a GEP (GetElementPtr) instruction for accessing a struct field.
    ///
    /// Given a pointer to a struct in memory, this computes the address of the
    /// field at `field_idx`. The `ty` parameter is the LLVM type of the struct.
    ///
    /// Returns a pointer to the field.
    fn build_struct_gep(
        &mut self,
        ty: Self::Type,
        ptr: Self::Value,
        field_idx: u32,
        name: &str,
    ) -> Self::Value;

    /// Convert a function value to a pointer value.
    /// This is used when passing functions as arguments or storing them.
    fn fn_to_ptr(&mut self, fn_value: Self::FunctionValue) -> Self::Value;
}
