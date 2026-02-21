use crate::{ctx::TirCtx, ty, TirTy, TirTypeList};
use tidec_abi::{
    layout::{self, BackendRepr, Primitive},
    size_and_align::{AbiAndPrefAlign, Size},
    target::AddressSpace,
    Layout,
};

pub struct LayoutCtx<'ctx> {
    tir_ctx: TirCtx<'ctx>,
}

impl<'ctx> LayoutCtx<'ctx> {
    // It accepts the `TirCtx` because it contains the `TargetDataLayout`.
    pub fn new(tir_ctx: TirCtx<'ctx>) -> Self {
        LayoutCtx { tir_ctx }
    }

    /// Computes the layout for a given type. We should cache the results
    /// to avoid recomputing the layout for the same type multiple times.
    pub fn compute_layout(&self, ty: TirTy<'ctx>) -> Layout<'ctx> {
        let data_layout = &self.tir_ctx.target().data_layout;

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

        let (size, align, backend_repr) = match &**ty {
            ty::TirTy::Unit => {
                // Unit / void is a zero-sized type.
                // Size is 0 bytes, alignment is 1 byte, backend representation is Memory
                // (ZSTs are always Memory because they have no scalar value).
                (
                    Size::ZERO,
                    AbiAndPrefAlign::new(1, 1),
                    BackendRepr::Memory,
                )
            }
            ty::TirTy::Bool => {
                // Bool is stored as a U8 scalar (1 byte) but only the low bit
                // is meaningful. At the LLVM level this maps to `i1`.
                scalar(Primitive::U8)
            }
            ty::TirTy::I8 => scalar(Primitive::I8),
            ty::TirTy::I16 => scalar(Primitive::I16),
            ty::TirTy::I32 => scalar(Primitive::I32),
            ty::TirTy::I64 => scalar(Primitive::I64),
            ty::TirTy::I128 => scalar(Primitive::I128),
            ty::TirTy::U8 => scalar(Primitive::U8),
            ty::TirTy::U16 => scalar(Primitive::U16),
            ty::TirTy::U32 => scalar(Primitive::U32),
            ty::TirTy::U64 => scalar(Primitive::U64),
            ty::TirTy::U128 => scalar(Primitive::U128),
            ty::TirTy::F16 => scalar(Primitive::F16),
            ty::TirTy::F32 => scalar(Primitive::F32),
            ty::TirTy::F64 => scalar(Primitive::F64),
            ty::TirTy::F128 => scalar(Primitive::F128),
            ty::TirTy::RawPtr(ref pointee, _) => {
                // We ignore the backend representation of the pointee type for now. This is because
                // we are only interested in the pointer type itself, which has a fixed size and alignment
                // regardless of the pointee type. However, in the future, we might want to consider
                // the pointee type for more advanced optimizations or analyses.
                let (size, align, _) = scalar(Primitive::Pointer(AddressSpace::DATA));

                if pointee.is_sized() {
                    (size, align, BackendRepr::Scalar(Primitive::Pointer(AddressSpace::DATA)))
                } else {
                    unimplemented!("Layout computation for unsized pointee types is not yet supported.")
                }
            }
            // TirTy::FnPty { param_tys, ret_ty } => {
            //     todo!()
            // }
            // TODO: Implement layout computation for Metadata types (e.g., for unsized types or trait objects).
            // Metadata represents type information for unsized types (such as slices or trait objects),
            // which require special handling for their layout. Support for this will be added in a future release.
            ty::TirTy::Metadata => unimplemented!("Layout computation for TirTy::Metadata (used for unsized types/trait objects) is not yet supported. See TODO comment for details."),
            ty::TirTy::Struct { fields, packed } => {
                return self.compute_struct_layout(fields, *packed);
            }
            ty::TirTy::Array(element_ty, count) => {
                return self.compute_array_layout(*element_ty, *count);
            }
        };

        self.tir_ctx.intern_layout(layout::Layout {
            size,
            align,
            backend_repr,
        })
    }

    /// Compute the layout for a struct type.
    ///
    /// Field offsets are computed using C-style struct layout rules:
    /// each field is placed at the first offset that satisfies its alignment
    /// requirement, with padding inserted as needed. The total size is then
    /// rounded up to the struct's overall alignment.
    ///
    /// If `packed` is `true`, no alignment padding is inserted between fields
    /// and the struct's overall alignment is 1.
    fn compute_struct_layout(&self, fields: &TirTypeList<'ctx>, packed: bool) -> Layout<'ctx> {
        let field_types = fields.as_slice();

        if field_types.is_empty() {
            // Empty struct is a ZST.
            return self.tir_ctx.intern_layout(layout::Layout {
                size: Size::ZERO,
                align: AbiAndPrefAlign::new(1, 1),
                backend_repr: BackendRepr::Memory,
            });
        }

        let mut struct_size: u64 = 0;
        let mut struct_align: u64 = 1;

        for field_ty in field_types {
            let field_layout = self.compute_layout(*field_ty);

            let field_align = if packed {
                1
            } else {
                field_layout.align.abi.bytes()
            };

            // Align the current offset to the field's alignment.
            if field_align > 0 {
                struct_size = (struct_size + field_align - 1) & !(field_align - 1);
            }

            // Advance past this field.
            struct_size += field_layout.size.bytes();

            // Track the maximum alignment.
            if field_align > struct_align {
                struct_align = field_align;
            }
        }

        // If packed, struct alignment is 1. Otherwise, use the max field alignment.
        let final_align = if packed { 1 } else { struct_align };

        // Round the total size up to the struct's alignment.
        if final_align > 0 {
            struct_size = (struct_size + final_align - 1) & !(final_align - 1);
        }

        self.tir_ctx.intern_layout(layout::Layout {
            size: Size::from_bytes(struct_size),
            align: AbiAndPrefAlign::new(final_align, final_align),
            backend_repr: BackendRepr::Memory,
        })
    }

    /// Compute the layout for an array type.
    ///
    /// The layout is: `element_size` (rounded up to element alignment) × `count`.
    /// An array of zero elements is a ZST.
    fn compute_array_layout(&self, element_ty: TirTy<'ctx>, count: u64) -> Layout<'ctx> {
        let elem_layout = self.compute_layout(element_ty);

        if count == 0 {
            return self.tir_ctx.intern_layout(layout::Layout {
                size: Size::ZERO,
                align: elem_layout.align,
                backend_repr: BackendRepr::Memory,
            });
        }

        // Element stride is the element size rounded up to its alignment.
        let elem_align = elem_layout.align.abi.bytes();
        let elem_stride = if elem_align > 0 {
            (elem_layout.size.bytes() + elem_align - 1) & !(elem_align - 1)
        } else {
            elem_layout.size.bytes()
        };

        let total_size = elem_stride * count;

        self.tir_ctx.intern_layout(layout::Layout {
            size: Size::from_bytes(total_size),
            align: elem_layout.align,
            backend_repr: BackendRepr::Memory,
        })
    }
}
