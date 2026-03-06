//! Builder for constructing a [`TirBody`] (function / closure / coroutine).
//!
//! [`FunctionBuilder`] provides a high-level API that lets compiler front-ends
//! incrementally build a TIR function body without having to manually manage
//! local indices, basic-block indices, or the low-level data structures.
//!
//! # Workflow
//!
//! 1. Create a [`FunctionBuilder`] with [`FunctionBuilder::new`], supplying the
//!    function metadata.
//! 2. Declare the return type and parameter types with
//!    [`declare_ret`](FunctionBuilder::declare_ret) and
//!    [`declare_arg`](FunctionBuilder::declare_arg).
//! 3. Declare additional locals with [`declare_local`](FunctionBuilder::declare_local).
//! 4. Create basic blocks with [`create_block`](FunctionBuilder::create_block).
//! 5. Fill each block using [`block_builder`](FunctionBuilder::block_builder) or
//!    the convenience methods that operate directly on blocks.
//! 6. Call [`build`](FunctionBuilder::build) to produce the final [`TirBody`].
//!
//! # Example
//!
//! ```rust,ignore
//! use tidec_builder::FunctionBuilder;
//! use tidec_tir::body::*;
//! use tidec_tir::syntax::*;
//!
//! let metadata = TirBodyMetadata { /* … */ };
//! let mut fb = FunctionBuilder::new(metadata);
//!
//! // _0: i32 (return place)
//! fb.declare_ret(i32_ty, false);
//! // _1: i32 (first argument)
//! fb.declare_arg(i32_ty, false);
//!
//! let entry = fb.create_block();
//! {
//!     let bb = fb.block_builder(entry);
//!     bb.push_assign_operand(Place::from(RETURN_LOCAL), Operand::Use(Place::from(Local::new(1))));
//! }
//! fb.set_terminator(entry, Terminator::Return);
//!
//! let body = fb.build();
//! ```

use crate::basic_block_builder::BasicBlockBuilder;
use std::num::NonZero;
use tidec_tir::body::{CallConv, Linkage, TirBody, TirBodyMetadata};
use tidec_tir::ctx::TirCtx;
use tidec_tir::syntax::{
    BasicBlock, BasicBlockData, BinaryOp, ConstOperand, ConstScalar, ConstValue, Local, LocalData,
    Operand, Place, RValue, RawScalarValue, Statement, SwitchTargets, Terminator, UnaryOp,
    RETURN_LOCAL,
};
use tidec_tir::TirTy;
use tidec_utils::idx::Idx;
use tidec_utils::index_vec::IdxVec;

/// Tracks in-progress basic blocks before they are finalized.
///
/// While the block is being built, it holds accumulated statements and an
/// optional terminator. The block is not considered complete until a terminator
/// has been set.
struct InProgressBlock<'ctx> {
    statements: Vec<Statement<'ctx>>,
    terminator: Option<Terminator<'ctx>>,
}

impl<'ctx> InProgressBlock<'ctx> {
    fn new() -> Self {
        Self {
            statements: Vec::new(),
            terminator: None,
        }
    }
}

/// Builder for constructing a [`TirBody`].
///
/// See the [module-level documentation](self) for usage details.
pub struct FunctionBuilder<'ctx> {
    metadata: TirBodyMetadata,

    /// Optional TIR context, enabling convenience methods like
    /// [`const_i32`](Self::const_i32) directly on the builder.
    /// Set when created via [`BuilderCtx::function_builder`](crate::BuilderCtx::function_builder).
    tir_ctx: Option<TirCtx<'ctx>>,

    /// Locals that form the return value + arguments.
    /// `ret_and_args[0]` is always the return place.
    ret_and_args: IdxVec<Local, LocalData<'ctx>>,

    /// Additional (non-argument) locals.
    locals: IdxVec<Local, LocalData<'ctx>>,

    /// The total number of locals allocated so far (args + non-arg locals).
    /// Used to hand out monotonically-increasing [`Local`] indices.
    next_local_idx: usize,

    /// In-progress basic blocks, indexed by [`BasicBlock`].
    blocks: IdxVec<BasicBlock, InProgressBlock<'ctx>>,
}

impl<'ctx> FunctionBuilder<'ctx> {
    /// Create a new function builder with the given metadata.
    ///
    /// No locals or basic blocks are created automatically – the caller must
    /// at least call [`declare_ret`](Self::declare_ret) to set the return
    /// place.
    ///
    /// For access to convenience methods like [`const_i32`](Self::const_i32),
    /// use [`BuilderCtx::function_builder`](crate::BuilderCtx::function_builder)
    /// instead, which attaches the TIR context automatically.
    pub fn new(metadata: TirBodyMetadata) -> Self {
        Self {
            metadata,
            tir_ctx: None,
            ret_and_args: IdxVec::new(),
            locals: IdxVec::new(),
            next_local_idx: 0,
            blocks: IdxVec::new(),
        }
    }

    /// Create a new function builder with a [`TirCtx`] reference.
    ///
    /// The TIR context enables convenience methods such as
    /// [`const_i32`](Self::const_i32), [`const_bool`](Self::const_bool), etc.
    pub(crate) fn with_ctx(metadata: TirBodyMetadata, tir_ctx: TirCtx<'ctx>) -> Self {
        Self {
            metadata,
            tir_ctx: Some(tir_ctx),
            ret_and_args: IdxVec::new(),
            locals: IdxVec::new(),
            next_local_idx: 0,
            blocks: IdxVec::new(),
        }
    }

    /// Returns a reference to the [`TirCtx`], if one was provided at
    /// construction time.
    pub fn tir_ctx(&self) -> Option<&TirCtx<'ctx>> {
        self.tir_ctx.as_ref()
    }

    // ───────────────────── Local declarations ────────────────────

    /// Declare the **return local** (`_0`).
    ///
    /// This must be the very first local declared; it will always receive
    /// index `RETURN_LOCAL` (`Local(0)`).
    ///
    /// # Panics
    ///
    /// Panics if any local has already been declared (i.e. `declare_ret` was
    /// already called or an argument was declared first).
    pub fn declare_ret(&mut self, ty: TirTy<'ctx>, mutable: bool) -> Local {
        assert_eq!(
            self.next_local_idx, 0,
            "declare_ret must be the first local declaration"
        );
        let local = Local::new(self.next_local_idx);
        debug_assert_eq!(local, RETURN_LOCAL);
        self.ret_and_args.push(LocalData { ty, mutable });
        self.next_local_idx += 1;
        local
    }

    /// Declare a function **argument**.
    ///
    /// Arguments are stored immediately after the return local and must be
    /// declared *after* [`declare_ret`](Self::declare_ret) and *before* any
    /// call to [`declare_local`](Self::declare_local).
    ///
    /// Returns the [`Local`] index assigned to this argument.
    ///
    /// # Panics
    ///
    /// Panics if the return local has not been declared yet.
    pub fn declare_arg(&mut self, ty: TirTy<'ctx>, mutable: bool) -> Local {
        assert!(
            !self.ret_and_args.is_empty(),
            "declare_ret must be called before declare_arg"
        );
        let local = Local::new(self.next_local_idx);
        self.ret_and_args.push(LocalData { ty, mutable });
        self.next_local_idx += 1;
        local
    }

    /// Declare an additional (non-argument) **local variable**.
    ///
    /// Returns the [`Local`] index assigned to this local.
    ///
    /// # Panics
    ///
    /// Panics if the return local has not been declared yet.
    pub fn declare_local(&mut self, ty: TirTy<'ctx>, mutable: bool) -> Local {
        assert!(
            !self.ret_and_args.is_empty(),
            "declare_ret must be called before declare_local"
        );
        let local = Local::new(self.next_local_idx);
        self.locals.push(LocalData { ty, mutable });
        self.next_local_idx += 1;
        local
    }

    // ──────────────────── Basic-block management ─────────────────

    /// Create a new, empty basic block and return its [`BasicBlock`] index.
    ///
    /// The first block created will be the entry block (`BasicBlock(0)`).
    pub fn create_block(&mut self) -> BasicBlock {
        let bb = BasicBlock::new(self.blocks.len());
        self.blocks.push(InProgressBlock::new());
        bb
    }

    /// Return a [`BasicBlockBuilder`] pre-populated with the statements that
    /// have been pushed into `block` so far.
    ///
    /// **Important:** The builder is independent of the [`FunctionBuilder`]'s
    /// internal storage. To persist the statements you build, call
    /// [`apply_block_builder`](Self::apply_block_builder).
    ///
    /// # Panics
    ///
    /// Panics if `block` has not been created yet.
    pub fn block_builder(&self, block: BasicBlock) -> BasicBlockBuilder<'ctx> {
        let _ = &self.blocks[block]; // bounds-check
        BasicBlockBuilder::new()
    }

    /// Replace the contents of `block` with the result of a
    /// [`BasicBlockBuilder`].
    ///
    /// This overwrites any previously pushed statements **and** the
    /// terminator.
    ///
    /// # Panics
    ///
    /// Panics if `block` has not been created yet.
    pub fn apply_block_builder(&mut self, block: BasicBlock, data: BasicBlockData<'ctx>) {
        let ip = &mut self.blocks[block];
        ip.statements = data.statements;
        ip.terminator = Some(data.terminator);
    }

    // ──────────────── Statement-level convenience API ────────────

    /// Push a [`Statement`] to the end of `block`.
    ///
    /// # Panics
    ///
    /// Panics if `block` has not been created yet.
    pub fn push_statement(&mut self, block: BasicBlock, stmt: Statement<'ctx>) {
        self.blocks[block].statements.push(stmt);
    }

    /// Push an `Assign` statement to `block`.
    pub fn push_assign(
        &mut self,
        block: BasicBlock,
        place: tidec_tir::syntax::Place<'ctx>,
        rvalue: tidec_tir::syntax::RValue<'ctx>,
    ) {
        self.push_statement(block, Statement::Assign(Box::new((place, rvalue))));
    }

    // ──────────────────── Terminator management ──────────────────

    /// Set the terminator for `block`, overwriting any previously set
    /// terminator.
    ///
    /// # Panics
    ///
    /// Panics if `block` has not been created yet.
    pub fn set_terminator(&mut self, block: BasicBlock, terminator: Terminator<'ctx>) {
        self.blocks[block].terminator = Some(terminator);
    }

    /// Returns `true` if the given block already has a terminator set.
    pub fn has_terminator(&self, block: BasicBlock) -> bool {
        self.blocks[block].terminator.is_some()
    }

    // ──────────────────────── Introspection ──────────────────────

    /// Returns the total number of locals declared so far (return + args +
    /// other locals).
    pub fn num_locals(&self) -> usize {
        self.next_local_idx
    }

    /// Returns the number of arguments (excluding the return local).
    pub fn num_args(&self) -> usize {
        // ret_and_args contains return + args, so subtract 1 for the return
        // place (if it exists).
        if self.ret_and_args.is_empty() {
            0
        } else {
            self.ret_and_args.len() - 1
        }
    }

    /// Returns the number of basic blocks created so far.
    pub fn num_blocks(&self) -> usize {
        self.blocks.len()
    }

    // ──────────────────── Metadata modifiers ─────────────────────

    /// Mark this function as a **declaration** (external, no body).
    ///
    /// Declarations are used for FFI functions (e.g. `printf`, `malloc`).
    /// When set, the codegen layer will emit a declaration instead of a
    /// definition. A dummy unreachable block should still be added.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut fb = ctx.function_builder(TirBodyMetadata::function(id, "printf"));
    /// fb.set_declaration();
    /// fb.set_varargs();
    /// fb.declare_ret(ctx.i32(), false);
    /// fb.declare_arg(ctx.ptr_imm(ctx.i8()), false);
    /// let entry = fb.create_block();
    /// fb.set_terminator(entry, Terminator::Unreachable);
    /// let printf_body = fb.build();
    /// ```
    pub fn set_declaration(&mut self) -> &mut Self {
        self.metadata.is_declaration = true;
        self
    }

    /// Mark this function as variadic (e.g. `printf`).
    pub fn set_varargs(&mut self) -> &mut Self {
        self.metadata.is_varargs = true;
        self
    }

    /// Set the calling convention for this function.
    pub fn set_call_conv(&mut self, cc: CallConv) -> &mut Self {
        self.metadata.call_conv = cc;
        self
    }

    /// Set the linkage for this function.
    pub fn set_linkage(&mut self, linkage: Linkage) -> &mut Self {
        self.metadata.linkage = linkage;
        self
    }

    /// Returns a shared reference to the function metadata.
    pub fn metadata(&self) -> &TirBodyMetadata {
        &self.metadata
    }

    /// Returns a mutable reference to the function metadata for
    /// arbitrary modifications.
    pub fn metadata_mut(&mut self) -> &mut TirBodyMetadata {
        &mut self.metadata
    }

    // ────────────────── Convenience constant methods ─────────────
    //
    // These methods require a `TirCtx` to have been provided at
    // construction time (via `BuilderCtx::function_builder`). They
    // allow writing `fb.const_i32(42)` instead of the verbose
    // `ConstOperand::Value(ConstValue::Scalar(...), i32_ty)` pattern.

    /// Create a constant `i32` operand.
    ///
    /// # Panics
    ///
    /// Panics if the `FunctionBuilder` was created without a `TirCtx`
    /// (i.e. via `FunctionBuilder::new` instead of `BuilderCtx::function_builder`).
    pub fn const_i32(&self, v: i32) -> Operand<'ctx> {
        let ctx = self.expect_tir_ctx();
        Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: v as u32 as u128,
                size: NonZero::new(4).unwrap(),
            })),
            ctx.intern_ty(tidec_tir::ty::TirTy::I32),
        ))
    }

    /// Create a constant `i64` operand.
    ///
    /// # Panics
    ///
    /// Panics if no `TirCtx` was provided.
    pub fn const_i64(&self, v: i64) -> Operand<'ctx> {
        let ctx = self.expect_tir_ctx();
        Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: v as u64 as u128,
                size: NonZero::new(8).unwrap(),
            })),
            ctx.intern_ty(tidec_tir::ty::TirTy::I64),
        ))
    }

    /// Create a constant `bool` operand.
    ///
    /// # Panics
    ///
    /// Panics if no `TirCtx` was provided.
    pub fn const_bool(&self, v: bool) -> Operand<'ctx> {
        let ctx = self.expect_tir_ctx();
        Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: u128::from(v),
                size: NonZero::new(1).unwrap(),
            })),
            ctx.intern_ty(tidec_tir::ty::TirTy::Bool),
        ))
    }

    /// Create a constant `f64` operand.
    ///
    /// # Panics
    ///
    /// Panics if no `TirCtx` was provided.
    pub fn const_f64(&self, v: f64) -> Operand<'ctx> {
        let ctx = self.expect_tir_ctx();
        Operand::Const(ConstOperand::Value(
            ConstValue::Scalar(ConstScalar::Value(RawScalarValue {
                data: v.to_bits() as u128,
                size: NonZero::new(8).unwrap(),
            })),
            ctx.intern_ty(tidec_tir::ty::TirTy::F64),
        ))
    }

    fn expect_tir_ctx(&self) -> &TirCtx<'ctx> {
        self.tir_ctx.as_ref().expect(
            "convenience constant methods require a TirCtx; \
             use BuilderCtx::function_builder() instead of FunctionBuilder::new()",
        )
    }

    // ──────────────── Place / Operand helpers ────────────────────

    /// Return the [`Place`] corresponding to the return local (`_0`).
    ///
    /// This is a convenience for `Place::from(RETURN_LOCAL)`.
    pub fn return_place(&self) -> Place<'ctx> {
        Place::from(RETURN_LOCAL)
    }

    /// Return a [`Place`] for the given [`Local`] (no projections).
    ///
    /// This is a convenience for `Place::from(local)`.
    pub fn local_place(&self, local: Local) -> Place<'ctx> {
        Place::from(local)
    }

    /// Create an [`Operand::Use`] that loads from the given [`Place`].
    pub fn use_place(&self, place: &Place<'ctx>) -> Operand<'ctx> {
        Operand::Use(place.clone())
    }

    /// Create an [`Operand::Use`] that loads from the given [`Local`].
    ///
    /// Shorthand for `self.use_place(&self.local_place(local))`.
    pub fn use_local(&self, local: Local) -> Operand<'ctx> {
        Operand::use_local(local)
    }

    // ──────────── High-level statement emission ─────────────────

    /// Emit `place = operand` (wraps the operand in [`RValue::Operand`]).
    ///
    /// # Panics
    ///
    /// Panics if `block` has not been created yet.
    pub fn assign_operand(
        &mut self,
        block: BasicBlock,
        place: Place<'ctx>,
        operand: Operand<'ctx>,
    ) {
        self.push_assign(block, place, RValue::Operand(operand));
    }

    /// Emit `dest = lhs <op> rhs` with an automatically declared
    /// temporary local of type `result_ty`.
    ///
    /// Returns the ([`Local`], [`Place`]) of the destination so that the
    /// caller can build an [`Operand::Use`] from it.
    ///
    /// # Panics
    ///
    /// Panics if `block` has not been created yet.
    pub fn assign_binary_op(
        &mut self,
        block: BasicBlock,
        op: BinaryOp,
        lhs: Operand<'ctx>,
        rhs: Operand<'ctx>,
        result_ty: TirTy<'ctx>,
    ) -> (Local, Place<'ctx>) {
        let local = self.declare_local(result_ty, false);
        let place = Place::from(local);
        self.push_assign(block, place.clone(), RValue::BinaryOp(op, lhs, rhs));
        (local, place)
    }

    /// Emit `dest = <op> src` with an automatically declared temporary
    /// local of type `result_ty`.
    ///
    /// Returns the ([`Local`], [`Place`]) of the destination.
    ///
    /// # Panics
    ///
    /// Panics if `block` has not been created yet.
    pub fn assign_unary_op(
        &mut self,
        block: BasicBlock,
        op: UnaryOp,
        src: Operand<'ctx>,
        result_ty: TirTy<'ctx>,
    ) -> (Local, Place<'ctx>) {
        let local = self.declare_local(result_ty, false);
        let place = Place::from(local);
        self.push_assign(block, place.clone(), RValue::UnaryOp(op, src));
        (local, place)
    }

    // ──────────── High-level terminator emission ────────────────

    /// Set the terminator of `block` to [`Terminator::Return`].
    ///
    /// # Panics
    ///
    /// Panics if `block` has not been created yet.
    pub fn emit_return(&mut self, block: BasicBlock) {
        self.set_terminator(block, Terminator::Return);
    }

    /// Set the terminator of `block` to [`Terminator::Goto`] targeting
    /// `target`.
    ///
    /// # Panics
    ///
    /// Panics if `block` has not been created yet.
    pub fn emit_goto(&mut self, block: BasicBlock, target: BasicBlock) {
        self.set_terminator(block, Terminator::Goto { target });
    }

    /// Set the terminator of `block` to a two-arm
    /// [`Terminator::SwitchInt`] (if/else branch).
    ///
    /// `discr` is the discriminant operand (expected to be a boolean).
    /// When true (`1`), control flows to `then_bb`; otherwise to `else_bb`.
    ///
    /// # Panics
    ///
    /// Panics if `block` has not been created yet.
    pub fn emit_branch(
        &mut self,
        block: BasicBlock,
        discr: Operand<'ctx>,
        then_bb: BasicBlock,
        else_bb: BasicBlock,
    ) {
        let targets = SwitchTargets::if_then(then_bb, else_bb);
        self.set_terminator(block, Terminator::SwitchInt { discr, targets });
    }

    /// Set the terminator of `block` to a [`Terminator::Call`].
    ///
    /// * `func`        — the function operand (e.g. from
    ///                    [`BuilderCtx::fn_operand`](crate::BuilderCtx::fn_operand)).
    /// * `args`        — call arguments.
    /// * `destination` — the [`Place`] where the return value is written.
    /// * `target`      — the continuation block after the call returns.
    ///
    /// # Panics
    ///
    /// Panics if `block` has not been created yet.
    pub fn emit_call(
        &mut self,
        block: BasicBlock,
        func: Operand<'ctx>,
        args: Vec<Operand<'ctx>>,
        destination: Place<'ctx>,
        target: BasicBlock,
    ) {
        self.set_terminator(
            block,
            Terminator::Call {
                func,
                args,
                destination,
                target,
            },
        );
    }

    // ────────────────────── Finalization ─────────────────────────

    /// Consume the builder and produce the finished [`TirBody`].
    ///
    /// # Panics
    ///
    /// * Panics if the return local has not been declared.
    /// * Panics if any basic block is missing its terminator.
    pub fn build(self) -> TirBody<'ctx> {
        self.try_build().unwrap_or_else(|e| panic!("{e}"))
    }

    /// Consume the builder and attempt to produce a [`TirBody`].
    ///
    /// Returns `Err(BuildError)` instead of panicking when validation
    /// fails—useful for front-ends that want to report errors gracefully.
    pub fn try_build(self) -> Result<TirBody<'ctx>, BuildError> {
        if self.ret_and_args.is_empty() {
            return Err(BuildError::MissingReturnLocal);
        }

        let mut basic_blocks: IdxVec<BasicBlock, BasicBlockData<'ctx>> = IdxVec::new();
        for (bb_idx, ip) in self.blocks.into_iter_enumerated() {
            let terminator = ip
                .terminator
                .ok_or(BuildError::MissingTerminator { block: bb_idx })?;
            basic_blocks.push(BasicBlockData {
                statements: ip.statements,
                terminator,
            });
        }

        Ok(TirBody {
            metadata: self.metadata,
            ret_and_args: self.ret_and_args,
            locals: self.locals,
            basic_blocks,
        })
    }
}

/// Errors that can occur when building a [`TirBody`] via
/// [`FunctionBuilder::try_build`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// The return local was never declared (no call to `declare_ret`).
    MissingReturnLocal,
    /// A basic block is missing its terminator.
    MissingTerminator {
        /// The block that has no terminator.
        block: BasicBlock,
    },
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::MissingReturnLocal => {
                write!(
                    f,
                    "cannot build a TirBody without a return local (call declare_ret first)"
                )
            }
            BuildError::MissingTerminator { block } => {
                write!(f, "basic block {} is missing a terminator", block.idx())
            }
        }
    }
}

impl std::error::Error for BuildError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tidec_abi::target::{BackendKind, TirTarget};
    use tidec_tir::body::*;
    use tidec_tir::ctx::{EmitKind, InternCtx, TirArena, TirArgs, TirCtx};

    use tidec_tir::ty;

    /// Helper to create a `TirCtx` for interning types in tests.
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

    #[test]
    fn minimal_function_with_return() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);

            let mut fb = FunctionBuilder::new(make_metadata("minimal"));
            let ret = fb.declare_ret(i32_ty, false);
            assert_eq!(ret, RETURN_LOCAL);

            let entry = fb.create_block();
            fb.set_terminator(entry, Terminator::Return);

            let body = fb.build();
            assert_eq!(body.ret_and_args.len(), 1); // only return local
            assert!(body.locals.is_empty());
            assert_eq!(body.basic_blocks.len(), 1);
            assert!(matches!(
                body.basic_blocks[BasicBlock::new(0)].terminator,
                Terminator::Return
            ));
        });
    }

    #[test]
    fn function_with_args_and_locals() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);
            let f64_ty = ctx.intern_ty(ty::TirTy::F64);

            let mut fb = FunctionBuilder::new(make_metadata("with_args"));
            fb.declare_ret(i32_ty, false);
            let arg1 = fb.declare_arg(i32_ty, false);
            let arg2 = fb.declare_arg(f64_ty, false);
            let tmp = fb.declare_local(i32_ty, true);

            assert_eq!(arg1, Local::new(1));
            assert_eq!(arg2, Local::new(2));
            assert_eq!(tmp, Local::new(3));

            assert_eq!(fb.num_args(), 2);
            assert_eq!(fb.num_locals(), 4);

            let entry = fb.create_block();
            fb.set_terminator(entry, Terminator::Return);

            let body = fb.build();
            assert_eq!(body.ret_and_args.len(), 3); // ret + 2 args
            assert_eq!(body.locals.len(), 1); // 1 additional local
        });
    }

    #[test]
    fn push_statements_directly() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);

            let mut fb = FunctionBuilder::new(make_metadata("push_stmts"));
            fb.declare_ret(i32_ty, false);
            let arg = fb.declare_arg(i32_ty, false);

            let entry = fb.create_block();
            fb.push_assign(
                entry,
                Place::from(RETURN_LOCAL),
                RValue::Operand(Operand::Use(Place::from(arg))),
            );
            fb.set_terminator(entry, Terminator::Return);

            let body = fb.build();
            assert_eq!(body.basic_blocks[BasicBlock::new(0)].statements.len(), 1);
        });
    }

    #[test]
    fn multiple_blocks_with_goto() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);

            let mut fb = FunctionBuilder::new(make_metadata("multi_block"));
            fb.declare_ret(i32_ty, false);

            let entry = fb.create_block();
            let exit = fb.create_block();

            fb.set_terminator(entry, Terminator::Goto { target: exit });
            fb.set_terminator(exit, Terminator::Return);

            assert_eq!(fb.num_blocks(), 2);
            assert!(fb.has_terminator(entry));
            assert!(fb.has_terminator(exit));

            let body = fb.build();
            assert_eq!(body.basic_blocks.len(), 2);
            assert!(matches!(
                body.basic_blocks[BasicBlock::new(0)].terminator,
                Terminator::Goto { target } if target == BasicBlock::new(1)
            ));
        });
    }

    #[test]
    fn apply_block_builder_replaces_content() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);

            let mut fb = FunctionBuilder::new(make_metadata("apply_bb"));
            fb.declare_ret(i32_ty, false);
            fb.declare_arg(i32_ty, false);

            let entry = fb.create_block();

            // Build the block externally.
            let mut bb = BasicBlockBuilder::new();
            bb.push_assign_operand(
                Place::from(RETURN_LOCAL),
                Operand::Use(Place::from(Local::new(1))),
            );
            let data = bb.build(Terminator::Return);

            fb.apply_block_builder(entry, data);
            let body = fb.build();

            assert_eq!(body.basic_blocks[BasicBlock::new(0)].statements.len(), 1);
            assert!(matches!(
                body.basic_blocks[BasicBlock::new(0)].terminator,
                Terminator::Return
            ));
        });
    }

    #[test]
    #[should_panic(expected = "declare_ret must be the first local declaration")]
    fn declare_ret_twice_panics() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);
            let mut fb = FunctionBuilder::new(make_metadata("bad"));
            fb.declare_ret(i32_ty, false);
            fb.declare_ret(i32_ty, false); // should panic
        });
    }

    #[test]
    #[should_panic(expected = "declare_ret must be called before declare_arg")]
    fn declare_arg_before_ret_panics() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);
            let mut fb = FunctionBuilder::new(make_metadata("bad"));
            fb.declare_arg(i32_ty, false); // should panic
        });
    }

    #[test]
    #[should_panic(expected = "cannot build a TirBody without a return local")]
    fn build_without_ret_panics() {
        let fb = FunctionBuilder::new(make_metadata("no_ret"));
        fb.build();
    }

    #[test]
    #[should_panic(expected = "missing a terminator")]
    fn build_with_missing_terminator_panics() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);
            let mut fb = FunctionBuilder::new(make_metadata("no_term"));
            fb.declare_ret(i32_ty, false);
            fb.create_block(); // no terminator set
            fb.build();
        });
    }

    #[test]
    fn set_terminator_overwrites() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);

            let mut fb = FunctionBuilder::new(make_metadata("overwrite"));
            fb.declare_ret(i32_ty, false);

            let entry = fb.create_block();
            fb.set_terminator(entry, Terminator::Unreachable);
            fb.set_terminator(entry, Terminator::Return);

            let body = fb.build();
            assert!(matches!(
                body.basic_blocks[BasicBlock::new(0)].terminator,
                Terminator::Return
            ));
        });
    }

    #[test]
    fn num_args_with_no_ret_returns_zero() {
        let fb = FunctionBuilder::<'_>::new(make_metadata("empty"));
        assert_eq!(fb.num_args(), 0);
    }

    #[test]
    fn block_builder_returns_fresh_builder() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);

            let mut fb = FunctionBuilder::new(make_metadata("fresh_bb"));
            fb.declare_ret(i32_ty, false);
            let entry = fb.create_block();

            let bb = fb.block_builder(entry);
            assert!(bb.is_empty());
        });
    }

    #[test]
    fn function_with_call_terminator() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);
            let unit_ty = ctx.intern_ty(ty::TirTy::Unit);

            let mut fb = FunctionBuilder::new(make_metadata("caller"));
            fb.declare_ret(unit_ty, false);
            let arg = fb.declare_arg(i32_ty, false);
            let dest = fb.declare_local(i32_ty, true);

            let entry = fb.create_block();
            let cont = fb.create_block();

            fb.set_terminator(
                entry,
                Terminator::Call {
                    func: Operand::Use(Place::from(arg)),
                    args: vec![Operand::Use(Place::from(arg))],
                    destination: Place::from(dest),
                    target: cont,
                },
            );
            fb.set_terminator(cont, Terminator::Return);

            let body = fb.build();
            assert_eq!(body.basic_blocks.len(), 2);
            assert!(matches!(
                body.basic_blocks[BasicBlock::new(0)].terminator,
                Terminator::Call { .. }
            ));
        });
    }

    #[test]
    fn function_metadata_preserved() {
        with_ctx(|ctx| {
            let i32_ty = ctx.intern_ty(ty::TirTy::I32);

            let mut fb = FunctionBuilder::new(make_metadata("my_fn"));
            fb.declare_ret(i32_ty, false);
            let entry = fb.create_block();
            fb.set_terminator(entry, Terminator::Return);

            let body = fb.build();
            assert_eq!(body.metadata.name, "my_fn");
            assert!(matches!(
                body.metadata.kind,
                TirBodyKind::Item(TirItemKind::Function)
            ));
            assert!(!body.metadata.is_varargs);
            assert!(!body.metadata.is_declaration);
        });
    }
}
