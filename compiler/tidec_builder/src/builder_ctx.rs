//! Builder context for ergonomic TIR construction.
//!
//! [`BuilderCtx`] encapsulates the complexity of arena allocation and interning,
//! providing a clean API for creating types, allocations, and other TIR entities
//! without exposing the underlying interning machinery.
//!
//! # Example
//!
//! ```rust,ignore
//! use tidec_builder::BuilderCtx;
//!
//! BuilderCtx::with_default(|ctx| {
//!     // Create types without manual interning
//!     let i32_ty = ctx.i32();
//!     let f64_ty = ctx.f64();
//!     let ptr_ty = ctx.ptr_imm(i32_ty);
//!     let struct_ty = ctx.struct_ty(&[i32_ty, f64_ty], false);
//!
//!     // Intern allocations
//!     let str_alloc = ctx.intern_c_str("hello");
//!     let bytes_alloc = ctx.intern_bytes(&[1, 2, 3, 4]);
//!
//!     // Use with builders
//!     let mut fb = ctx.function_builder(metadata);
//!     fb.declare_ret(i32_ty, false);
//!     // ...
//! });
//! ```

use tidec_abi::layout::TyAndLayout;
use tidec_abi::target::{BackendKind, TirTarget};
use tidec_tir::alloc::AllocId;
use tidec_tir::body::{DefId, GlobalId};
use tidec_tir::ctx::{EmitKind, InternCtx, TirArena, TirArgs, TirCtx};
use tidec_tir::ty::{self, Mutability};
use tidec_tir::{TirAllocation, TirTy, TirTypeList};

use crate::{FunctionBuilder, UnitBuilder};
use tidec_tir::body::TirBodyMetadata;

/// A builder context that manages TIR construction with automatic interning.
///
/// This struct encapsulates the arena, interning context, and TIR context,
/// providing ergonomic methods for creating types and interning allocations
/// without exposing the underlying complexity to the user.
///
/// # Lifetime
///
/// The `'ctx` lifetime parameter represents the lifetime of the arena and all
/// interned values. All types and allocations created through this context
/// live for `'ctx`.
pub struct BuilderCtx<'ctx> {
    ctx: TirCtx<'ctx>,
}

impl<'ctx> BuilderCtx<'ctx> {
    /// Create a new `BuilderCtx` from existing components.
    ///
    /// For most use cases, prefer [`with_default`](Self::with_default) or
    /// [`with_target`](Self::with_target) which handle arena setup automatically.
    pub fn new(ctx: TirCtx<'ctx>) -> Self {
        Self { ctx }
    }

    /// Run a closure with a default `BuilderCtx`.
    ///
    /// This is the simplest way to use the builder API. It creates an arena
    /// and context with default settings (LLVM backend, object emission).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// BuilderCtx::with_default(|ctx| {
    ///     let i32_ty = ctx.i32();
    ///     // ... build your TIR ...
    /// });
    /// ```
    pub fn with_default<F, R>(f: F) -> R
    where
        F: for<'a> FnOnce(BuilderCtx<'a>) -> R,
    {
        Self::with_target(BackendKind::Llvm, EmitKind::Object, f)
    }

    /// Run a closure with a `BuilderCtx` configured for the specified target.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// BuilderCtx::with_target(BackendKind::Llvm, EmitKind::LlvmIr, |ctx| {
    ///     let i32_ty = ctx.i32();
    ///     // ... build your TIR ...
    /// });
    /// ```
    pub fn with_target<F, R>(backend: BackendKind, emit: EmitKind, f: F) -> R
    where
        F: for<'a> FnOnce(BuilderCtx<'a>) -> R,
    {
        let target = TirTarget::new(backend);
        let args = TirArgs { emit_kind: emit };
        let arena = TirArena::default();
        let intern_ctx = InternCtx::new(&arena);
        let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);
        let builder_ctx = BuilderCtx::new(tir_ctx);
        f(builder_ctx)
    }

    /// Returns a reference to the underlying `TirCtx`.
    ///
    /// This can be used when you need direct access to the TIR context,
    /// for example to compute layouts or access target information.
    pub fn tir_ctx(&self) -> &TirCtx<'ctx> {
        &self.ctx
    }

    // =========================================================================
    // Primitive types
    // =========================================================================

    /// Create the unit type `()`.
    pub fn unit(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::Unit)
    }

    /// Create the boolean type.
    pub fn bool(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::Bool)
    }

    /// Create the `i8` type.
    pub fn i8(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::I8)
    }

    /// Create the `i16` type.
    pub fn i16(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::I16)
    }

    /// Create the `i32` type.
    pub fn i32(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::I32)
    }

    /// Create the `i64` type.
    pub fn i64(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::I64)
    }

    /// Create the `i128` type.
    pub fn i128(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::I128)
    }

    /// Create the `u8` type.
    pub fn u8(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::U8)
    }

    /// Create the `u16` type.
    pub fn u16(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::U16)
    }

    /// Create the `u32` type.
    pub fn u32(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::U32)
    }

    /// Create the `u64` type.
    pub fn u64(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::U64)
    }

    /// Create the `u128` type.
    pub fn u128(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::U128)
    }

    /// Create the `f16` type.
    pub fn f16(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::F16)
    }

    /// Create the `f32` type.
    pub fn f32(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::F32)
    }

    /// Create the `f64` type.
    pub fn f64(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::F64)
    }

    /// Create the `f128` type.
    pub fn f128(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::F128)
    }

    /// Create the metadata type.
    pub fn metadata(&self) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::Metadata)
    }

    // =========================================================================
    // Composite types
    // =========================================================================

    /// Create an immutable raw pointer type (`*imm T`).
    pub fn ptr_imm(&self, pointee: TirTy<'ctx>) -> TirTy<'ctx> {
        self.ctx
            .intern_ty(ty::TirTy::RawPtr(pointee, Mutability::Imm))
    }

    /// Create a mutable raw pointer type (`*mut T`).
    pub fn ptr_mut(&self, pointee: TirTy<'ctx>) -> TirTy<'ctx> {
        self.ctx
            .intern_ty(ty::TirTy::RawPtr(pointee, Mutability::Mut))
    }

    /// Create a raw pointer type with explicit mutability.
    pub fn ptr(&self, pointee: TirTy<'ctx>, mutability: Mutability) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::RawPtr(pointee, mutability))
    }

    /// Create a struct type from field types.
    ///
    /// # Arguments
    ///
    /// * `fields` - The types of the struct fields.
    /// * `packed` - If `true`, the struct uses packed layout (no alignment padding).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pair_ty = ctx.struct_ty(&[ctx.i32(), ctx.f64()], false);
    /// ```
    pub fn struct_ty(&self, fields: &[TirTy<'ctx>], packed: bool) -> TirTy<'ctx> {
        let fields = self.ctx.intern_type_list(fields);
        self.ctx.intern_ty(ty::TirTy::Struct { fields, packed })
    }

    /// Create a fixed-size array type.
    ///
    /// # Arguments
    ///
    /// * `element` - The element type.
    /// * `len` - The number of elements.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let arr_ty = ctx.array(ctx.i32(), 10); // [i32; 10]
    /// ```
    pub fn array(&self, element: TirTy<'ctx>, len: u64) -> TirTy<'ctx> {
        self.ctx.intern_ty(ty::TirTy::Array(element, len))
    }

    // =========================================================================
    // Type list interning
    // =========================================================================

    /// Intern a list of types.
    ///
    /// This is useful for function signatures or other contexts where you need
    /// a type list but not necessarily a struct type.
    pub fn type_list(&self, types: &[TirTy<'ctx>]) -> TirTypeList<'ctx> {
        self.ctx.intern_type_list(types)
    }

    // =========================================================================
    // Allocation interning
    // =========================================================================

    /// Intern a C-string (null-terminated) and register it as a memory allocation.
    ///
    /// Returns the `AllocId` that can be used to reference this string in the TIR.
    pub fn intern_c_str(&self, s: &str) -> AllocId {
        self.ctx.intern_c_str(s)
    }

    /// Intern raw bytes and register them as a memory allocation.
    ///
    /// Returns the `AllocId` that can be used to reference these bytes in the TIR.
    pub fn intern_bytes(&self, bytes: &[u8]) -> AllocId {
        self.ctx.intern_bytes(bytes)
    }

    /// Register a function as a global allocation.
    ///
    /// Returns the `AllocId` for the function reference.
    pub fn intern_fn(&self, def_id: DefId) -> AllocId {
        self.ctx.intern_fn(def_id)
    }

    /// Register a global variable (static) as a global allocation.
    ///
    /// Returns the `AllocId` that can be used to reference this global.
    pub fn intern_static(&self, global_id: GlobalId) -> AllocId {
        self.ctx.intern_static(global_id)
    }

    /// Intern an allocation directly.
    ///
    /// For most use cases, prefer [`intern_c_str`](Self::intern_c_str) or
    /// [`intern_bytes`](Self::intern_bytes).
    pub fn intern_alloc(&self, alloc: tidec_tir::alloc::Allocation) -> TirAllocation<'ctx> {
        self.ctx.intern_alloc(alloc)
    }

    // =========================================================================
    // Builder factory methods
    // =========================================================================

    /// Create a new function builder with the given metadata.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut fb = ctx.function_builder(metadata);
    /// fb.declare_ret(ctx.i32(), false);
    /// fb.declare_arg(ctx.i32(), false);
    /// let entry = fb.create_block();
    /// // ... build the function body ...
    /// let body = fb.build();
    /// ```
    pub fn function_builder(&self, metadata: TirBodyMetadata) -> FunctionBuilder<'ctx> {
        FunctionBuilder::new(metadata)
    }

    /// Create a new unit (module) builder with the given name.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut unit = ctx.unit_builder("my_module");
    /// unit.add_body(my_function_body);
    /// let tir_unit = unit.build();
    /// ```
    pub fn unit_builder(&self, name: impl Into<String>) -> UnitBuilder<'ctx> {
        UnitBuilder::new(name)
    }

    // =========================================================================
    // Target and layout information
    // =========================================================================

    /// Returns the target configuration.
    pub fn target(&self) -> &TirTarget {
        self.ctx.target()
    }

    /// Returns the backend kind (e.g., LLVM).
    pub fn backend_kind(&self) -> &BackendKind {
        self.ctx.backend_kind()
    }

    /// Returns the emit kind (e.g., Object, Assembly, LLVM IR).
    pub fn emit_kind(&self) -> &EmitKind {
        self.ctx.emit_kind()
    }

    /// Compute the layout of a type.
    ///
    /// This is useful for determining sizes, alignments, and field offsets.
    pub fn layout_of(&self, ty: TirTy<'ctx>) -> TyAndLayout<'ctx, TirTy<'ctx>> {
        self.ctx.layout_of(ty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_types_are_interned() {
        BuilderCtx::with_default(|ctx| {
            let i32_a = ctx.i32();
            let i32_b = ctx.i32();
            assert_eq!(i32_a, i32_b, "Same primitive type should be deduplicated");

            let f64_a = ctx.f64();
            let f64_b = ctx.f64();
            assert_eq!(f64_a, f64_b);

            assert_ne!(i32_a, f64_a, "Different types should not be equal");
        });
    }

    #[test]
    fn pointer_types_are_interned() {
        BuilderCtx::with_default(|ctx| {
            let i32_ty = ctx.i32();
            let ptr1 = ctx.ptr_imm(i32_ty);
            let ptr2 = ctx.ptr_imm(i32_ty);
            assert_eq!(ptr1, ptr2, "Same pointer type should be deduplicated");

            let ptr_mut = ctx.ptr_mut(i32_ty);
            assert_ne!(
                ptr1, ptr_mut,
                "Different mutability should produce different types"
            );
        });
    }

    #[test]
    fn struct_types_are_created_correctly() {
        BuilderCtx::with_default(|ctx| {
            let i32_ty = ctx.i32();
            let f64_ty = ctx.f64();
            let struct_ty = ctx.struct_ty(&[i32_ty, f64_ty], false);

            assert!(struct_ty.is_struct());
        });
    }

    #[test]
    fn array_types_are_created_correctly() {
        BuilderCtx::with_default(|ctx| {
            let i32_ty = ctx.i32();
            let arr_ty = ctx.array(i32_ty, 10);

            assert!(arr_ty.is_array());
        });
    }

    #[test]
    fn c_string_interning() {
        BuilderCtx::with_default(|ctx| {
            let alloc1 = ctx.intern_c_str("hello");
            let alloc2 = ctx.intern_c_str("world");

            // Different strings should get different alloc IDs
            assert_ne!(alloc1, alloc2);
        });
    }

    #[test]
    fn bytes_interning() {
        BuilderCtx::with_default(|ctx| {
            let alloc1 = ctx.intern_bytes(&[1, 2, 3, 4]);
            let alloc2 = ctx.intern_bytes(&[5, 6, 7, 8]);

            assert_ne!(alloc1, alloc2);
        });
    }

    #[test]
    fn type_list_interning() {
        BuilderCtx::with_default(|ctx| {
            let i32_ty = ctx.i32();
            let f64_ty = ctx.f64();

            let list = ctx.type_list(&[i32_ty, f64_ty]);
            assert_eq!(list.as_slice().len(), 2);
        });
    }

    #[test]
    fn layout_computation() {
        BuilderCtx::with_default(|ctx| {
            let i32_ty = ctx.i32();
            let layout = ctx.layout_of(i32_ty);

            assert_eq!(layout.layout.size.bytes(), 4);
        });
    }

    #[test]
    fn with_target_sets_backend() {
        BuilderCtx::with_target(BackendKind::Llvm, EmitKind::LlvmIr, |ctx| {
            assert!(matches!(ctx.backend_kind(), BackendKind::Llvm));
            assert!(matches!(ctx.emit_kind(), EmitKind::LlvmIr));
        });
    }

    #[test]
    fn factory_methods_create_builders() {
        use tidec_tir::body::*;
        use tidec_tir::syntax::*;

        BuilderCtx::with_default(|ctx| {
            let metadata = TirBodyMetadata {
                def_id: DefId(0),
                name: "test_fn".to_string(),
                kind: TirBodyKind::Item(TirItemKind::Function),
                inlined: false,
                linkage: Linkage::External,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::None,
                call_conv: CallConv::C,
                is_varargs: false,
                is_declaration: false,
            };

            let mut fb = ctx.function_builder(metadata);
            fb.declare_ret(ctx.i32(), false);
            let entry = fb.create_block();
            fb.set_terminator(entry, Terminator::Return);
            let body = fb.build();

            assert_eq!(body.metadata.name, "test_fn");

            let mut unit = ctx.unit_builder("test_module");
            unit.add_body(body);
            let tir_unit = unit.build();

            assert_eq!(tir_unit.metadata.unit_name, "test_module");
            assert_eq!(tir_unit.bodies.len(), 1);
        });
    }
}
