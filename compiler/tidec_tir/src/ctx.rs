use std::{collections::HashSet};

use tidec_abi::{
    layout::{self, TyAndLayout},
    target::{BackendKind, TirTarget}, Layout,
};
use tidec_utils::interner::Interner;
use crate::{layout_ctx::LayoutCtx, ty, TirTy};

#[derive(Debug, Clone, Copy)]
pub enum EmitKind {
    Assembly,
    Object,
    LlvmIr,
}

#[derive(Debug, Clone, Copy)]
pub struct TirArgs {
    pub emit_kind: EmitKind,
}

#[derive(Debug, Clone, Copy)]
/// A pointer to a value allocated in an arena.
pub struct ArenaPrt<'ctx, T: Sized>(&'ctx T);

#[derive(Debug, Clone)]
/// An arena for allocating TIR values.
pub struct TirArena<'ctx> {
    types: Vec<Box<ty::TirTy<TirCtx<'ctx>>>>,
}

impl<'ctx> Default for TirArena<'ctx> {
    fn default() -> Self {
        Self {
            types: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
/// The context for all interned entities in TIR.
/// 
/// It contains an arena for interning all TIR types and layouts, as well as
/// other cacheable information.
pub struct InternCtx<'ctx> {
    /// The arena for allocating TIR types, layouts, and other interned entities.
    arena: &'ctx TirArena<'ctx>,
    /// A set of all interned TIR types.
    types: HashSet<ArenaPrt<'ctx, ty::TirTy<TirCtx<'ctx>>>>,
}

impl<'ctx> InternCtx<'ctx> {
    pub fn new(arena: &'ctx TirArena<'ctx>) -> Self {
        Self {
            arena,
            types: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TirCtx<'ctx> {
    target: &'ctx TirTarget,
    arguments: &'ctx TirArgs,

    intern_ctx: &'ctx InternCtx<'ctx>,

    // TODO(bruzzone): here we should have, other then an arena, also a HashMap from DefId
    // to the body of the function.
}

impl<'ctx> TirCtx<'ctx> {
    pub fn new(
        target: &'ctx TirTarget,
        arguments: &'ctx TirArgs,
        intern_ctx: &'ctx InternCtx<'ctx>,
    ) -> Self {
        Self {
            target,
            arguments,
            intern_ctx,
        }
    }

    pub fn target(&self) -> &TirTarget {
        &self.target
    }

    pub fn layout_of(self, ty: TirTy<'ctx>) -> TyAndLayout<'ctx, TirTy<'ctx>> {
        let layout_ctx = LayoutCtx::new(self);
        let layout = layout_ctx.compute_layout(ty);
        TyAndLayout { 
            ty,
            layout,
        }
    }

    pub fn backend_kind(&self) -> &BackendKind {
        &self.target.codegen_backend
    }

    pub fn emit_kind(&self) -> &EmitKind {
        &self.arguments.emit_kind
    }

    // ===== Direct inter =====
    pub fn intern_layout(&self, _layout: layout::Layout) -> Layout<'ctx> {
        todo!()
    }
}

impl<'ctx> Interner for TirCtx<'ctx> {
    type Ty = TirTy<'ctx>;
    
    fn intern_ty<T>(&self, _ty: T) -> Self::Ty {
        todo!()
    }
}
