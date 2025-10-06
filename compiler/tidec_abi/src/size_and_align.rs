#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Specifies both the ABI-required and preferred alignment for a type, in bytes.
///
/// Both `abi` and `pref` are powers of two. The ABI alignment (`abi`) is the minimum
/// required alignment for correct program execution, as defined by the platform's ABI.
/// The preferred alignment (`pref`) is a potentially larger value that may yield better
/// performance on some architectures.
///
/// For example, in LLVM, if a preferred alignment is not explicitly set, it defaults to
/// the ABI alignment.
///
/// This type is commonly used during layout computation and codegen to determine
/// how types should be aligned in memory.
pub struct AbiAndPrefAlign {
    /// The alignment required by the ABI for this type.
    pub abi: Align,
    /// The preferred alignment for this type, which may be larger than the ABI alignment.
    pub pref: Align,
}

impl AbiAndPrefAlign {
    /// Creates a new `AbiAndPrefAlign` with the specified ABI and preferred
    /// alignment in bytes.
    pub fn new(abi: u64, pref: u64) -> Self {
        Self {
            abi: Align::from_bytes(abi).unwrap(),
            pref: Align::from_bytes(pref).unwrap(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Size of a type in bytes.
pub struct Size(u64);

impl Size {
    pub const ZERO: Size = Size(0);

    /// Rounds `bits` up to the next-higher byte boundary, if `bits` is
    /// not a multiple of 8.
    pub fn from_bits(bits: impl TryInto<u64>) -> Size {
        let bits = bits.try_into().ok().unwrap();
        // Avoid potential overflow from `bits + 7`.
        Size(bits / 8 + (bits % 8).div_ceil(8))
    }

    #[inline]
    /// Returns the size in bytes.
    pub fn bytes(&self) -> u64 {
        self.0
    }

    #[inline]
    /// Returns the size in bits.
    pub fn bits(&self) -> u64 {
        #[cold]
        // `cold` because this should never happen in practice.
        fn overflow(bytes: u64) -> ! {
            panic!("Size::bits: {bytes} bytes in bits doesn't fit in u64")
        }

        self.bytes()
            .checked_mul(8)
            .unwrap_or_else(|| overflow(self.bytes()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Alignment of a type in bytes (always a power of two).
pub struct Align(u64);

#[derive(Debug)]
pub enum AlignError {
    TooLarge(u64),
    NotPowerOfTwo(u64),
}

impl Align {
    #[inline]
    pub fn from_bits(bits: u64) -> Result<Align, AlignError> {
        Align::from_bytes(Size::from_bits(bits).bytes())
    }

    #[inline]
    /// Creates an `Align` from a byte count.
    pub const fn from_bytes(align: u64) -> Result<Align, AlignError> {
        // To prevent overflow.
        // For example, when `align` is 0, `align.trailing_zeros()` is 64.
        // This means that `1 << tz` results in a panic with "attempt to shift left with overflow"
        // because `1` followed by 64 zeros is too large for a u64.
        if align == 0 {
            return Ok(Align(0));
        }

        #[cold]
        const fn not_power_of_2(align: u64) -> AlignError {
            AlignError::NotPowerOfTwo(align)
        }

        #[cold]
        const fn too_large(align: u64) -> AlignError {
            AlignError::TooLarge(align)
        }

        let tz = align.trailing_zeros();
        if align != (1 << tz) {
            return Err(not_power_of_2(align));
        }

        if align > u64::MAX / 8 {
            return Err(too_large(align));
        }

        Ok(Align(align))
    }

    #[inline]
    pub const fn bytes(&self) -> u64 {
        self.0
    }
}
