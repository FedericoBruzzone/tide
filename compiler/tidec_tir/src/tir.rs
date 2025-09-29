use crate::{
    basic_blocks::{BasicBlock, BasicBlockData},
    layout_ctx::LayoutCtx,
    syntax::{Body, Local, LocalData, TirTy},
    visitor::Visitor,
};
use tidec_abi::{
    layout::TyAndLayout,
    target::{BackendKind, TirTarget},
};
use tidec_utils::index_vec::IdxVec;
use tracing::{debug, instrument};

#[derive(Eq, PartialEq, Hash, Clone, Copy)]
pub struct DefId(pub usize);

#[derive(Clone, Copy)]
/// Specifies the linkage of a symbol.
/// All Global Variables and Functions have one of the following types of linkage.
///
/// NOTE: A symbol with internal or private linkage must have default visibility.
/// NOTE: It is illegal for a global variable or function declaration to have any linkage type other than external or extern_weak.
pub enum Linkage {
    /// Global values with "private" linkage are only directly accessible by objects in
    /// the current module. In particular, linking code into a module with a private global
    /// value may cause the private to be renamed as necessary to avoid collisions. Because
    /// the symbol is private to the module, all references can be updated. This doesn't show
    /// up in any symbol table in the object file.
    Private,
    /// Similar to private, but the value shows as a local symbol (STB_LOCAL in the case of ELF)
    /// in the object file. This corresponds to the notion of the `static` keyword in C.
    Internal,
    /// Globals with "available_externally" linkage are never emitted into the object file corresponding
    /// to the backend (e.g., LLVM) module. From the linker's perspective, an available_externally global
    /// is equivalent to an external declaration. They exist to allow inlining and other optimizations to
    /// take place given knowledge of the definition of the global, which is known to be somewhere outside
    /// the module. Globals with available_externally linkage are allowed to be discarded at will, and allow
    /// inlining and other optimizations. This linkage type is only allowed on definitions, not declarations.
    AvailableExternally,
    /// Globals with "linkonce" linkage are merged with other globals of the same name when linkage occurs.
    /// This can be used to implement some forms of inline functions, templates, or other code which must be
    /// generated in each translation unit that uses it, but where the body may be overridden with a more
    /// definitive definition later. Unreferenced linkonce globals are allowed to be discarded. Note that
    /// linkonce linkage does not actually allow the optimizer to inline the body of this function into
    /// callers because it doesn't know if this definition of the function is the definitive definition within
    /// the program or whether it will be overridden by a stronger definition. To enable inlining and other
    /// optimizations, use "linkonce_odr" linkage.
    LinkOnce,
    /// "weak" linkage has the same merging semantics as linkonce linkage, except that unreferenced globals with
    /// weak linkage may not be discarded. This is used for globals that are declared "weak" in C source code.
    Weak,
    /// "common" linkage is most similar to "weak" linkage, but they are used for tentative definitions in C,
    /// such as "int X;" at global scope. Symbols with "common" linkage are merged in the same way as weak symbols,
    /// and they may not be deleted if unreferenced. common symbols may not have an explicit section, must have a
    /// zero initializer, and may not be marked 'constant'. Functions and aliases may not have common linkage.
    Common,
    /// "appending" linkage may only be applied to global variables of pointer to array type. When two global
    /// variables with appending linkage are linked together, the two global arrays are appended together.
    /// This is the LLVM, typesafe, equivalent of having the system linker append together "sections" with
    /// identical names when .o files are linked.
    ///
    /// LLVM SPECIFIC: Unfortunately this doesn't correspond to any feature in .o files, so it can only be used for variables
    /// like llvm.global_ctors which llvm interprets specially.
    Appending,
    /// The semantics of this linkage follow the ELF object file model: the symbol is weak until linked,
    /// if not linked, the symbol becomes null instead of being an undefined reference.
    ExternWeak,
    /// The odr suffix indicates that all globals defined with the given name are equivalent, along the lines
    /// of the C++ "one definition rule" ("ODR"). Informally, this means we can inline functions and fold loads
    /// of constants.
    /// Formally, use the following definition: when an odr function is called, one of the definitions is
    /// non-deterministically chosen to run. For odr variables, if any byte in the value is not equal in all
    /// initializers, that byte is a poison value. For aliases and ifuncs, apply the rule for the underlying function or variable.
    ///
    /// These linkage types are otherwise the same as their non-odr versions.
    LinkOnceODR,
    WeakODR,
    /// If none of the above identifiers are used, the global is externally visible, meaning that it participates
    /// in linkage and can be used to resolve external symbol references.
    External,
}

#[derive(Clone, Copy)]
/// Specifies the symbol visibility with regards to dynamic linking.
/// All Global Variables and Functions have one of the following visibility styles.
///
/// A symbol with internal or private linkage must have default visibility.
pub enum Visibility {
    /// On targets that use the ELF object file format, default visibility means that
    /// the declaration is visible to other modules and, in shared libraries, means that
    /// the declared entity may be overridden. On Darwin, default visibility means that the
    /// declaration is visible to other modules. On XCOFF, default visibility means no explicit
    /// visibility bit will be set and whether the symbol is visible (i.e "exported") to other
    /// modules depends primarily on export lists provided to the linker. Default visibility
    /// corresponds to "external linkage" in the language.
    Default,
    /// Two declarations of an object with hidden visibility refer to the same object if they
    /// are in the same shared object. Usually, hidden visibility indicates that the symbol will
    /// not be placed into the dynamic symbol table, so no other module (executable or shared library)
    /// can reference it directly.
    Hidden,
    // On ELF, protected visibility indicates that the symbol will be placed in the dynamic symbol table,
    // but that references within the defining module will bind to the local symbol. That is, the symbol
    // cannot be overridden by another module.
    Protected,
}

/// A user-callable item in TIR.
pub enum TirItemKind {
    /// A function.
    Function,
    /// A closure.
    Closure,
    /// A coroutine.
    Coroutine,
}

#[derive(Clone, Copy)]
/// Specifies the significance of a global value's address, used for enabling
/// optimizations related to constant merging and deduplication.
///
/// The `UnnamedAddress` enum is commonly used in compiler internals or
/// intermediate representations (e.g., LLVM IR) to indicate how the address of
/// a global variable or constant can be treated by the optimizer.
///
/// This information allows the backend to perform optimizations like merging
/// identical constants or emitting address-independent data.
pub enum UnnamedAddress {
    /// The address of the global value is significant and must not be merged
    /// with others. This is the most conservative option.
    None,
    /// The address of the global value is significant only within the current
    /// translation unit. The optimizer may merge local constants with the same
    /// content if they are not exposed externally.
    Local,
    /// The address is completely insignificant. This allows the optimizer to
    /// merge identical constants across translation units or even eliminate
    /// duplicates entirely.
    Global,
}

#[derive(Clone, Copy)]
/// The calling convention of a function.
///
/// The calling convention is a low-level detail that specifies how
/// arguments are passed to a function and how the return value is obtained.
/// It is important for the backend to know the calling convention in order
/// to generate the correct code for function calls and returns.
///
/// For more details, see the LLVM documentation on calling conventions:
/// https://llvm.org/docs/LangRef.html#call-conventions
pub enum CallConv {
    C = 0,
    Rust = 1, // Added by Tidec
    Fast = 8,
    Cold = 9,
    GHC = 10,
    HiPE = 11,
    AnyReg = 13,
    PreserveMost = 14,
    PreserveAll = 15,
    Swift = 16,
    CxxFastTls = 17,
    Tail = 18,
    CfguardCheck = 19,
    SwiftTail = 20,
    PreserveNone = 21,
    FirstTargetCC = 63, // TODO: In the LLVM documentation it was 64. Overlapping with the X86StdCall
    X86StdCall = 64,
    X86FastCall = 65,
    ArmApcs = 66,
    ArmAapcs = 67,
    ArmAapcsVfp = 68,
    Msp430Intr = 69,
    X86ThisCall = 70,
    PtxKernel = 71,
    PtxDevice = 72,
    SpirFunc = 75,
    SpirKernel = 76,
    IntelOclBi = 77,
    X86_64SysV = 78,
    Win64 = 79,
    X86VectorCall = 80,
    DummyHhvm = 81,
    DummyHhvmC = 82,
    X86Intr = 83,
    AvrIntr = 84,
    AvrSignal = 85,
    AvrBuiltin = 86,
    AmdgpuVs = 87,
    AmdgpuGs = 88,
    AmdgpuPs = 89,
    AmdgpuCs = 90,
    AmdgpuKernel = 91,
    X86RegCall = 92,
    AmdgpuHs = 93,
    Msp430Builtin = 94,
    AmdgpuLs = 95,
    AmdgpuEs = 96,
    Aarch64VectorCall = 97,
    Aarch64SveVectorCall = 98,
    WasmEmscriptenInvoke = 99,
    AmdgpuGfx = 100,
    M68kIntr = 101,
    Aarch64SmeAbiSupportRoutinesPreserveMostFromX0 = 102,
    Aarch64SmeAbiSupportRoutinesPreserveMostFromX2 = 103,
    AmdgpuCsChain = 104,
    AmdgpuCsChainPreserve = 105,
    M68kRtd = 106,
    GRAAL = 107,
    Arm64ecThunkX64 = 108,
    Arm64ecThunkNative = 109,
    RiscvVectorCall = 110,
    Aarch64SmeAbiSupportRoutinesPreserveMostFromX1 = 111,
    MaxID = 1023,
}

/// The kind of a TIR body.
// TODO(bruzzone): add other kinds of body; e.g. virtual function, fn pointer, etc.
// See: rustc_middle::ty::InstanceKind
pub enum TirBodyKind {
    Item(TirItemKind),
}

/// The metadata of a TIR body (function).
pub struct TirBodyMetadata {
    /// The definition ID of the function.
    pub def_id: DefId,
    /// The name of the function.
    /// It aims to be the `symbol name` for the backend purpose.
    pub name: String,
    /// The kind of the body.
    pub kind: TirBodyKind,
    /// If the function should be inlined.
    pub inlined: bool,
    /// The linkage of the function.
    pub linkage: Linkage,
    /// The visibility of the function.
    pub visibility: Visibility,
    /// The unnamed address of the function.
    pub unnamed_address: UnnamedAddress,
    /// The calling convention of the function.
    pub call_conv: CallConv,
}

/// The body of a function in TIR. A body could be a function, a closure, a coroutine, etc.
/// A body is expected to be monomorphized and specialized, that is, when generic parameters are
/// involved, each instantiation of the generics should have its own body.
///
/// Semantically, a body is a portion of code that constitutes a complete unit of execution.
pub struct TirBody {
    /// The metadata of the function.
    // TODO(bruzzone): consider to detach the metadata from the body
    pub metadata: TirBodyMetadata,

    /// The locals for return value and arguments of the function.
    /// The first local is the return value, and the rest are the arguments.
    pub ret_and_args: IdxVec<Local, LocalData>,

    /// The rest of the locals.
    pub locals: IdxVec<Local, LocalData>,

    /// The basic blocks of the function.
    pub basic_blocks: IdxVec<BasicBlock, BasicBlockData>,
}

impl TirBody {
    pub fn visit<'tir, V>(&'tir self, visitor: &mut V)
    where
        V: Visitor<'tir>,
    {
        visitor.visit_body(self)
    }
}

/// The metadata of a TIR unit (module).
pub struct TirUnitMetadata {
    pub unit_name: String,
}

/// The TIR unit (module).
pub struct TirUnit {
    /// The metadata of the unit.
    pub metadata: TirUnitMetadata,

    /// The functions in the unit.
    pub bodies: IdxVec<Body, TirBody>,
}

#[derive(Debug)]
/// The kind of code to emit.
pub enum EmitKind {
    Object,
    Assembly,
    LlvmIr,
}

#[derive(Debug)]
/// The arguments for TIR type context. Usually provided by the user.
pub struct TirArgs {
    pub emit_kind: EmitKind,
    // TODO(bruzzone): add more arguments here
}

#[derive(Debug)]
pub struct TirCtx {
    target: TirTarget,
    arguments: TirArgs,
    // TODO(bruzzone): here we should have, other then an arena, also a HashMap from DefId
    // to the body of the function.
}

impl TirCtx {
    #[instrument]
    pub fn new(codegen_backend: BackendKind, emit_kind: EmitKind) -> Self {
        let target = TirTarget::new(codegen_backend);
        let arguments = TirArgs { emit_kind };
        let ctx = TirCtx { target, arguments };
        debug!("TirTyCtx created: {:?}", ctx);
        ctx
    }

    pub fn target(&self) -> &TirTarget {
        &self.target
    }

    pub fn layout_of(&self, ty: TirTy) -> TyAndLayout<TirTy> {
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
