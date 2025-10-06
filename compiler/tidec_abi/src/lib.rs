pub mod calling_convention;
pub mod layout;
pub mod size_and_align;
pub mod target;

use std::ops::Deref;

use tidec_utils::interner::Interned;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Layout<'ctx>(Interned<'ctx, layout::Layout>);

impl<'ctx> Deref for Layout<'ctx> {
    type Target = Interned<'ctx, layout::Layout>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
