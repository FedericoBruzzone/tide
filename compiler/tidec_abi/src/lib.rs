pub mod calling_convention;
pub mod layout;
pub mod size_and_align;
pub mod target;

use tidec_utils::interner::Interned;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Layout<'ctx>(Interned<'ctx, layout::Layout>);
