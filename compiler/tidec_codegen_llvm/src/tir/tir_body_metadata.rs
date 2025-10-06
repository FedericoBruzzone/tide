use inkwell::{module::Linkage, values::UnnamedAddress, GlobalVisibility};
use tidec_tir::body;

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

impl LinkageUtils for body::Linkage {
    fn into_linkage(self) -> Linkage {
        match self {
            body::Linkage::Private => Linkage::LinkerPrivate,
            body::Linkage::Internal => Linkage::Internal,
            body::Linkage::AvailableExternally => Linkage::AvailableExternally,
            body::Linkage::LinkOnce => Linkage::LinkOnceAny,
            body::Linkage::Weak => Linkage::WeakAny,
            body::Linkage::Common => Linkage::Common,
            body::Linkage::Appending => Linkage::Appending,
            body::Linkage::ExternWeak => Linkage::ExternalWeak,
            body::Linkage::LinkOnceODR => Linkage::LinkOnceODR,
            body::Linkage::WeakODR => Linkage::WeakODR,
            body::Linkage::External => Linkage::External,
        }
    }
}

impl VisibilityUtils for body::Visibility {
    fn into_visibility(self) -> GlobalVisibility {
        match self {
            body::Visibility::Default => GlobalVisibility::Default,
            body::Visibility::Hidden => GlobalVisibility::Hidden,
            body::Visibility::Protected => GlobalVisibility::Protected,
        }
    }
}

impl CallConvUtils for body::CallConv {
    fn into_call_conv(self) -> u32 {
        self as u32
    }
}

impl UnnamedAddressUtils for body::UnnamedAddress {
    fn into_unnamed_address(self) -> UnnamedAddress {
        match self {
            body::UnnamedAddress::None => UnnamedAddress::None,
            body::UnnamedAddress::Local => UnnamedAddress::Local,
            body::UnnamedAddress::Global => UnnamedAddress::Global,
        }
    }
}
