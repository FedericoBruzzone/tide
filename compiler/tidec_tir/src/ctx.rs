use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    collections::HashSet,
    hash::Hash,
    ops::Deref,
    ptr::NonNull,
};

use crate::{layout_ctx::LayoutCtx, ty, TirTy};
use tidec_abi::{
    layout::{self, TyAndLayout},
    target::{BackendKind, TirTarget},
    Layout,
};
use tidec_utils::interner::{Interned, Interner};

#[derive(Debug, Clone, Copy)]
pub enum EmitKind {
    Assembly,
    Object,
    LlvmIr,
}

#[derive(Debug, Clone, Copy)]
pub struct TirArgs {
    pub emit_kind: EmitKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// A pointer to a value allocated in an arena.
///
/// This is a thin wrapper around a reference to a value allocated in an arena.
/// It is used to indicate that the value is allocated in an arena, and should
/// not be deallocated manually.
pub struct ArenaPrt<'ctx, T: Sized>(&'ctx T);

// Allow borrowing the underlying value so InternedSet<T> can accept an R = underlying type.
impl<'ctx, T> Borrow<T> for ArenaPrt<'ctx, T> {
    fn borrow(&self) -> &T {
        self.0
    }
}

#[derive(Debug, Clone)]
/// A chunk of memory allocated in the arena.
///
/// This is used to store multiple values in a single allocation, to reduce
/// the overhead of multiple allocations. Each chunk is a contiguous block of
/// memory that can hold multiple values of type `T`.
pub struct ArenaChunk<T = u8> {
    _mem: NonNull<[T]>,
}

#[derive(Debug, Clone)]
pub struct ArenaDropless {
    /// A pointer to the first free byte in the current chunk.
    start: Cell<*mut u8>,

    /// A pointer to the end of the free space in the current chunk.
    end: Cell<*mut u8>,

    /// The chunks of memory allocated in the arena.
    inner: RefCell<Vec<ArenaChunk>>,
}

impl ArenaDropless {
    /// Allocates a new value in the arena, returning a pointer to it.
    ///
    /// This function is safe to call, as long as the value is `Sized`.
    /// The caller must ensure that the value is not dropped manually,
    /// as it will be dropped when the arena is dropped.
    pub fn alloc<T: Sized>(&self, value: T) -> &T {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();

        // Ensure we have enough space in the current chunk.
        if unsafe { self.start.get().add(size) } > self.end.get() {
            // Not enough space, allocate a new chunk.
            let chunk_size = std::cmp::max(1024, size + align);
            let layout = std::alloc::Layout::from_size_align(chunk_size, align).unwrap();
            let ptr = unsafe { std::alloc::alloc(layout) };
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            let chunk = ArenaChunk {
                _mem: NonNull::slice_from_raw_parts(NonNull::new(ptr).unwrap(), chunk_size),
            };
            self.inner.borrow_mut().push(chunk);
            self.start.set(ptr);
            self.end.set(unsafe { ptr.add(chunk_size) });
        }

        // Allocate the value in the current chunk.
        let ptr = self.start.get() as *mut T;
        unsafe {
            ptr.write(value);
        }
        self.start.set(unsafe { self.start.get().add(size) });

        unsafe { &*ptr }
    }
}

#[derive(Debug, Clone)]
/// An arena for allocating TIR values.
pub struct TirArena<'ctx> {
    // types: Vec<Box<ty::TirTy<TirCtx<'ctx>>>>,
    /// We use a dropless arena because TIR types do not need to be dropped.
    /// This avoids the overhead of running destructors when the arena is dropped.
    /// Additionally, since TIR types are immutable after creation, we do not need
    /// to worry about memory leaks.
    dropless: ArenaDropless,

    /// The lifetime marker for the arena.
    /// This ensures that the arena lives as long as the context that uses it.
    _marker: std::marker::PhantomData<&'ctx ()>,
}

impl<'ctx> Deref for TirArena<'ctx> {
    type Target = ArenaDropless;

    fn deref(&self) -> &Self::Target {
        &self.dropless
    }
}

impl<'ctx> Default for TirArena<'ctx> {
    fn default() -> Self {
        Self {
            dropless: ArenaDropless {
                start: Cell::new(std::ptr::null_mut()),
                end: Cell::new(std::ptr::null_mut()),
                inner: RefCell::new(Vec::new()),
            },
            _marker: std::marker::PhantomData,
        }
    }
}

#[derive(Debug, Clone)]
/// A set of interned values of type `T`.
///
/// We need to use a `RefCell` here because we want to mutate the set
/// even when we have a shared reference to the `InternedSet`. That is,
/// internal mutability is required.
pub struct InternedSet<T: Sized + Eq + std::hash::Hash>(RefCell<HashSet<T>>);

impl<T: Sized + Clone + Copy + Eq + std::hash::Hash> InternedSet<T> {
    pub fn intern<R>(&self, value: R, intern_in_arena: impl FnOnce(R) -> T) -> T
    where
        T: Borrow<R>,
        R: Hash + Eq,
    {
        let set = &self.0;
        
        // Check for existing value, and let the immutable borrow drop immediately
        let existing = {
            let set_ref = set.borrow(); // Immutable borrow starts here
            set_ref.get(value.borrow()).copied() // .copied() is needed because existing is 'T: Copy'
        }; // Immutable borrow ends here when `set_ref` goes out of scope

        if let Some(existing_value) = existing {
            // If it exists, return the copied value. No borrow is active now.
            existing_value
        } else {
            // If it doesn't exist, we can now safely take a mutable borrow.
            let new = intern_in_arena(value);
            set.borrow_mut().insert(new); // Mutable borrow starts and ends here
            new
        }
    }
}

#[derive(Debug, Clone)]
/// The context for all interned entities in TIR.
///
/// It contains an arena for interning all TIR types and layouts, as well as
/// other cacheable information.
///
/// Note that InternedSets store arena pointers. This ensures that the
/// interned values live as long as the arena, and are not deallocated
/// prematurely. Additionally, to compare interned values, we only need to
/// compare their pointers, which is efficient.
pub struct InternCtx<'ctx> {
    /// The arena for allocating TIR types, layouts, and other interned entities.
    arena: &'ctx TirArena<'ctx>,
    /// A set of all interned TIR types.
    types: InternedSet<ArenaPrt<'ctx, ty::TirTy<TirCtx<'ctx>>>>,
}

impl<'ctx> InternCtx<'ctx> {
    pub fn new(arena: &'ctx TirArena<'ctx>) -> Self {
        Self {
            arena,
            types: InternedSet(RefCell::new(HashSet::new())),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TirCtx<'ctx> {
    target: &'ctx TirTarget,
    arguments: &'ctx TirArgs,

    intern_ctx: &'ctx InternCtx<'ctx>,
    // TODO(bruzzone): here we should have, other then an arena, also a HashMap from DefId
    // to the body of the function.
}

impl<'ctx> TirCtx<'ctx> {
    pub fn new(
        target: &'ctx TirTarget,
        arguments: &'ctx TirArgs,
        intern_ctx: &'ctx InternCtx<'ctx>,
    ) -> Self {
        Self {
            target,
            arguments,
            intern_ctx,
        }
    }

    pub fn target(&self) -> &TirTarget {
        &self.target
    }

    pub fn layout_of(self, ty: TirTy<'ctx>) -> TyAndLayout<'ctx, TirTy<'ctx>> {
        let layout_ctx = LayoutCtx::new(self);
        let layout = layout_ctx.compute_layout(ty);
        TyAndLayout { ty, layout }
    }

    pub fn backend_kind(&self) -> &BackendKind {
        &self.target.codegen_backend
    }

    pub fn emit_kind(&self) -> &EmitKind {
        &self.arguments.emit_kind
    }

    // ===== Direct inter =====
    pub fn intern_layout(&self, _layout: layout::Layout) -> Layout<'ctx> {
        todo!()
    }

    pub fn intern_ty(&self, ty: ty::TirTy<TirCtx<'ctx>>) -> TirTy<'ctx> {
        TirTy(Interned::new(
            self.intern_ctx
                .types
                .intern(ty, |ty: ty::TirTy<TirCtx<'ctx>>| {
                    ArenaPrt(self.intern_ctx.arena.alloc(ty))
                })
                .0,
        ))
    }
}

impl<'ctx> Interner for TirCtx<'ctx> {
    type Ty = TirTy<'ctx>;
}
