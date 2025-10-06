use crate::layout::TyAndLayout;

/// Describes the full application binary interface (ABI) of a function.
///
/// A function ABI specifies how each argument is passed to the backend
/// (e.g., in registers, via pointers, or ignored) and how the return
/// value is produced.
///
/// This struct is produced during lowering from Rust MIR types into
/// a form suitable for LLVM or another backend.
///
/// # Examples
///
/// A simple function `fn add(a: i32, b: i32) -> i32` might lower to:
///
/// ```ignore
/// FnAbi {
///     args: [
///         ArgAbi { layout: i32, mode: PassMode::Direct },
///         ArgAbi { layout: i32, mode: PassMode::Direct },
///     ],
///     ret: ArgAbi { layout: i32, mode: PassMode::Direct },
/// }
/// ```
///
/// In contrast, a function returning a large struct `fn foo() -> BigStruct`
/// may use `PassMode::Indirect` for the return value, indicating that the
/// caller allocates space and passes a hidden pointer where the result is stored.
pub struct FnAbi<'ctx, T> {
    /// The type, layout, and passing convention for each argument.
    pub args: Box<[ArgAbi<'ctx, T>]>,

    /// The type, layout, and passing convention for the return value.
    pub ret: ArgAbi<'ctx, T>,
}

/// Describes how a single argument or return value is represented
/// and passed according to the ABI.
///
/// Each argument has a memory layout (`TyAndLayout`) and a `PassMode`
/// describing how it is lowered to machine code.
pub struct ArgAbi<'ctx, T> {
    /// The memory layout of the argument or return value
    /// (size, alignment, and type information).
    pub layout: TyAndLayout<'ctx, T>,

    /// The convention for passing this value to/from the backend.
    pub mode: PassMode,
}

impl<'ctx, T> ArgAbi<'ctx, T> {
    pub fn new(layout: TyAndLayout<'ctx, T>, mode: PassMode) -> Self {
        ArgAbi { layout, mode }
    }
}

/// The possible ways in which an argument or return value
/// can be passed across the ABI boundary.
//
// TODO: pub struct ArgAttributes {
//     pub regular: ArgAttribute,
//     pub arg_ext: ArgExtension,
//     /// The minimum size of the pointee, guaranteed to be valid for the duration of the whole call
//     /// (corresponding to LLVM's dereferenceable_or_null attributes, i.e., it is okay for this to be
//     /// set on a null pointer, but all non-null pointers must be dereferenceable).
//     pub pointee_size: Size,
//     /// The minimum alignment of the pointee, if any.
//     pub pointee_align: Option<Align>,
// }
pub enum PassMode {
    /// The argument is ignored (e.g., a zero-sized type).
    Ignore,
    /// The argument is passed directly, typically in registers or
    /// as a plain immediate value.
    ///
    /// # Example
    /// A parameter of type `i32` is usually passed in a register
    /// as `PassMode::Direct`.
    // TODO(bruzzone): Consider adding more details to Direct, such as:
    // - `attrs`: Attributes like `signext`, `zeroext`, etc.
    Direct,
    /// The argument is passed indirectly, via a hidden pointer
    /// to memory allocated by the caller or callee.
    ///
    /// # Example
    /// A large struct parameter may be passed by reference instead
    /// of by value:
    ///
    /// ```ignore
    /// struct BigStruct([u8; 128]);
    ///
    /// fn foo(x: BigStruct); // `x` is passed as PassMode::Indirect
    /// ```
    // TODO(bruzzone): Consider adding more details to Indirect, such as:
    // - `attrs`: Attributes like `noalias`, `readonly`, etc.
    // - `meta_attrs`: Metadata attributes for optimization hints.
    // - `on_stack`: Whether the argument must be passed on the stack.
    Indirect,
}
