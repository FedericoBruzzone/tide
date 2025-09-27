use inkwell::{module::Linkage, values::UnnamedAddress, GlobalVisibility};
use tidec_lir::lir;

/// A trait to convert LirLinkage into LLVM Linkage.
///
/// We need to do this due to the orphan rule in Rust. This could cause the
/// stop of the compilation process of an external crate.
pub trait LinkageUtils {
    fn into_linkage(self) -> Linkage;
}

/// A trait to convert LirVisibility into LLVM Visibility (GlobalVisibility).
///
/// We need to do this due to the orphan rule in Rust. This could cause the
/// stop of the compilation process of an external crate.
pub trait VisibilityUtils {
    fn into_visibility(self) -> GlobalVisibility;
}

/// A trait to convert LirCallConv into LLVM CallConv (u32).
///
/// We need to do this due to the orphan rule in Rust. This could cause the
/// stop of the compilation process of an external crate.
pub trait CallConvUtils {
    fn into_call_conv(self) -> u32;
}

/// A trait to convert LirUnnamedAddress into LLVM UnnamedAddress.
///
/// We need to do this due to the orphan rule in Rust. This could cause the
/// stop of the compilation process of an external crate.
pub trait UnnamedAddressUtils {
    fn into_unnamed_address(self) -> UnnamedAddress;
}

impl LinkageUtils for lir::Linkage {
    fn into_linkage(self) -> Linkage {
        match self {
            lir::Linkage::Private => Linkage::LinkerPrivate,
            lir::Linkage::Internal => Linkage::Internal,
            lir::Linkage::AvailableExternally => Linkage::AvailableExternally,
            lir::Linkage::LinkOnce => Linkage::LinkOnceAny,
            lir::Linkage::Weak => Linkage::WeakAny,
            lir::Linkage::Common => Linkage::Common,
            lir::Linkage::Appending => Linkage::Appending,
            lir::Linkage::ExternWeak => Linkage::ExternalWeak,
            lir::Linkage::LinkOnceODR => Linkage::LinkOnceODR,
            lir::Linkage::WeakODR => Linkage::WeakODR,
            lir::Linkage::External => Linkage::External,
        }
    }
}

impl VisibilityUtils for lir::Visibility {
    fn into_visibility(self) -> GlobalVisibility {
        match self {
            lir::Visibility::Default => GlobalVisibility::Default,
            lir::Visibility::Hidden => GlobalVisibility::Hidden,
            lir::Visibility::Protected => GlobalVisibility::Protected,
        }
    }
}

impl CallConvUtils for lir::CallConv {
    fn into_call_conv(self) -> u32 {
        self as u32
    }
}

impl UnnamedAddressUtils for lir::UnnamedAddress {
    fn into_unnamed_address(self) -> UnnamedAddress {
        match self {
            lir::UnnamedAddress::None => UnnamedAddress::None,
            lir::UnnamedAddress::Local => UnnamedAddress::Local,
            lir::UnnamedAddress::Global => UnnamedAddress::Global,
        }
    }
}
