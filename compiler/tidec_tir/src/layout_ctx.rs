use crate::{syntax::TirTy, tir::TirCtx};
use tidec_abi::{
    layout::{BackendRepr, Layout, Primitive, TyAndLayout},
    size_and_align::{AbiAndPrefAlign, Size},
};

pub struct LayoutCtx<'a> {
    lir_ctx: &'a TirCtx,
}

impl<'a> LayoutCtx<'a> {
    // It accepts the `TirCtx` because it contains the `TargetDataLayout`.
    pub fn new(lir_ctx: &'a TirCtx) -> Self {
        LayoutCtx { lir_ctx }
    }

    /// Computes the layout for a given type. We should cache the results
    /// to avoid recomputing the layout for the same type multiple times.
    pub fn compute_layout(&self, ty: TirTy) -> TyAndLayout<TirTy> {
        let data_layout = &self.lir_ctx.target().data_layout;

        let scalar = |primitive: Primitive| -> (Size, AbiAndPrefAlign, BackendRepr) {
            let (size, align) = match primitive {
                Primitive::I8 => (Size::from_bits(8), data_layout.int8_align),
                Primitive::I16 => (Size::from_bits(16), data_layout.int16_align),
                Primitive::I32 => (Size::from_bits(32), data_layout.int32_align),
                Primitive::I64 => (Size::from_bits(64), data_layout.int64_align),
                Primitive::I128 => (Size::from_bits(128), data_layout.int128_align),
                Primitive::U8 => (Size::from_bits(8), data_layout.int8_align),
                Primitive::U16 => (Size::from_bits(16), data_layout.int16_align),
                Primitive::U32 => (Size::from_bits(32), data_layout.int32_align),
                Primitive::U64 => (Size::from_bits(64), data_layout.int64_align),
                Primitive::U128 => (Size::from_bits(128), data_layout.int128_align),
                Primitive::F16 => (Size::from_bits(16), data_layout.float16_align),
                Primitive::F32 => (Size::from_bits(32), data_layout.float32_align),
                Primitive::F64 => (Size::from_bits(64), data_layout.float64_align),
                Primitive::F128 => (Size::from_bits(128), data_layout.float128_align),
                Primitive::Pointer(address_space) => (
                    data_layout.pointer_size(),
                    data_layout.pointer_align(address_space),
                ),
            };
            (size, align, BackendRepr::Scalar(primitive))
        };

        let (size, align, backend_repr) = match ty {
            TirTy::I8 => scalar(Primitive::I8),
            TirTy::I16 => scalar(Primitive::I16),
            TirTy::I32 => scalar(Primitive::I32),
            TirTy::I64 => scalar(Primitive::I64),
            TirTy::I128 => scalar(Primitive::I128),
            TirTy::U8 => scalar(Primitive::U8),
            TirTy::U16 => scalar(Primitive::U16),
            TirTy::U32 => scalar(Primitive::U32),
            TirTy::U64 => scalar(Primitive::U64),
            TirTy::U128 => scalar(Primitive::U128),
            TirTy::F16 => scalar(Primitive::F16),
            TirTy::F32 => scalar(Primitive::F32),
            TirTy::F64 => scalar(Primitive::F64),
            TirTy::F128 => scalar(Primitive::F128),
            // TODO: Implement layout computation for Metadata types (e.g., for unsized types or trait objects).
            // Metadata represents type information for unsized types (such as slices or trait objects),
            // which require special handling for their layout. Support for this will be added in a future release.
            TirTy::Metadata => unimplemented!("Layout computation for TirTy::Metadata (used for unsized types/trait objects) is not yet supported. See TODO comment for details."),
        };

        TyAndLayout {
            ty,
            layout: Layout {
                size,
                align,
                backend_repr,
            },
        }
    }
}
