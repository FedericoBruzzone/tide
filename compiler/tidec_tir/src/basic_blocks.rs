use tidec_utils::idx::Idx;

use crate::syntax::{Statement, Terminator};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BasicBlock(usize);
pub const ENTRY_BLOCK: BasicBlock = BasicBlock(0);

#[derive(Debug)]
/// The data of a basic block.
///
/// A basic block is a sequence of statements that ends with a terminator.
/// The terminator is the last statement of the block and transfers control to another block.
pub struct BasicBlockData {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}

////////// Trait implementations  //////////

impl Idx for BasicBlock {
    fn new(idx: usize) -> Self {
        BasicBlock(idx)
    }

    fn idx(&self) -> usize {
        self.0
    }

    fn incr(&mut self) {
        self.0 += 1;
    }

    fn incr_by(&mut self, by: usize) {
        self.0 += by;
    }
}
