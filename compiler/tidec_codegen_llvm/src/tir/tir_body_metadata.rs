use inkwell::{module::Linkage, values::UnnamedAddress, GlobalVisibility};
use tidec_tir::tir;

/// A trait to convert TirLinkage into LLVM Linkage.
///
/// We need to do this due to the orphan rule in Rust. This could cause the
/// stop of the compilation process of an external crate.
pub trait LinkageUtils {
    fn into_linkage(self) -> Linkage;
}

/// A trait to convert TirVisibility into LLVM Visibility (GlobalVisibility).
///
/// We need to do this due to the orphan rule in Rust. This could cause the
/// stop of the compilation process of an external crate.
pub trait VisibilityUtils {
    fn into_visibility(self) -> GlobalVisibility;
}

/// A trait to convert TirCallConv into LLVM CallConv (u32).
///
/// We need to do this due to the orphan rule in Rust. This could cause the
/// stop of the compilation process of an external crate.
pub trait CallConvUtils {
    fn into_call_conv(self) -> u32;
}

/// A trait to convert TirUnnamedAddress into LLVM UnnamedAddress.
///
/// We need to do this due to the orphan rule in Rust. This could cause the
/// stop of the compilation process of an external crate.
pub trait UnnamedAddressUtils {
    fn into_unnamed_address(self) -> UnnamedAddress;
}

impl LinkageUtils for tir::Linkage {
    fn into_linkage(self) -> Linkage {
        match self {
            tir::Linkage::Private => Linkage::LinkerPrivate,
            tir::Linkage::Internal => Linkage::Internal,
            tir::Linkage::AvailableExternally => Linkage::AvailableExternally,
            tir::Linkage::LinkOnce => Linkage::LinkOnceAny,
            tir::Linkage::Weak => Linkage::WeakAny,
            tir::Linkage::Common => Linkage::Common,
            tir::Linkage::Appending => Linkage::Appending,
            tir::Linkage::ExternWeak => Linkage::ExternalWeak,
            tir::Linkage::LinkOnceODR => Linkage::LinkOnceODR,
            tir::Linkage::WeakODR => Linkage::WeakODR,
            tir::Linkage::External => Linkage::External,
        }
    }
}

impl VisibilityUtils for tir::Visibility {
    fn into_visibility(self) -> GlobalVisibility {
        match self {
            tir::Visibility::Default => GlobalVisibility::Default,
            tir::Visibility::Hidden => GlobalVisibility::Hidden,
            tir::Visibility::Protected => GlobalVisibility::Protected,
        }
    }
}

impl CallConvUtils for tir::CallConv {
    fn into_call_conv(self) -> u32 {
        self as u32
    }
}

impl UnnamedAddressUtils for tir::UnnamedAddress {
    fn into_unnamed_address(self) -> UnnamedAddress {
        match self {
            tir::UnnamedAddress::None => UnnamedAddress::None,
            tir::UnnamedAddress::Local => UnnamedAddress::Local,
            tir::UnnamedAddress::Global => UnnamedAddress::Global,
        }
    }
}
