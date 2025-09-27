use tidec_abi::{
    calling_convention::function::FnAbi,
    layout::TyAndLayout,
    size_and_align::{Align, Size},
};
use tidec_lir::{
    lir::{LirBody, LirBodyMetadata, LirCtx, LirUnit},
    syntax::{ConstScalar, LirTy, Local, LocalData},
};
use tidec_utils::index_vec::IdxVec;

use crate::lir::{OperandRef, PlaceRef};

/// This trait is used to get the layout of a type.
/// It is used to get the layout of a type in the codegen backend.
pub trait LayoutOf {
    /// Returns the layout of the given type.
    fn layout_of(&self, ty: LirTy) -> TyAndLayout<LirTy>;
}

pub trait FnAbiOf {
    /// Returns the function ABI for the given return type and argument types.
    fn fn_abi_of(
        &self,
        lit_ty_ctx: &LirCtx,
        ret_and_args: &IdxVec<Local, LocalData>,
    ) -> FnAbi<LirTy>;
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
    type MetadataValue: Copy + PartialEq + std::fmt::Debug;
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
pub trait PreDefineCodegenMethods: Sized + CodegenBackendTypes {
    fn predefine_body(
        &self,
        lir_body_metadata: &LirBodyMetadata,
        lir_body_ret_and_args: &IdxVec<Local, LocalData>,
    );
}

/// The definition methods for the codegen backend. It is used to define (compile) function bodies.
/// The definition should be done after pre-defining all functions (see `PreDefineCodegenMethods`).
pub trait DefineCodegenMethods: Sized + CodegenBackendTypes {
    fn define_body(&self, lir_body: &LirBody);
}

/// The codegen backend methods.
pub trait CodegenMethods<'be>:
    Sized
    + LayoutOf
    + FnAbiOf
    + CodegenBackendTypes
    + CodegenBackend
    + PreDefineCodegenMethods
    + DefineCodegenMethods
{
    /// Creates a new codegen context for the given LIR type context and module.
    fn new(lir_ty_ctx: LirCtx, context: &'be Self::Context, module: Self::Module) -> Self;

    /// Return the LIR type context associated with this codegen context.
    fn lir_ctx(&self) -> &LirCtx;

    /// Compile the given LIR unit.
    fn compile_lir_unit<'a, B: BuilderMethods<'a, 'be>>(&self, lir_unit: LirUnit);

    /// Emit the output of the codegen backend.
    /// This could be writing to a file ASM, object file, or JIT execution.
    /// The output format is backend-specific.
    fn emit_output(&self);

    /// Returns the function value for the given LIR body if it exists.
    fn get_fn(&self, lir_body_metadata: &LirBodyMetadata) -> Option<Self::FunctionValue>;

    /// Returns the function value for the given LIR body or defines it if it does not exist.
    fn get_or_define_fn(
        &self,
        lir_fn_metadata: &LirBodyMetadata,
        lir_fn_ret_and_args: &IdxVec<Local, LocalData>,
    ) -> Self::FunctionValue;
}

/// The builder methods for the codegen backend.
/// This trait is used to define the methods used in the codegen backend.
pub trait BuilderMethods<'a, 'be>: Sized + CodegenBackendTypes {
    /// The associated codegen context type.
    /// This ensures that the codegen context is compatible with the codegen backend types.
    type CodegenCtx: CodegenMethods<
            'be,
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
    fn load_operand(&mut self, place_ref: &PlaceRef<Self::Value>) -> OperandRef<Self::Value>;

    /// Build a store instruction to store the given value to the given place reference.
    /// This is used to store a value to memory.
    /// The value is assumed to be of the same type as the place reference.
    /// The alignment is the alignment of the place reference.
    fn build_load(&mut self, ty: Self::Type, ptr: Self::Value, align: Align) -> Self::Value;

    /// Construct a backend value from a constant scalar and its LIR type.
    /// This is used to create constant values in the backend.
    ///
    /// For instance, in LLVM this could correspond to `LLVMConstInt` or `LLVMConstReal`.
    fn const_scalar_to_backend_value(
        &self,
        const_scalar: ConstScalar,
        ty_layout: TyAndLayout<LirTy>,
    ) -> Self::Value;
}
