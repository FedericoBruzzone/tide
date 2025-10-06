use std::cmp::Ordering;
use std::fmt::{self, Debug};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::ptr;

pub trait Ty<I: Interner<Ty = Self>>: Sized + Clone + Copy + Debug + Eq + PartialEq + Hash {}

pub trait Interner: Sized + Clone + Copy {
    type Ty: Ty<Self>;
}

/// A reference to a value that is interned, and is known to be unique.
///
/// Note that it is possible to have a `T` and a `Interned<T>` that are (or
/// refer to) equal but different values. But if you have two different
/// `Interned<T>`s, they both refer to the same value, at a single location in
/// memory. This means that equality and hashing can be done on the value's
/// address rather than the value's contents, which can improve performance.
pub struct Interned<'a, T>(&'a T);

impl<T> Interned<'_, T> {
    /// Creates a new `Interned` value. 
    /// 
    /// This function is *not* unsafe to call, but the caller must ensure that
    /// the value is unique. That is, there must not be any other `Interned`
    /// values that refer to the same value.
    pub fn new<'a>(value: &'a T) -> Interned<'a, T> {
        Interned(value)
    }
}


impl<'a, T> Clone for Interned<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Copy for Interned<'a, T> {}

impl<'a, T> Deref for Interned<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.0
    }
}

impl<'a, T> PartialEq for Interned<'a, T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // Pointer equality is sufficient, due to the uniqueness constraint.
        ptr::eq(self.0, other.0)
    }
}

impl<'a, T> Eq for Interned<'a, T> {}

impl<'a, T: PartialOrd> PartialOrd for Interned<'a, T> {
    fn partial_cmp(&self, other: &Interned<'a, T>) -> Option<Ordering> {
        // Pointer equality implies equality, due to the uniqueness constraint,
        // but the contents must be compared otherwise.
        if ptr::eq(self.0, other.0) {
            Some(Ordering::Equal)
        } else {
            let res = self.0.partial_cmp(other.0);
            debug_assert_ne!(res, Some(Ordering::Equal));
            res
        }
    }
}

impl<'a, T: Ord> Ord for Interned<'a, T> {
    fn cmp(&self, other: &Interned<'a, T>) -> Ordering {
        // Pointer equality implies equality, due to the uniqueness constraint,
        // but the contents must be compared otherwise.
        if ptr::eq(self.0, other.0) {
            Ordering::Equal
        } else {
            let res = self.0.cmp(other.0);
            debug_assert_ne!(res, Ordering::Equal);
            res
        }
    }
}

impl<'a, T> Hash for Interned<'a, T>
where
    T: Hash,
{
    #[inline]
    fn hash<H: Hasher>(&self, s: &mut H) {
        // Pointer hashing is sufficient, due to the uniqueness constraint.
        ptr::hash(self.0, s)
    }
}


impl<T: Debug> Debug for Interned<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
