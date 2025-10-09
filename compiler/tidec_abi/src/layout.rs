use crate::{
    size_and_align::{AbiAndPrefAlign, Size},
    target::AddressSpace,
};

#[derive(Debug, Clone, Copy)]
/// Represents a type along with its size and alignment information.
///
/// This is commonly used during codegen and layout computation to reason about
/// how values should be represented in memory on the target platform.
pub struct TyAndLayout<'ctx, T> {
    /// The type this layout refers to.
    ///
    /// This is usually a TIR type, but can be any type that has a size and alignment.
    pub ty: T,
    /// The layout information for the type, including size and alignment.
    pub layout: crate::Layout<'ctx>,
}

impl<'ctx, T> std::ops::Deref for TyAndLayout<'ctx, T> {
    type Target = crate::Layout<'ctx>;

    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Represents the layout of a type in the target architecture.
///
/// This struct contains the size, alignment, and backend representation
/// of a type, which is essential for code generation and memory layout decisions.
// TODO(bruzzone): Add fields and variants (tag union, struct, etc.).
pub struct Layout {
    /// The size of the type in bytes.
    pub size: Size,
    /// The ABI and preferred alignment of the type.
    pub align: AbiAndPrefAlign,
    /// `backend_repr` specifies how the value is represented to the codegen backend.
    ///
    /// This representation is independent of the type’s structural layout as described by
    /// `variants` and `fields`. For example, a type like `MyType<Result<isize, isize>>`
    /// may still use `ScalarPair` as its backend representation.
    ///
    /// Therefore, even when `backend_repr` is not `Memory`, you must still consider
    /// `fields` and `variants` to fully understand and access all parts of the layout.
    pub backend_repr: BackendRepr,
}

impl Layout {
    /// Returns true if the layout represents a zero-sized type.
    pub fn is_zst(&self) -> bool {
        match self.backend_repr {
            BackendRepr::Scalar(_) /* | BackendRepr::ScalarPair(_, _) */ => false,
            BackendRepr::Memory => self.size.bytes() == 0,
        }
    }

    pub fn is_bool(&self) -> bool {
        matches!(self.backend_repr, BackendRepr::Scalar(Primitive::U8))
    }

    pub fn is_immediate(&self) -> bool {
        match self.backend_repr {
            BackendRepr::Scalar(_)  => true,
            BackendRepr::Memory /* | BackendRepr::ScalarPair(_, _) */ => false,
        }
    }

    pub fn is_memory(&self) -> bool {
        matches!(self.backend_repr, BackendRepr::Memory)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Represents how values are passed to the backend during code generation.
///
/// This is *not* the same as the platform's ABI.
/// While the platform ABI may influence these choices, this enum primarily describes
/// the *syntactic form* used when emitting code — for example, whether a value is
/// passed as a scalar (like an integer or float) or as a memory reference.
///
/// Most codegen backends treat SSA values (e.g., scalars or vectors) differently from
/// values stored in memory. As a general rule, small values are best handled as scalars
/// or short vectors, while larger values are better represented as memory blobs.
///
/// Note: This representation does *not* guarantee how a value will be lowered to the
/// actual calling convention — that is determined separately by the ABI implementation.
pub enum BackendRepr {
    /// The value is represented as a scalar, such as an integer or float.
    Scalar(Primitive),
    /// The value is represented as a memory reference, such as a pointer or
    /// a reference to a struct or array.
    Memory,
    // Scalar pair, which is a pair of scalars. It is often used for
    // returning multiple values from a function. This allows the backend to
    // optimize the representation of multiple return values. Additionally,
    // it is used for "fat pointers", which are pointers that include extra
    // metadata, such as a pointer to a slice or a trait object. For example,
    // a slice `&str` is represented as a pair of a pointer to the data
    // and a length.
    // ScalarPair(Primitive, Primitive),
}

impl BackendRepr {
    /// Converts the `BackendRepr` to its corresponding `Primitive` type if it is a scalar.
    pub fn to_primitive(&self) -> Primitive {
        match self {
            BackendRepr::Scalar(p) => *p,
            // BackendRepr::ScalarPair(p1, p2) => Some((*p1, *p2)),
            BackendRepr::Memory => {
                panic!("Memory backend representation does not have a primitive type")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Represents primitive types that can be used in the backend representation.
pub enum Primitive {
    /// A signed integer type.
    I8,
    I16,
    I32,
    I64,
    I128,
    /// An unsigned integer type.
    U8,
    U16,
    U32,
    U64,
    U128,
    /// A floating-point type.
    F16,
    F32,
    F64,
    F128,
    /// A pointer type.
    Pointer(AddressSpace),
}

impl Primitive {
    pub fn is_signed_integer(&self) -> bool {
        matches!(
            self,
            Primitive::I8 | Primitive::I16 | Primitive::I32 | Primitive::I64 | Primitive::I128
        )
    }

    pub fn is_unsigned_integer(&self) -> bool {
        matches!(
            self,
            Primitive::U8 | Primitive::U16 | Primitive::U32 | Primitive::U64 | Primitive::U128
        )
    }

    pub fn is_integer(&self) -> bool {
        self.is_signed_integer() || self.is_unsigned_integer()
    }

    pub fn is_floating_point(&self) -> bool {
        matches!(
            self,
            Primitive::F16 | Primitive::F32 | Primitive::F64 | Primitive::F128
        )
    }

    pub fn is_pointer(&self) -> bool {
        matches!(self, Primitive::Pointer(_))
    }
}
