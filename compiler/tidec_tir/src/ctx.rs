use tidec_abi::{
    layout::TyAndLayout,
    target::{BackendKind, TirTarget},
};
use tidec_utils::interner::Interner;

use crate::{layout_ctx::LayoutCtx, TirTy};

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
pub struct TirCtx<'ctx> {
    target: &'ctx TirTarget,
    arguments: &'ctx TirArgs,
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

    fn intern_ty(&self, ty: Self::Ty) -> Self::Ty {
        // TODO(bruzzone): implement proper interning
        ty
    }
}
