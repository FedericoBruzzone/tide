//! Builder for a single basic block.
//!
//! A [`BasicBlockBuilder`] accumulates [`Statement`]s and is finalized by
//! setting a [`Terminator`]. The result is a [`BasicBlockData`] that can be
//! inserted into a function body via the [`FunctionBuilder`](crate::FunctionBuilder).

use tidec_tir::syntax::{
    AggregateKind, BasicBlockData, BinaryOp, CastKind, Operand, Place, RValue, Statement,
    Terminator, UnaryOp,
};
use tidec_tir::ty::Mutability;
use tidec_tir::TirTy;

/// A builder for constructing a single [`BasicBlockData`].
///
/// Statements are appended in order via the various `push_*` helpers, and the
/// block is finalized by calling [`build`](Self::build) with a [`Terminator`].
///
/// # Example
///
/// ```rust,ignore
/// let mut bb = BasicBlockBuilder::new();
/// bb.push_assign(place, rvalue);
/// let data = bb.build(Terminator::Return);
/// ```
pub struct BasicBlockBuilder<'ctx> {
    statements: Vec<Statement<'ctx>>,
}

impl<'ctx> BasicBlockBuilder<'ctx> {
    /// Create a new, empty basic-block builder.
    pub fn new() -> Self {
        Self {
            statements: Vec::new(),
        }
    }

    /// Create a new basic-block builder with pre-allocated capacity for
    /// `capacity` statements.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            statements: Vec::with_capacity(capacity),
        }
    }

    // ───────────────────────── Raw push ──────────────────────────

    /// Push an arbitrary [`Statement`] to the block.
    pub fn push_statement(&mut self, stmt: Statement<'ctx>) -> &mut Self {
        self.statements.push(stmt);
        self
    }

    // ───────────────────────── Assign helpers ────────────────────

    /// Append an `Assign(place, rvalue)` statement.
    pub fn push_assign(&mut self, place: Place<'ctx>, rvalue: RValue<'ctx>) -> &mut Self {
        self.statements
            .push(Statement::Assign(Box::new((place, rvalue))));
        self
    }

    /// Append an assignment of an [`Operand`] to a [`Place`].
    ///
    /// This is a shorthand for `push_assign(place, RValue::Operand(operand))`.
    pub fn push_assign_operand(&mut self, place: Place<'ctx>, operand: Operand<'ctx>) -> &mut Self {
        self.push_assign(place, RValue::Operand(operand))
    }

    /// Append a unary-operation assignment: `place = unary_op(operand)`.
    pub fn push_assign_unary_op(
        &mut self,
        place: Place<'ctx>,
        op: UnaryOp,
        operand: Operand<'ctx>,
    ) -> &mut Self {
        self.push_assign(place, RValue::UnaryOp(op, operand))
    }

    /// Append a binary-operation assignment: `place = lhs binop rhs`.
    pub fn push_assign_binary_op(
        &mut self,
        place: Place<'ctx>,
        op: BinaryOp,
        lhs: Operand<'ctx>,
        rhs: Operand<'ctx>,
    ) -> &mut Self {
        self.push_assign(place, RValue::BinaryOp(op, lhs, rhs))
    }

    /// Append a cast assignment: `place = cast(operand) as ty`.
    pub fn push_assign_cast(
        &mut self,
        place: Place<'ctx>,
        kind: CastKind,
        operand: Operand<'ctx>,
        ty: TirTy<'ctx>,
    ) -> &mut Self {
        self.push_assign(place, RValue::Cast(kind, operand, ty))
    }

    /// Append an aggregate construction assignment:
    /// `place = AggregateKind(operands…)`.
    pub fn push_assign_aggregate(
        &mut self,
        place: Place<'ctx>,
        kind: AggregateKind<'ctx>,
        operands: Vec<Operand<'ctx>>,
    ) -> &mut Self {
        self.push_assign(place, RValue::Aggregate(kind, operands))
    }

    /// Append an address-of assignment: `place = &[mut] source`.
    pub fn push_assign_address_of(
        &mut self,
        place: Place<'ctx>,
        mutability: Mutability,
        source: Place<'ctx>,
    ) -> &mut Self {
        self.push_assign(place, RValue::AddressOf(mutability, source))
    }

    // ───────────────────────── Introspection ─────────────────────

    /// Returns the number of statements already pushed.
    pub fn len(&self) -> usize {
        self.statements.len()
    }

    /// Returns `true` if no statements have been pushed yet.
    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }

    // ───────────────────────── Finalization ──────────────────────

    /// Finalize the block with the given terminator and return the completed
    /// [`BasicBlockData`].
    ///
    /// The builder is consumed by this call.
    pub fn build(self, terminator: Terminator<'ctx>) -> BasicBlockData<'ctx> {
        BasicBlockData {
            statements: self.statements,
            terminator,
        }
    }
}

impl<'ctx> Default for BasicBlockBuilder<'ctx> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tidec_tir::syntax::{BasicBlock, Local, SwitchTargets, RETURN_LOCAL};
    use tidec_utils::idx::Idx;

    #[test]
    fn empty_block_with_return() {
        let bb = BasicBlockBuilder::new();
        let data = bb.build(Terminator::Return);
        assert!(data.statements.is_empty());
        assert!(matches!(data.terminator, Terminator::Return));
    }

    #[test]
    fn default_is_empty() {
        let bb = BasicBlockBuilder::<'_>::default();
        assert!(bb.is_empty());
        assert_eq!(bb.len(), 0);
    }

    #[test]
    fn push_assign_increases_length() {
        let mut bb = BasicBlockBuilder::new();
        assert_eq!(bb.len(), 0);

        let place: Place<'_> = Place::from(Local::new(1));
        let operand = Operand::Use(Place::from(Local::new(2)));
        bb.push_assign_operand(place, operand);
        assert_eq!(bb.len(), 1);
    }

    #[test]
    fn build_with_goto_terminator() {
        let mut bb = BasicBlockBuilder::new();
        let place: Place<'_> = Place::from(RETURN_LOCAL);
        let operand = Operand::Use(Place::from(Local::new(1)));
        bb.push_assign_operand(place, operand);

        let target = BasicBlock::new(1);
        let data = bb.build(Terminator::Goto { target });

        assert_eq!(data.statements.len(), 1);
        assert!(matches!(
            data.terminator,
            Terminator::Goto { target: t } if t == BasicBlock::new(1)
        ));
    }

    #[test]
    fn with_capacity_starts_empty() {
        let bb = BasicBlockBuilder::<'_>::with_capacity(16);
        assert!(bb.is_empty());
    }

    #[test]
    fn push_statement_raw() {
        let mut bb = BasicBlockBuilder::new();
        let place: Place<'_> = Place::from(Local::new(0));
        let rvalue = RValue::Operand(Operand::Use(Place::from(Local::new(1))));
        let stmt = Statement::Assign(Box::new((place, rvalue)));
        bb.push_statement(stmt);
        assert_eq!(bb.len(), 1);
    }

    #[test]
    fn multiple_statements_preserve_order() {
        let mut bb = BasicBlockBuilder::new();

        // Push three assigns targeting locals 0, 1, 2.
        for i in 0..3 {
            let place: Place<'_> = Place::from(Local::new(i));
            let rvalue = RValue::Operand(Operand::Use(Place::from(Local::new(i + 10))));
            bb.push_assign(place, rvalue);
        }

        let data = bb.build(Terminator::Unreachable);
        assert_eq!(data.statements.len(), 3);
        assert!(matches!(data.terminator, Terminator::Unreachable));
    }

    #[test]
    fn build_with_switch_int_terminator() {
        let bb = BasicBlockBuilder::new();
        let discr = Operand::Use(Place::from(Local::new(5)));
        let targets = SwitchTargets::if_then(BasicBlock::new(1), BasicBlock::new(2));
        let data = bb.build(Terminator::SwitchInt { discr, targets });

        assert!(data.statements.is_empty());
        assert!(matches!(data.terminator, Terminator::SwitchInt { .. }));
    }

    #[test]
    fn chaining_api() {
        let data = {
            let mut bb = BasicBlockBuilder::new();
            let p0: Place<'_> = Place::from(Local::new(0));
            let p1: Place<'_> = Place::from(Local::new(1));
            let op = Operand::Use(Place::from(Local::new(2)));
            bb.push_assign_operand(p0, op.clone())
                .push_assign_unary_op(p1, UnaryOp::Neg, op);
            bb.build(Terminator::Return)
        };
        assert_eq!(data.statements.len(), 2);
    }
}
