use crate::ctx::Interner;
use std::hash::Hash;

#[derive(Debug, Clone, Copy)]
pub enum TirTy<I: Interner> {
    // The `()` type. This is equivalent to a
    // zero-sized type or void in some languages.
    // Unit,

    // Signed integers
    I8,
    I16,
    I32,
    I64,
    I128,

    // Unsigned integers
    U8,
    U16,
    U32,
    U64,
    U128,

    // Floating-point types
    F16,
    F32,
    F64,
    F128,

    /// A raw pointer.
    /// The first field is the pointee type.
    /// `Mutability` indicates whether the pointer is mutable or immutable.
    /// For example, `*imm T` or `*mut T`.
    ///
    /// Note that when mutable is a c-style pointer.
    RawPtr(I::Ty, Mutability),

    /// A function pointer.
    // FnPty {
    //     param_tys: Vec<TirTy>,
    //     ret_ty: Box<TirTy>,
    // },

    // https://llvm.org/docs/TypeMetadata.html
    Metadata,
}

impl<I: Interner> TirTy<I> {
    pub fn is_floating_point(&self) -> bool {
        matches!(self, TirTy::F16 | TirTy::F32 | TirTy::F64 | TirTy::F128)
    }

    pub fn is_signed_integer(&self) -> bool {
        matches!(
            self,
            TirTy::I8 | TirTy::I16 | TirTy::I32 | TirTy::I64 | TirTy::I128
        )
    }

    /// This function returns true if the type is a sized type.
    /// That is, it has a known size at compile time.
    pub fn is_sized(&self) -> bool {
        match self {
            TirTy::I8
            | TirTy::I16
            | TirTy::I32
            | TirTy::I64
            | TirTy::I128
            | TirTy::U8
            | TirTy::U16
            | TirTy::U32
            | TirTy::U64
            | TirTy::U128
            | TirTy::F16
            | TirTy::F32
            | TirTy::F64
            | TirTy::F128 => true,
            TirTy::RawPtr(_, _) => true,
            // TirTy::FnPty { .. } => true,
            TirTy::Metadata => false,
        }
    }
}

#[derive(Debug, Hash, Copy, Clone, Eq, PartialEq)]
pub enum Mutability {
    Mut,
    Imm,
}

impl<I: Interner> PartialEq for TirTy<I> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TirTy::I8, TirTy::I8)
            | (TirTy::I16, TirTy::I16)
            | (TirTy::I32, TirTy::I32)
            | (TirTy::I64, TirTy::I64)
            | (TirTy::I128, TirTy::I128)
            | (TirTy::U8, TirTy::U8)
            | (TirTy::U16, TirTy::U16)
            | (TirTy::U32, TirTy::U32)
            | (TirTy::U64, TirTy::U64)
            | (TirTy::U128, TirTy::U128)
            | (TirTy::F16, TirTy::F16)
            | (TirTy::F32, TirTy::F32)
            | (TirTy::F64, TirTy::F64)
            | (TirTy::F128, TirTy::F128) => true,
            (TirTy::RawPtr(ty1, mut1), TirTy::RawPtr(ty2, mut2)) => ty1 == ty2 && mut1 == mut2,
            // (
            //     TirTy::FnPty {
            //         param_tys: params1,
            //         ret_ty: ret1,
            //     },
            //     TirTy::FnPty {
            //         param_tys: params2,
            //         ret_ty: ret2,
            //     },
            // ) => params1 == params2 && ret1 == ret2,
            (TirTy::Metadata, TirTy::Metadata) => true,
            _ => false,
        }
    }
}

impl<I: Interner> Eq for TirTy<I> {}

impl<I: Interner> Hash for TirTy<I> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            TirTy::I8 => 0.hash(state),
            TirTy::I16 => 1.hash(state),
            TirTy::I32 => 2.hash(state),
            TirTy::I64 => 3.hash(state),
            TirTy::I128 => 4.hash(state),
            TirTy::U8 => 5.hash(state),
            TirTy::U16 => 6.hash(state),
            TirTy::U32 => 7.hash(state),
            TirTy::U64 => 8.hash(state),
            TirTy::U128 => 9.hash(state),
            TirTy::F16 => 10.hash(state),
            TirTy::F32 => 11.hash(state),
            TirTy::F64 => 12.hash(state),
            TirTy::F128 => 13.hash(state),
            TirTy::RawPtr(ty, mutability) => {
                14.hash(state);
                ty.hash(state);
                mutability.hash(state);
            }
            // TirTy::FnPty { param_tys, ret_ty } => {
            //     15.hash(state);
            //     param_tys.hash(state);
            //     ret_ty.hash(state);
            // }
            TirTy::Metadata => 16.hash(state),
        }
    }
}