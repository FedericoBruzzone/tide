pub mod alloc;
pub mod body;
pub mod ctx;
pub mod layout_ctx;
pub mod syntax;
pub mod ty;

use crate::ctx::TirCtx;
use std::ops::Deref;
use tidec_utils::interner::{Interned, Ty, TypeList};

/// An interned allocation, similar to how `TirTy` and `Layout` are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TirAllocation<'ctx>(pub Interned<'ctx, crate::alloc::Allocation>);

impl<'ctx> Deref for TirAllocation<'ctx> {
    type Target = Interned<'ctx, crate::alloc::Allocation>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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

/// An interned list of types, used to represent struct fields.
///
/// This is a thin wrapper around `&'ctx [TirTy<'ctx>]` (an arena-allocated
/// slice of interned types). It is `Copy` because slice references are `Copy`.
///
/// Two `TirTypeList` values are equal if and only if they point to the same
/// arena-allocated slice (pointer equality), which is guaranteed by the
/// interning infrastructure.
#[derive(Clone, Copy)]
pub struct TirTypeList<'ctx>(&'ctx [TirTy<'ctx>]);

impl<'ctx> TirTypeList<'ctx> {
    /// Create a new `TirTypeList` from an arena-allocated slice.
    pub fn new(slice: &'ctx [TirTy<'ctx>]) -> Self {
        TirTypeList(slice)
    }

    /// Returns the underlying slice.
    pub fn as_slice(&self) -> &'ctx [TirTy<'ctx>] {
        self.0
    }
}

impl<'ctx> std::fmt::Debug for TirTypeList<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TirTypeList({:?})", self.0)
    }
}

impl<'ctx> PartialEq for TirTypeList<'ctx> {
    fn eq(&self, other: &Self) -> bool {
        // Pointer equality: same arena allocation means same list.
        std::ptr::eq(self.0.as_ptr(), other.0.as_ptr()) && self.0.len() == other.0.len()
    }
}

impl<'ctx> Eq for TirTypeList<'ctx> {}

impl<'ctx> std::hash::Hash for TirTypeList<'ctx> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the pointer and length for pointer-identity hashing.
        (self.0.as_ptr() as usize).hash(state);
        self.0.len().hash(state);
    }
}

impl<'ctx> TypeList<TirCtx<'ctx>> for TirTypeList<'ctx> {
    fn as_slice(&self) -> &[TirTy<'ctx>] {
        self.0
    }
}
