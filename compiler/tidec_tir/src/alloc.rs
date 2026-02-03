//! Allocation types for constant values in TIR.
//!
//! This module provides the infrastructure for representing constant values
//! that need to be stored in memory, following the design of Rust's MIR.
//!
//! The key types are:
//! - [`AllocId`]: An abstract identifier for an allocation
//! - [`Allocation`]: The actual bytes and metadata of an allocation
//! - [`GlobalAlloc`]: What an allocation can represent (memory, function, etc.)

use crate::body::DefId;
use crate::TirAllocation;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use tidec_abi::size_and_align::{Align, Size};

/// A unique identifier for a constant allocation.
///
/// `AllocId` is an abstract identifier that allows the compiler to distinguish
/// between different memory blocks. It does not represent a real machine address,
/// but rather serves as a key to look up the actual allocation data.
///
/// This indirection ensures that when a "raw constant" (which is basically just
/// an `AllocId`) is turned into a `ConstValue` and later converted back, the
/// identity of the original allocation is preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocId(usize);

impl AllocId {
    /// Create a new unique allocation ID.
    pub fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        AllocId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

impl Default for AllocId {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents the content of a constant allocation.
///
/// An `Allocation` contains the raw bytes of a constant value, along with
/// metadata about its alignment and any relocations (pointers to other allocations).
///
/// This type implements `Eq` and `Hash` to support interning via `InternedSet`.
#[derive(Debug, Clone)]
pub struct Allocation {
    /// The raw bytes of the allocation.
    bytes: Vec<u8>,
    /// The alignment of this allocation.
    align: Align,
    /// Relocations: maps byte offsets to the AllocId they point to.
    /// This is used for pointers within the allocation.
    /// Uses BTreeMap for deterministic ordering (required for Hash).
    relocations: BTreeMap<Size, AllocId>,
    /// Whether this allocation is mutable.
    mutability: Mutability,
}

impl PartialEq for Allocation {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
            && self.align == other.align
            && self.relocations == other.relocations
            && self.mutability == other.mutability
    }
}

impl Eq for Allocation {}

impl Hash for Allocation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bytes.hash(state);
        self.align.hash(state);
        // Hash relocations in a deterministic order (BTreeMap is sorted)
        for (offset, alloc_id) in &self.relocations {
            offset.hash(state);
            alloc_id.hash(state);
        }
        self.mutability.hash(state);
    }
}

/// Mutability of an allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mutability {
    /// The allocation is immutable.
    Immutable,
    /// The allocation is mutable.
    Mutable,
}

impl Allocation {
    /// Create a new allocation with the given bytes and alignment.
    pub fn new(bytes: Vec<u8>, align: Align) -> Self {
        Self {
            bytes,
            align,
            relocations: BTreeMap::new(),
            mutability: Mutability::Immutable,
        }
    }

    /// Create a new allocation from a string.
    /// The string is null-terminated for C compatibility.
    pub fn from_c_str(s: &str) -> Self {
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0); // Null terminator for C strings
                       // Use 1-byte alignment for strings
        Self::new(bytes, Align::from_bytes(1).unwrap())
    }

    /// Create a new allocation from bytes without null termination.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::new(bytes.to_vec(), Align::from_bytes(1).unwrap())
    }

    /// Get the bytes of this allocation.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Get the size of this allocation in bytes.
    pub fn size(&self) -> Size {
        Size::from_bits(self.bytes.len() as u64 * 8)
    }

    /// Get the alignment of this allocation.
    pub fn align(&self) -> Align {
        self.align
    }

    /// Check if this allocation is mutable.
    pub fn is_mutable(&self) -> bool {
        self.mutability == Mutability::Mutable
    }

    /// Add a relocation at the given offset.
    pub fn add_relocation(&mut self, offset: Size, alloc_id: AllocId) {
        self.relocations.insert(offset, alloc_id);
    }

    /// Get the relocations in this allocation.
    pub fn relocations(&self) -> &BTreeMap<Size, AllocId> {
        &self.relocations
    }
}

/// What a global allocation can represent.
///
/// This enum describes the different kinds of things that can be stored
/// in the global allocation map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlobalAlloc<'ctx> {
    /// A constant memory allocation (e.g., a string literal, array, or struct).
    /// The allocation is interned and wrapped in `TirAllocation`.
    Memory(TirAllocation<'ctx>),
    /// A reference to a function.
    Function(DefId),
    /// A static variable.
    Static(DefId),
}

impl<'ctx> GlobalAlloc<'ctx> {
    /// Returns the `TirAllocation` if this is a memory allocation.
    pub fn unwrap_memory(&self) -> TirAllocation<'ctx> {
        match self {
            GlobalAlloc::Memory(alloc) => *alloc,
            _ => panic!("expected Memory, got {:?}", self),
        }
    }

    /// Returns `Some(DefId)` if this is a function reference.
    pub fn unwrap_function(&self) -> DefId {
        match self {
            GlobalAlloc::Function(def_id) => *def_id,
            _ => panic!("expected Function, got {:?}", self),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alloc_id_unique() {
        let id1 = AllocId::new();
        let id2 = AllocId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_allocation_from_c_str() {
        let alloc = Allocation::from_c_str("hello");
        assert_eq!(alloc.bytes(), b"hello\0");
        assert_eq!(alloc.size(), Size::from_bytes(6));
    }
}
