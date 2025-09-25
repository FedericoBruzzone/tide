use tracing::{info, instrument};

use crate::size_and_align::{AbiAndPrefAlign, Size};

#[derive(Debug)]
/// Describes the target configuration used during code generation.
///
/// This struct encapsulates information about the backend, data layout,
/// and optional target triple. It is used to drive architecture- and
/// platform-specific decisions throughout the compiler.
pub struct TirTarget {
    /// The codegen backend to use.
    pub codegen_backend: BackendKind,
    /// The data layout configuration for the target, including type alignments,
    /// pointer size, and other ABI-relevant properties.
    pub data_layout: TargetDataLayout,
    /// The target triple string identifying the target architecture, vendor,
    /// operating system, and environment.
    ///
    /// If this is `None`, the target triple will not be set in the LLVM module,
    /// which may affect platform-specific codegen behavior or defaults.
    pub target_triple: Option<TargetTriple>,
}

impl TirTarget {
    pub fn new(codegen_backend: BackendKind) -> Self {
        TirTarget {
            data_layout: TargetDataLayout::new(),
            codegen_backend,
            target_triple: None,
        }
    }

    // TODO: make it better. Perhaps by using a specific TargetDataLayout for each
    // compiler backend.
    pub fn data_layout_string(&self) -> String {
        match self.codegen_backend {
            BackendKind::Llvm => self.data_layout.as_llvm_datalayout_string(),
            BackendKind::Cranelift => self.data_layout.as_cranelift_datalayout_string(),
            BackendKind::Gcc => self.data_layout.as_gcc_datalayout_string(),
        }
    }

    // TODO: make it better. Perhaps by using a specific TargetDataLayout for each
    // compiler backend.
    pub fn target_triple_string(&self) -> Option<String> {
        self.target_triple.as_ref()?;

        match self.codegen_backend {
            BackendKind::Llvm => Some(
                self.target_triple
                    .as_ref()
                    .unwrap()
                    .into_llvm_triple_string(),
            ),
            BackendKind::Cranelift => Some(
                self.target_triple
                    .as_ref()
                    .unwrap()
                    .into_cranelift_triple_string(),
            ),
            BackendKind::Gcc => Some(
                self.target_triple
                    .as_ref()
                    .unwrap()
                    .into_gcc_triple_string(),
            ),
        }
    }
}

#[derive(Debug)]
/// The backend kind for code generation.
///
/// This enum represents the different backends that can be used for code generation.
pub enum BackendKind {
    /// The LLVM backend.
    Llvm,

    /// The Cranelift backend.
    Cranelift,

    /// The GCC (GNU Compiler Collection) backend.
    Gcc,
}

#[derive(Debug)]
/// Describes the target platform's data layout, including type alignments, pointer size,
/// and other ABI-related information used during code generation.
///
/// It includes alignment requirements for integer, float, and vector types, as well as
/// general properties such as pointer size and aggregate alignment.
pub struct TargetDataLayout {
    /// The endianness of the target architecture.
    pub endianess: Endianess,

    // Integer type alignments, for both signed and unsigned integers.
    pub int1_align: AbiAndPrefAlign,
    pub int8_align: AbiAndPrefAlign,
    pub int16_align: AbiAndPrefAlign,
    pub int32_align: AbiAndPrefAlign,
    pub int64_align: AbiAndPrefAlign,
    pub int128_align: AbiAndPrefAlign,

    // Floating point type alignments
    pub float16_align: AbiAndPrefAlign,
    pub float32_align: AbiAndPrefAlign,
    pub float64_align: AbiAndPrefAlign,
    pub float128_align: AbiAndPrefAlign,

    /// The size of pointers in bytes.
    pub pointer_size: Size,
    /// The ABI and preferred alignment for pointers.
    pub pointer_align: AbiAndPrefAlign,

    /// The minimum and preferred alignment for aggregate types (e.g., structs, arrays).
    pub aggregate_align: AbiAndPrefAlign,

    /// Alignments for vector types.
    pub vector_align: Vec<(Size, AbiAndPrefAlign)>,

    /// An identifier that specifies the address space that some operation
    /// should operate on. Special address spaces have an effect on code generation,
    /// depending on the target and the address spaces it implements.
    pub instruction_address_space: AddressSpace,
}

impl Default for TargetDataLayout {
    fn default() -> Self {
        TargetDataLayout {
            endianess: Endianess::Big,
            int1_align: AbiAndPrefAlign::new(8, 8),
            int8_align: AbiAndPrefAlign::new(8, 8),
            int16_align: AbiAndPrefAlign::new(16, 16),
            int32_align: AbiAndPrefAlign::new(32, 32),
            int64_align: AbiAndPrefAlign::new(32, 64),
            int128_align: AbiAndPrefAlign::new(32, 64),
            float16_align: AbiAndPrefAlign::new(16, 16),
            float32_align: AbiAndPrefAlign::new(32, 32),
            float64_align: AbiAndPrefAlign::new(64, 64),
            float128_align: AbiAndPrefAlign::new(128, 128),
            pointer_size: Size::from_bits(64),
            pointer_align: AbiAndPrefAlign::new(64, 64),
            aggregate_align: AbiAndPrefAlign::new(0, 64),
            vector_align: vec![
                (Size::from_bits(64), AbiAndPrefAlign::new(64, 64)),
                (Size::from_bits(128), AbiAndPrefAlign::new(128, 128)),
            ],
            instruction_address_space: AddressSpace::DATA,
        }
    }
}

impl TargetDataLayout {
    #[instrument]
    pub fn new() -> Self {
        let target_data_layout = TargetDataLayout::default();
        info!("TargetDataLayout created: {:?}", target_data_layout);
        target_data_layout
    }

    pub fn pointer_size(&self) -> Size {
        self.pointer_size
    }

    pub fn pointer_align(&self, address_space: AddressSpace) -> AbiAndPrefAlign {
        match address_space {
            AddressSpace::DATA => self.pointer_align,
        }
    }

    /// For example, for x86_64-unknown-linux-gnu, the data layout string could be:
    /// `e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128`
    pub fn as_llvm_datalayout_string(&self) -> String {
        let format_align = |name: &str, align: &AbiAndPrefAlign| {
            format!("-{}:{}:{}", name, align.abi.bytes(), align.pref.bytes())
        };

        let mut s = String::new();

        // Add endianess
        s.push(if self.endianess == Endianess::Little {
            'e'
        } else {
            'E'
        });

        // Add pointer and integer alignments
        s.push_str(&format!(
            "-p:{}:{}:{}",
            self.pointer_size.bytes(),
            self.pointer_align.abi.bytes(),
            self.pointer_align.pref.bytes()
        ));

        // Format for integer types
        s.push_str(&format_align("i1", &self.int1_align));
        s.push_str(&format_align("i8", &self.int8_align));
        s.push_str(&format_align("i16", &self.int16_align));
        s.push_str(&format_align("i32", &self.int32_align));
        s.push_str(&format_align("i64", &self.int64_align));
        s.push_str(&format_align("i128", &self.int128_align));

        // Format for floating point types
        s.push_str(&format_align("f16", &self.float16_align));
        s.push_str(&format_align("f32", &self.float32_align));
        s.push_str(&format_align("f64", &self.float64_align));
        s.push_str(&format_align("f128", &self.float128_align));

        // Aggregate alignment
        s.push_str(&format_align("a", &self.aggregate_align));

        // Vector alignments
        for (size, align) in &self.vector_align {
            s.push_str(&format!(
                "-v{}:{}:{}",
                size.bytes(),
                align.abi.bytes(),
                align.pref.bytes()
            ));
        }

        // Instruction address space
        s.push_str(&format!("-P{}", u32::from(&self.instruction_address_space)));

        s
    }

    fn as_cranelift_datalayout_string(&self) -> String {
        unimplemented!()
    }

    fn as_gcc_datalayout_string(&self) -> String {
        unimplemented!()
    }

    // /// Parse data layout from an [llvm data layout string](https://llvm.org/docs/LangRef.html#data-layout)
    // /// For example, for x86_64-unknown-linux-gnu, the data layout string is:
    // /// `e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128`
    // pub fn parse_from_llvm_datalayout_string<'a>(
    //     input: &'a str,
    // ) -> Result<TargetDataLayout, TargetDataLayoutErrors<'a>> {
    //     // Parse an address space index from a string.
    //     let parse_address_space = |s: &'a str, cause: &'a str| {
    //         s.parse::<u32>().map(AddressSpace).map_err(|err| {
    //             TargetDataLayoutErrors::InvalidAddressSpace { addr_space: s, cause, err }
    //         })
    //     };
    //
    //     // Parse a bit count from a string.
    //     let parse_bits = |s: &'a str, kind: &'a str, cause: &'a str| {
    //         s.parse::<u64>().map_err(|err| TargetDataLayoutErrors::InvalidBits {
    //             kind,
    //             bit: s,
    //             cause,
    //             err,
    //         })
    //     };
    //
    //     // Parse a size string.
    //     let parse_size =
    //         |s: &'a str, cause: &'a str| parse_bits(s, "size", cause).map(Size::from_bits);
    //
    //     // Parse an alignment string.
    //     let parse_align = |s: &[&'a str], cause: &'a str| {
    //         if s.is_empty() {
    //             return Err(TargetDataLayoutErrors::MissingAlignment { cause });
    //         }
    //         let align_from_bits = |bits| {
    //             Align::from_bits(bits)
    //                 .map_err(|err| TargetDataLayoutErrors::InvalidAlignment { cause, err })
    //         };
    //         let abi = parse_bits(s[0], "alignment", cause)?;
    //         let pref = s.get(1).map_or(Ok(abi), |pref| parse_bits(pref, "alignment", cause))?;
    //         Ok(AbiAndPrefAlign { abi: align_from_bits(abi)?, pref: align_from_bits(pref)? })
    //     };
    //
    //     let mut dl = TargetDataLayout::default();
    //     let mut i128_align_src = 64;
    //     for spec in input.split('-') {
    //         let spec_parts = spec.split(':').collect::<Vec<_>>();
    //
    //         match &*spec_parts {
    //             ["e"] => dl.endian = Endian::Little,
    //             ["E"] => dl.endian = Endian::Big,
    //             [p] if p.starts_with('P') => {
    //                 dl.instruction_address_space = parse_address_space(&p[1..], "P")?
    //             }
    //             ["a", a @ ..] => dl.aggregate_align = parse_align(a, "a")?,
    //             ["f16", a @ ..] => dl.f16_align = parse_align(a, "f16")?,
    //             ["f32", a @ ..] => dl.f32_align = parse_align(a, "f32")?,
    //             ["f64", a @ ..] => dl.f64_align = parse_align(a, "f64")?,
    //             ["f128", a @ ..] => dl.f128_align = parse_align(a, "f128")?,
    //             // FIXME(erikdesjardins): we should be parsing nonzero address spaces
    //             // this will require replacing TargetDataLayout::{pointer_size,pointer_align}
    //             // with e.g. `fn pointer_size_in(AddressSpace)`
    //             [p @ "p", s, a @ ..] | [p @ "p0", s, a @ ..] => {
    //                 dl.pointer_size = parse_size(s, p)?;
    //                 dl.pointer_align = parse_align(a, p)?;
    //             }
    //             [s, a @ ..] if s.starts_with('i') => {
    //                 let Ok(bits) = s[1..].parse::<u64>() else {
    //                     parse_size(&s[1..], "i")?; // For the user error.
    //                     continue;
    //                 };
    //                 let a = parse_align(a, s)?;
    //                 match bits {
    //                     1 => dl.i1_align = a,
    //                     8 => dl.i8_align = a,
    //                     16 => dl.i16_align = a,
    //                     32 => dl.i32_align = a,
    //                     64 => dl.i64_align = a,
    //                     _ => {}
    //                 }
    //                 if bits >= i128_align_src && bits <= 128 {
    //                     // Default alignment for i128 is decided by taking the alignment of
    //                     // largest-sized i{64..=128}.
    //                     i128_align_src = bits;
    //                     dl.i128_align = a;
    //                 }
    //             }
    //             [s, a @ ..] if s.starts_with('v') => {
    //                 let v_size = parse_size(&s[1..], "v")?;
    //                 let a = parse_align(a, s)?;
    //                 if let Some(v) = dl.vector_align.iter_mut().find(|v| v.0 == v_size) {
    //                     v.1 = a;
    //                     continue;
    //                 }
    //                 // No existing entry, add a new one.
    //                 dl.vector_align.push((v_size, a));
    //             }
    //             _ => {} // Ignore everything else.
    //         }
    //     }
    //     Ok(dl)
    // }
}

#[derive(Debug, PartialEq, Eq)]
/// The endianness of the target architecture.
pub enum Endianess {
    /// Little-endian.
    Little,

    /// Big-endian.
    Big,
}

#[derive(Debug)]
/// Represents a target triple, which uniquely identifies a compilation target.
///
/// A target triple is a string that encodes information about the target architecture,
/// vendor, operating system, environment, and ABI. This is commonly used to select
/// appropriate code generation strategies, linkers, standard libraries, and target-specific
/// configurations.
///
/// Example: `"x86_64-unknown-linux-gnu"`
///
/// Each component of the triple is stored separately for easier access and manipulation.
pub struct TargetTriple {
    /// The target architecture (e.g., "x86_64", "aarch64").
    pub arch: String,
    /// The target vendor (e.g., "unknown", "apple").
    pub vendor: String,
    /// The target operating system (e.g., "linux", "windows").
    pub os: String,
    /// The target environment or runtime (e.g., "gnu", "msvc", "musl").
    pub env: String,
    /// The ABI used on the target (e.g., "eabi", "gnu").
    pub abi: String,
}

impl TargetTriple {
    #[tracing::instrument]
    pub fn new(arch: &str, vendor: &str, os: &str, env: &str, abi: &str) -> Self {
        TargetTriple {
            arch: arch.to_string(),
            vendor: vendor.to_string(),
            os: os.to_string(),
            env: env.to_string(),
            abi: abi.to_string(),
        }
    }

    // ARCHITECTURE-VENDOR-OPERATING_SYSTEM-ENVIRONMENT
    pub fn into_llvm_triple_string(&self) -> String {
        format!(
            "{}-{}-{}-{}-{}",
            self.arch, self.vendor, self.os, self.env, self.abi
        )
    }

    pub fn into_cranelift_triple_string(&self) -> String {
        unimplemented!()
    }

    pub fn into_gcc_triple_string(&self) -> String {
        unimplemented!()
    }
}

// TODO: Other address spaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressSpace {
    /// The default address space.
    DATA = 0,
}

impl From<&AddressSpace> for u32 {
    fn from(addr_space: &AddressSpace) -> Self {
        match *addr_space {
            AddressSpace::DATA => 0,
        }
    }
}
