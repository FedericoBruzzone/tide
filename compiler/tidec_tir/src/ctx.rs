use std::{collections::HashSet};

use tidec_abi::{
    layout::{self, TyAndLayout},
    target::{BackendKind, TirTarget},
};
use tidec_utils::interner::Interner;

use crate::{layout_ctx::LayoutCtx, ty, TirTy};

#[derive(Debug, Clone, Copy)]
pub enum EmitKind {
    Assembly,
    Object,
    Ir,
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

#[derive(Debug, Clone, Copy)]
pub struct TirCtx<'ctx> {
    target: &'ctx TirTarget,
    arguments: &'ctx TirArgs,

    intern_ctx: &'ctx InternCtx<'ctx>,
    // TODO(bruzzone): here we should have, other then an arena, also a HashMap from DefId
    // to the body of the function.
}

impl<'ctx> TirCtx<'ctx> {
    pub fn target(&self) -> &TirTarget {
        &self.target
    }

    pub fn layout_of(&self, ty: TirTy<'ctx>) -> TyAndLayout<TirTy<'ctx>> {
        let layout_ctx = LayoutCtx::new(self);
        layout_ctx.compute_layout(ty)
    }

    pub fn backend_kind(&self) -> &BackendKind {
        &self.target.codegen_backend
    }

    pub fn emit_kind(&self) -> &EmitKind {
        &self.arguments.emit_kind
    }
}

impl<'ctx> Interner for TirCtx<'ctx> {
    type Ty = TirTy<'ctx>;
    type Layout = layout::Layout;

    fn intern_layout(&self, ty: Self::Layout) -> Self::Layout {
        todo!()
    }

    fn intern_ty(&self, ty: Self::Ty) -> Self::Ty {
        // TODO(bruzzone): implement proper interning
        todo!()
    }
}
