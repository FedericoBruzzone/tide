use std::num::NonZero;
use tidec_abi::size_and_align::Size;
use tidec_utils::idx::Idx;
use crate::{ctx::TirCtx, TirTy};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// A `Local` variable in the TIR.
///
/// `Local` acts as an index into the set of local variables declared within a function or
/// basic block. These variables include user-declared bindings, temporaries created
/// during compilation, and compiler-generated variables such as those for intermediate
/// values or storage management.
///
/// The index (`usize`) identifies the local variable uniquely within its context.
/// The zeroth local (`Local(0)`) often refers to the return place of a function.
pub struct Local(usize);
pub const RETURN_LOCAL: Local = Local(0);

impl Local {
    pub fn next(&self) -> Local {
        Local(self.0 + 1)
    }
}

impl From<Local> for Place {
    fn from(val: Local) -> Self {
        Place {
            local: val,
            projection: vec![],
        }
    }
}

#[derive(Debug, Clone)]
/// Represents a memory location (or "place") within TIR that can be used
/// as the target of assignments or the source of loads.
///
/// A `Place` consists of:
/// - A `local`: the base variable or temporary (identified by a `Local`)
/// - A `projection`: a sequence of projections used to navigate through the
///   structure of compound types (e.g., fields, dereferences, array indexing).
///
/// For example, in the Rust expression `x.0.y`, the base local would be `x`, and the
/// projection would include field accessors to reach `.0` and `.y`.
///
/// `Place`s are used in TIR to abstract over memory references in a type-safe and
/// structured manner, allowing the compiler to track aliasing, lifetimes,
/// and optimize memory access.
///
/// For example,
/// ```rust
/// let mut x = 5;          // `x` is a place
/// let y = &mut x;      // `y` is a place (pointer to x)
/// struct S { a: i32 }
/// let s = S { a: 10 };
/// let _ = s.a;                 // `s.a` is a place
/// ```
pub struct Place {
    /// The base local variable from which this place starts.
    pub local: Local,

    /// A (possibly empty) list of projections representing access to subparts
    /// of the base local, such as fields or dereferenced pointers.
    pub projection: Vec<Projection>,
}

impl Place {
    #[inline]
    pub fn try_local(&self) -> Option<Local> {
        if self.projection.is_empty() {
            Some(self.local)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
/// Represents a single step in a `Place` projection path.
///
/// A `Projection` allows navigation into more complex data structures
/// from a base `Local`. Multiple projections can be chained to model
/// deeply nested memory accesses.
///
/// Common projection types include:
/// - Field access (e.g., `.field`)
/// - Dereferencing a pointer (e.g., `*p`)
/// - Indexing into an array or slice (e.g., `[i]`)
///
/// TODO: This enum is currently a placeholder and should be extended with
/// specific variants such as `Field`, `Deref`, `Index`, etc.
pub enum Projection {
    Todo,
}


#[derive(Debug, Clone)]
/// Represents a right-hand side (RValue) in TIR during code generation.
///
/// An `RValue` is something that can be **evaluated to produce a value**.  
/// It corresponds to expressions on the right-hand side of assignments or
/// the values returned by function calls in source code.
///
/// For example,
/// ```rust
/// let x = 5;
/// let y = x + 1;     // `x + 1` is an operand
/// let z = 42;        // `42` is an operand
/// let s = "hi";      // `"hi"` is an operand (a fat pointer and length)
/// ```
pub enum RValue<'ctx> {
    /// An operand value.
    Operand(Operand<'ctx>),
    /// A unary operation applied to an operand. The operand's type is preserved.
    ///
    /// For example, negation (`-x`), logical not (`!x`), or bitwise not (`~x`).
    UnaryOp(UnaryOp, Operand<'ctx>),
    /// A binary operation applied to two operands. The result type is the same as the operands' type.
    ///
    /// For example, addition (`x + y`), subtraction (`x - y`), multiplication (`x * y`), etc.
    BinaryOp(BinaryOp, Operand<'ctx>, Operand<'ctx>),
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    /// Artihmetic positive (no-op).
    Pos,
    /// Arithmetic negation.
    Neg,
}

#[derive(Debug, Clone)]
pub enum BinaryOp {
    /// Addition.
    Add,
    /// Addition but with UB on overflow (Integer only)
    AddUnchecked,
    /// Subtraction.
    Sub,
    /// Subtraction but with UB on overflow (Integer only).
    SubUnchecked,
    /// Multiplication.
    Mul,
    /// Multiplication but with UB on overflow (Integer only).
    MulUnchecked,
    /// Division.
    ///
    /// For integer types, division by zero is UB.
    /// `MIN / -1` is UB for signed integers because it overflows.
    ///
    /// Floating-point division by zero is safe, and does not need guards.
    // TODO(bruzzone): tide might add checks for UBs in Div.
    Div,
}

impl BinaryOp {
    /// Returns the resulting type of the binary operation, which is the same as the operand types.
    pub fn ty<'ctx>(&self, tir_ctx: &TirCtx<'ctx>, lhs_ty: TirTy<'ctx>, rhs_ty: TirTy<'ctx>) -> TirTy<'ctx> {
        let _ = tir_ctx;
        let _ = rhs_ty;
        match self {
            BinaryOp::Add
            | BinaryOp::AddUnchecked
            | BinaryOp::Sub
            | BinaryOp::SubUnchecked
            | BinaryOp::Mul
            | BinaryOp::MulUnchecked
            | BinaryOp::Div => lhs_ty,
        }
    }
}

#[derive(Debug, Clone)]
/// An operand in TIR.
/// Semantically, an operand is a value that can be used in expressions.
// TODO(bruzzone): consider to switch to `copy` and `move` semantic, instead of `use`
pub enum Operand<'ctx> {
    /// A place value.
    ///
    /// This represents a value located at a specific memory location.
    /// It can be used to read from or write to that location.
    /// For example, loading a value from a variable or a field of a struct.
    /// Currently, this is a placeholder and should be expanded with more details.
    Use(Place),
    /// A constant value.
    ///
    /// Wraps a `ConstOperand`, which represents a constant known at compile-time.
    /// This includes literals (`42`, `"hi"`), const functions, and other compile-time
    /// evaluable values.
    ///
    /// TODO: Consider separating this into a dedicated `Operand` enum with variants like
    /// `Const`, `Copy`, and `Move` for clarity and future extensibility.
    Const(ConstOperand<'ctx>),
}

#[derive(Debug, Clone, Copy)]
/// Semantically, a constant is already a value; it cannot change.
// TODO(bruzzone): Add more variants for different constant types.
pub enum ConstOperand<'ctx> {
    /// A constant value that can be used in the TIR.
    Value(ConstValue, TirTy<'ctx>),
}

impl<'ctx> ConstOperand<'ctx> {
    /// Returns the type of the constant operand.
    pub fn ty(&self) -> TirTy<'ctx> {
        match self {
            ConstOperand::Value(_, ty) => *ty,
        }
    }

    pub fn value(&self) -> ConstValue {
        match self {
            ConstOperand::Value(val, _) => *val,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// Represents a constant value.
// TODO(bruzzone): Add indirect variant. A value not representable by the other variants; needs to be stored in-memory.
// TODO(bruzzone): Add slice variant for strings, arrays, etc. We could use the `Invariant` variant
// to avoid this optimization.
pub enum ConstValue {
    /// A constant value that is a zero-sized type (ZST).
    ZST,
    /// A constant scalar value.
    /// The consts with this variant have typically a layout that is compatible with scalar types, such as integers, floats, or pointers. That is, the backend representation of the constant is a scalar value.
    Scalar(ConstScalar),
    // A value that cannot be represented directly by the other variants,
    // and thus must be stored in memory.
    //
    // This is used for constants such as strings, slices, and large or
    // aggregate values that do not fit into a single scalar or scalar pair.
    //
    // # Fields
    //
    // * [`alloc_id`] — An abstract identifier for the allocation backing
    //   this value. Unlike a real machine pointer, an [`AllocId`] refers
    //   to a constant allocation managed by the compiler. This indirection
    //   ensures that when a "raw constant" (which is basically just an
    //   `AllocId`) is turned into a [`ConstValue`] and later converted
    //   back, the identity of the original allocation is preserved.
    //
    // * [`offset`] — A byte offset into the referenced allocation. This
    //   allows an `Indirect` constant to represent a subslice or substring
    //   within a larger allocation, rather than always starting at the
    //   beginning. For example, a slice `&arr[3..]` would use the same
    //   `AllocId` as `arr`, but with a nonzero offset.
    //
    // # Notes
    //
    // * This variant must **not** be used for scalars or zero-sized types
    //   (those are handled by other variants).
    // * It is perfectly valid, however, for `&str` or other slice types
    //   to be represented as `Indirect`.
    //
    // # Example
    //
    // ```rust
    // // For `const S: &str = "hi";`
    // // tidec creates a global allocation containing the bytes [104, 105],
    // // assigns it an `AllocId`, and represents `S` as:
    //
    // ConstValue::Indirect {
    //     alloc_id: <id of "hi">,
    //     offset: 0,
    // }
    // ```
    // Indirect {
    //     /// The backing memory of the value. This may cover more than just
    //     /// the bytes of the current value, e.g. when pointing into a larger
    //     /// `ConstValue`. The `AllocId` is an abstract identifier for
    //     /// the allocation.
    //     alloc_id: AllocId,
    //     /// The byte offset into the referenced allocation.
    //     offset: u64,
    // },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// Represents a constant scalar value.
// TODO(bruzzone): Add pointer variant for constants that are pointers to other constants or memory locations.
pub enum ConstScalar {
    /// Raw byte representation of the constant.
    Value(RawScalarValue),
    // Represents a pointer in the compiler’s abstract memory model.
    //
    // A `Pointer` is not a raw machine address. Instead, it encodes a
    // reference into tide's internal allocation map, allowing  to track provenance, validity,
    // and offsets safely.
    //
    // # Fields
    //
    // * `provenance: AllocId` — Identifies the allocation this pointer points to.
    //   This is an abstract ID that allows the compiler to distinguish between
    //   different memory blocks, even if their raw addresses are identical.
    //
    // * `offset: u64` — The byte offset from the start of the allocation.
    //   Together with `provenance`, this determines the exact location
    //   the pointer refers to.
    //
    // * `size: NonZeroU8` — The size of the pointer itself in bytes, typically
    //   4 on 32-bit targets or 8 on 64-bit targets. Storing this ensures
    //   that the pointer always knows its size, independent of target context.
    //
    // Note that `&str` and other slice types **should not** use this variant.
    // Instead, they should be represented as `ConstValue::Indirect`, which
    // can point to a sequence of bytes in memory.
    //
    // Do not interpret the internal `offset` or `provenance` as raw memory
    // addresses; instead, use the accessor methods provided by `Scalar` and
    // `ConstValue` for safe manipulation.
    // Pointer {
    //   /// The address this pointer points to.
    //   provenance: AllocId,
    //   /// The offset from the start of the allocation.
    //   offset: u64,
    //   /// The size of the pointer in bytes.
    //   size: NonZeroU8,
    // },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// A compact representation of the raw bytes of a scalar value.
///
/// This type is used in tide's value model (e.g. in [`Scalar`]) to represent
/// primitive runtime values such as integers, floats, and pointers. Unlike
/// general memory values, scalars always have a bounded size (`1–16` bytes),
/// which makes it possible to store them in a single `u128` together with
/// their size.
///
/// # Layout
///
/// This struct is marked as `#[repr(C, packed)]` so that its size is exactly
/// 17 bytes:
///
/// - [`data`] contains up to 16 bytes of the value, stored in the *low-order*
///   bytes of a `u128`.
/// - [`size`] specifies how many of those low-order bytes are actually part
///   of the value. The valid range is `1..=16`.
///
/// Packing is used to reduce padding so that this type can be embedded in
/// enums (like [`Scalar`]) without wasting space. Without packing, this type
/// would be padded up to 24 or 32 bytes, which would significantly increase
/// memory usage across the compiler.
///
/// # Invariants
///
/// * `size` is always nonzero (`1..=16`).
/// * Only the lowest `size` bytes of `data` are meaningful; the higher bytes
///   must be zeroed.
/// * Consumers must respect the declared size: reading more or fewer bytes
///   than `size` is undefined behavior.
///
/// # Example
///
/// ```rust
/// use std::num::NonZeroU8;
/// use tidec_tir::syntax::RawScalarValue;
///
/// // A 1-byte scalar (u8 = 127)
/// let small = RawScalarValue {
///     data: 127,
///     size: NonZeroU8::new(1).unwrap(),
/// };
///
/// // An 8-byte scalar (u64 = 0x12345678)
/// let big = RawScalarValue {
///     data: 0x12345678,
///     size: NonZeroU8::new(8).unwrap(),
/// };
/// ```
#[repr(C, packed)]
pub struct RawScalarValue {
    /// The first `size` bytes of this `u128` represent the scalar's raw value.
    ///
    /// Only the low-order `size` bytes are valid. All remaining bytes must be
    /// zero. For example, the `u32` value `0xDEADBEEF` would be stored as:
    ///
    /// ```ignore
    /// data = 0x00000000DEADBEEF;
    /// size = 4
    /// ```
    pub data: u128,
    /// The number of valid low-order bytes in [`data`].
    ///
    /// Always in the range `1..=16`. This cannot be zero.
    pub size: NonZero<u8>,
}

impl RawScalarValue {
    /// Attempts to convert this raw scalar value to a bit representation of the specified size.
    /// It fails if the sizes do not match. That is, if `self.size` is not equal to `taget_size`.
    pub fn to_bits(&self, taget_size: Size) -> u128 {
        assert_ne!(taget_size.bytes() as u8, 0);
        if self.size.get() != taget_size.bytes() as u8 {
            panic!(
                "Mismatched sizes: expected {}, got {}",
                taget_size.bytes(),
                self.size
            );
        }
        self.data
    }
}

#[derive(Debug, Clone)]
pub struct LocalData<'ctx> {
    pub ty: TirTy<'ctx>,
    pub mutable: bool,
}

#[derive(Debug, Clone)]
/// A statement in a basic block.
///
/// A statement is an operation that does not transfer control to another block (i.e., it is not a
/// terminator of a basic block). It is a part of the block's execution.
pub enum Statement<'ctx> {
    // An assignment statement. We use a Box to keep the size small.
    Assign(Box<(Place, RValue<'ctx>)>),
}

#[derive(Debug, Clone)]
/// The terminator of a basic block.
///
/// The terminator of a basic block is the last statement of the block.
/// It is an operation that ends the block and transfers control to another block.
pub enum Terminator<'ctx> {
    /// Returns from the function.
    ///
    /// The semantics of return is, at least, assign the value in the current
    /// return place (`Local(0)`) to the place specified, via a `Call` terminator
    /// by the caller.
    Return,
    /// A function call.
    ///
    /// This terminator represents a function call, which transfers control to the
    /// called function and, upon return, continues execution at a specified basic block.
    Call {
        /// The function to call.
        func: Operand<'ctx>,
        /// The arguments to the function.
        args: Vec<Operand<'ctx>>,
        /// The destination for the return value.
        destination: Place,
        /// The basic block to continue execution at after the call.
        target: BasicBlock,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BasicBlock(usize);
pub const ENTRY_BLOCK: BasicBlock = BasicBlock(0);

#[derive(Debug, Clone)]
/// The data of a basic block.
///
/// A basic block is a sequence of statements that ends with a terminator.
/// The terminator is the last statement of the block and transfers control to another block.
pub struct BasicBlockData<'ctx> {
    pub statements: Vec<Statement<'ctx>>,
    pub terminator: Terminator<'ctx>,
}

////////// Trait implementations  //////////

impl Idx for BasicBlock {
    fn new(idx: usize) -> Self {
        BasicBlock(idx)
    }

    fn idx(&self) -> usize {
        self.0
    }

    fn incr(&mut self) {
        self.0 += 1;
    }

    fn incr_by(&mut self, by: usize) {
        self.0 += by;
    }
}

////////// Trait implementations  //////////

impl Idx for Local {
    fn new(idx: usize) -> Self {
        Local(idx)
    }

    fn idx(&self) -> usize {
        self.0
    }

    fn incr(&mut self) {
        self.0 += 1;
    }

    fn incr_by(&mut self, by: usize) {
        self.0 += by;
    }
}
