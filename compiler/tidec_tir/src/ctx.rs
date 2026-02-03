use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    hash::Hash,
    ops::Deref,
    ptr::NonNull,
};

use crate::{
    alloc::{AllocId, Allocation, GlobalAlloc},
    body::DefId,
    layout_ctx::LayoutCtx,
    ty, TirAllocation, TirTy,
};
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
    Executable,
    LlvmIr,
    LlvmBitcode,
}

#[derive(Debug, Clone, Copy)]
pub struct TirArgs {
    pub emit_kind: EmitKind,
}

#[derive(Debug)]
/// A pointer to a value allocated in an arena.
///
/// This is a thin wrapper around a reference to a value allocated in an arena.
/// It is used to indicate that the value is allocated in an arena, and should
/// not be deallocated manually.
///
/// `Eq` and `Hash` are implemented to compare by the underlying value, not pointer address.
/// This is required for proper deduplication in `InternedSet`.
///
/// Note: `ArenaPrt` is always `Copy` since it just holds a reference (`&'ctx T` is Copy).
/// We manually implement `Clone` and `Copy` to avoid the derive macro adding a `T: Copy` bound.
pub struct ArenaPrt<'ctx, T: Sized>(&'ctx T);

// Manually implement Clone without requiring T: Clone
impl<T> Clone for ArenaPrt<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

// Manually implement Copy without requiring T: Copy
// This is safe because &'ctx T is always Copy
impl<T> Copy for ArenaPrt<'_, T> {}

// Implement PartialEq by comparing the underlying values.
impl<T: PartialEq> PartialEq for ArenaPrt<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Eq> Eq for ArenaPrt<'_, T> {}

// Implement Hash by hashing the underlying value.
impl<T: Hash> Hash for ArenaPrt<'_, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

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

impl<T: Sized + Eq + std::hash::Hash> InternedSet<T> {
    /// Create a new empty interned set.
    pub fn new() -> Self {
        Self(RefCell::new(HashSet::new()))
    }
}

impl<T: Sized + Eq + std::hash::Hash> Default for InternedSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Sized + Copy + Eq + std::hash::Hash> InternedSet<T> {
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

#[derive(Debug)]
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
    /// A set of all interned layouts.
    layouts: InternedSet<ArenaPrt<'ctx, layout::Layout>>,
    /// A set of all interned allocations (for deduplication of identical allocations).
    allocations: InternedSet<ArenaPrt<'ctx, Allocation>>,
    /// Global allocation map for tracking allocations by ID.
    /// This maps AllocId to GlobalAlloc for lookup during codegen.
    alloc_map: GlobalAllocMap<'ctx>,
}

#[derive(Debug, Default)]
/// A map from allocation IDs to global allocations.
///
/// This is separate from `InternCtx` because it's not about deduplication/interning,
/// but about tracking allocations by their unique ID for codegen lookup.
pub struct GlobalAllocMap<'ctx> {
    /// Map from AllocId to GlobalAlloc for lookup during codegen.
    /// Uses RefCell for interior mutability.
    alloc_id_map: RefCell<HashMap<AllocId, GlobalAlloc<'ctx>>>,
}

impl<'ctx> GlobalAllocMap<'ctx> {
    /// Create a new empty allocation map.
    pub fn new() -> Self {
        Self {
            alloc_id_map: RefCell::new(HashMap::new()),
        }
    }

    /// Insert a global allocation and return its AllocId.
    pub fn insert(&self, alloc: GlobalAlloc<'ctx>) -> AllocId {
        let id = AllocId::new();
        self.alloc_id_map.borrow_mut().insert(id, alloc);
        id
    }

    /// Get a global allocation by its ID.
    pub fn get(&self, id: AllocId) -> Option<GlobalAlloc<'ctx>> {
        self.alloc_id_map.borrow().get(&id).copied()
    }

    /// Get a global allocation by its ID, panicking if not found.
    pub fn get_unwrap(&self, id: AllocId) -> GlobalAlloc<'ctx> {
        self.alloc_id_map
            .borrow()
            .get(&id)
            .copied()
            .unwrap_or_else(|| panic!("unknown allocation ID: {:?}", id))
    }

    /// Iterate over all global allocations.
    /// Returns a Vec to avoid holding the RefCell borrow.
    pub fn iter(&self) -> Vec<(AllocId, GlobalAlloc<'ctx>)> {
        self.alloc_id_map
            .borrow()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect()
    }
}

impl<'ctx> InternCtx<'ctx> {
    pub fn new(arena: &'ctx TirArena<'ctx>) -> Self {
        Self {
            arena,
            types: Default::default(),
            layouts: Default::default(),
            allocations: Default::default(),
            alloc_map: GlobalAllocMap::new(),
        }
    }

    /// Intern an allocation, returning an interned `TirAllocation`.
    /// If an identical allocation already exists, returns the existing one.
    pub fn intern_allocation(&self, alloc: Allocation) -> TirAllocation<'ctx> {
        let interned_ref = self
            .allocations
            .intern(alloc, |alloc| ArenaPrt(self.arena.alloc(alloc)))
            .0;
        TirAllocation(tidec_utils::interner::Interned::new(interned_ref))
    }

    /// Get a reference to the global allocation map.
    pub fn alloc_map(&self) -> &GlobalAllocMap<'ctx> {
        &self.alloc_map
    }
}

#[derive(Debug, Clone, Copy)]
/// The main context for TIR compilation.
///
/// It holds references to the target, arguments, and intern context.
/// The global allocation map is accessed through the intern context.
/// It provides methods for computing layouts, interning types and layouts,
/// and accessing target-specific information.
pub struct TirCtx<'ctx> {
    target: &'ctx TirTarget,
    arguments: &'ctx TirArgs,
    intern_ctx: &'ctx InternCtx<'ctx>,
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
        self.target
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
    pub fn intern_layout(&self, layout: layout::Layout) -> Layout<'ctx> {
        Layout(Interned::new(
            self.intern_ctx
                .layouts
                .intern(layout, |layout: layout::Layout| {
                    ArenaPrt(self.intern_ctx.arena.alloc(layout))
                })
                .0,
        ))
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

    // ===== Allocation interning =====

    /// Intern an allocation in the arena and return an interned `TirAllocation`.
    /// Identical allocations are deduplicated.
    pub fn intern_alloc(&self, alloc: Allocation) -> TirAllocation<'ctx> {
        self.intern_ctx.intern_allocation(alloc)
    }

    /// Intern a C-string (null-terminated) and register it as a memory allocation.
    /// Returns the `AllocId` for the string.
    /// Identical strings are deduplicated.
    pub fn intern_c_str(&self, s: &str) -> AllocId {
        let alloc = Allocation::from_c_str(s);
        let interned = self.intern_alloc(alloc);
        self.intern_ctx
            .alloc_map()
            .insert(GlobalAlloc::Memory(interned))
    }

    /// Intern raw bytes and register them as a memory allocation.
    /// Returns the `AllocId` for the bytes.
    /// Identical byte sequences are deduplicated.
    pub fn intern_bytes(&self, bytes: &[u8]) -> AllocId {
        let alloc = Allocation::from_bytes(bytes);
        let interned = self.intern_alloc(alloc);
        self.intern_ctx
            .alloc_map()
            .insert(GlobalAlloc::Memory(interned))
    }

    /// Register a function as a global allocation.
    /// Returns the `AllocId` for the function reference.
    pub fn intern_fn(&self, def_id: DefId) -> AllocId {
        self.intern_ctx
            .alloc_map()
            .insert(GlobalAlloc::Function(def_id))
    }

    /// Register a global allocation directly.
    /// Returns the `AllocId` for the allocation.
    pub fn insert_alloc(&self, alloc: GlobalAlloc<'ctx>) -> AllocId {
        self.intern_ctx.alloc_map().insert(alloc)
    }

    /// Get a global allocation by its ID.
    pub fn get_global_alloc(&self, id: AllocId) -> Option<GlobalAlloc<'ctx>> {
        self.intern_ctx.alloc_map().get(id)
    }

    /// Get a global allocation by its ID, panicking if not found.
    pub fn get_global_alloc_unwrap(&self, id: AllocId) -> GlobalAlloc<'ctx> {
        self.intern_ctx.alloc_map().get_unwrap(id)
    }

    /// Iterate over all global allocations.
    pub fn iter_global_allocs(&self) -> Vec<(AllocId, GlobalAlloc<'ctx>)> {
        self.intern_ctx.alloc_map().iter()
    }
}

impl<'ctx> Interner for TirCtx<'ctx> {
    type Ty = TirTy<'ctx>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc::Allocation;

    #[test]
    fn test_allocation_interning_deduplication() {
        let arena = TirArena::default();
        let intern_ctx = InternCtx::new(&arena);

        // Intern the same string twice
        let alloc1 = Allocation::from_c_str("hello");
        let alloc2 = Allocation::from_c_str("hello");

        let interned1 = intern_ctx.intern_allocation(alloc1);
        let interned2 = intern_ctx.intern_allocation(alloc2);

        // They should be equal (deduplicated via Interned)
        assert_eq!(interned1, interned2);
    }

    #[test]
    fn test_different_allocations_not_deduplicated() {
        let arena = TirArena::default();
        let intern_ctx = InternCtx::new(&arena);

        let alloc1 = Allocation::from_c_str("hello");
        let alloc2 = Allocation::from_c_str("world");

        let interned1 = intern_ctx.intern_allocation(alloc1);
        let interned2 = intern_ctx.intern_allocation(alloc2);

        // They should be different (not deduplicated)
        assert_ne!(interned1, interned2);
    }

    #[test]
    fn test_global_alloc_map() {
        use crate::alloc::GlobalAlloc;
        use crate::body::DefId;

        let alloc_map = GlobalAllocMap::new();

        // Insert a function allocation
        let def_id = DefId(42);
        let alloc_id = alloc_map.insert(GlobalAlloc::Function(def_id));

        // Retrieve it
        let retrieved = alloc_map.get_unwrap(alloc_id);
        assert!(matches!(retrieved, GlobalAlloc::Function(id) if id == def_id));
    }
}
