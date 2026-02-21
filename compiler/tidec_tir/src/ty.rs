use std::hash::Hash;
use tidec_utils::interner::Interner;

#[derive(Debug, Clone, Copy)]
pub enum TirTy<I: Interner> {
    // The `()` type. This is equivalent to a
    // zero-sized type or void in some languages.
    Unit,

    /// Boolean type. Represents a 1-bit truth value (`true` / `false`).
    ///
    /// At the LLVM level this maps to `i1`. At the ABI/layout level it is
    /// stored as a `U8` scalar (1 byte, same alignment as `u8`), but only
    /// the low bit is meaningful.
    ///
    /// Comparison operators (`Eq`, `Ne`, `Lt`, …) produce values of this type.
    Bool,

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

    /// A struct (product) type.
    ///
    /// Contains a list of field types stored as an interned type list.
    /// The `packed` flag indicates whether the struct uses packed layout
    /// (no inter-field padding for alignment).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // struct { i32, f64 }
    /// TirTy::Struct { fields: intern_type_list(&[i32_ty, f64_ty]), packed: false }
    /// ```
    Struct {
        /// The field types of the struct, stored as an interned type list.
        fields: I::TypeList,
        /// If `true`, the struct has packed layout (no alignment padding).
        packed: bool,
    },

    /// A fixed-size array type.
    ///
    /// Contains the element type and the number of elements.
    /// The element type must be sized.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // [i32; 4]
    /// TirTy::Array(i32_ty, 4)
    /// ```
    Array(I::Ty, u64),

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

    /// Returns `true` if this type is any integer type (signed or unsigned),
    /// excluding `Bool`.
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
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
        )
    }

    /// Returns `true` if this type is a raw pointer type.
    pub fn is_pointer(&self) -> bool {
        matches!(self, TirTy::RawPtr(_, _))
    }

    /// Returns `true` if this type is the boolean type.
    pub fn is_bool(&self) -> bool {
        matches!(self, TirTy::Bool)
    }

    /// Returns `true` if this type is the unit type (`()`).
    pub fn is_unit(&self) -> bool {
        matches!(self, TirTy::Unit)
    }

    /// Returns `true` if this type is a struct type.
    pub fn is_struct(&self) -> bool {
        matches!(self, TirTy::Struct { .. })
    }

    /// Returns `true` if this type is an array type.
    pub fn is_array(&self) -> bool {
        matches!(self, TirTy::Array(_, _))
    }

    /// This function returns true if the type is a sized type.
    /// That is, it has a known size at compile time.
    pub fn is_sized(&self) -> bool {
        match self {
            TirTy::Unit => true,
            TirTy::Bool => true,
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
            TirTy::Struct { .. } => true,
            TirTy::Array(_, _) => true,
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
            (TirTy::Unit, TirTy::Unit) => true,
            (TirTy::Bool, TirTy::Bool) => true,
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
            (
                TirTy::Struct {
                    fields: f1,
                    packed: p1,
                },
                TirTy::Struct {
                    fields: f2,
                    packed: p2,
                },
            ) => f1 == f2 && p1 == p2,
            (TirTy::Array(ty1, len1), TirTy::Array(ty2, len2)) => ty1 == ty2 && len1 == len2,
            (TirTy::Metadata, TirTy::Metadata) => true,
            _ => false,
        }
    }
}

impl<I: Interner> Eq for TirTy<I> {}

impl<I: Interner> Hash for TirTy<I> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            TirTy::Unit => 0.hash(state),
            TirTy::Bool => 1.hash(state),
            TirTy::I8 => 2.hash(state),
            TirTy::I16 => 3.hash(state),
            TirTy::I32 => 4.hash(state),
            TirTy::I64 => 5.hash(state),
            TirTy::I128 => 6.hash(state),
            TirTy::U8 => 7.hash(state),
            TirTy::U16 => 8.hash(state),
            TirTy::U32 => 9.hash(state),
            TirTy::U64 => 10.hash(state),
            TirTy::U128 => 11.hash(state),
            TirTy::F16 => 12.hash(state),
            TirTy::F32 => 13.hash(state),
            TirTy::F64 => 14.hash(state),
            TirTy::F128 => 15.hash(state),
            TirTy::RawPtr(ty, mutability) => {
                16.hash(state);
                ty.hash(state);
                mutability.hash(state);
            }
            TirTy::Struct { fields, packed } => {
                17.hash(state);
                fields.hash(state);
                packed.hash(state);
            }
            TirTy::Array(ty, len) => {
                18.hash(state);
                ty.hash(state);
                len.hash(state);
            }
            TirTy::Metadata => 19.hash(state),
        }
    }
}
