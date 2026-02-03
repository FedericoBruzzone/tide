pub mod alloc;
pub mod body;
pub mod ctx;
pub mod layout_ctx;
pub mod syntax;
pub mod ty;

use crate::ctx::TirCtx;
use std::ops::Deref;
use tidec_utils::interner::{Interned, Ty};

pub struct TirTy<'ctx>(pub Interned<'ctx, crate::ty::TirTy<TirCtx<'ctx>>>);
impl<'ctx> Ty<TirCtx<'ctx>> for TirTy<'ctx> {}

impl<'ctx> std::fmt::Debug for TirTy<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl<'ctx> Clone for TirTy<'ctx> {
    fn clone(&self) -> Self {
        *self // Assuming Interned is Copy
    }
}

impl<'ctx> Copy for TirTy<'ctx> {}

impl<'ctx> PartialEq for TirTy<'ctx> {
    fn eq(&self, other: &Self) -> bool {
        // Just compare the Interned fields.
        self.0.eq(&other.0)
    }
}

impl<'ctx> Eq for TirTy<'ctx> {} // Trivial if PartialEq is implemented correctly

impl<'ctx> std::hash::Hash for TirTy<'ctx> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash only the Interned field, which internally will skip the non-Hashable parts.
        self.0.hash(state);
    }
}

impl<'ctx> Deref for TirTy<'ctx> {
    type Target = Interned<'ctx, crate::ty::TirTy<ctx::TirCtx<'ctx>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
