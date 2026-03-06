//! # tidec_builder – Programmatic construction of Tide IR
//!
//! `tidec_builder` provides a high-level, ergonomic API for compiler front-ends
//! that want to target the Tide Intermediate Representation (TIR). Instead of
//! manually assembling [`TirUnit`], [`TirBody`], [`BasicBlockData`] and all of
//! their substructures, users can rely on the builder types exposed here.
//!
//! ## Quick overview
//!
//! | Builder | Purpose |
//! |---|---|
//! | [`BuilderCtx`] | Manages arena and interning, provides ergonomic type creation. |
//! | [`UnitBuilder`] | Construct a [`TirUnit`] (module) with globals and function bodies. |
//! | [`FunctionBuilder`] | Construct a [`TirBody`] (function / closure / coroutine). |
//! | [`BasicBlockBuilder`] | Append statements and set the terminator of a single basic block. |
//!
//! ## Example (pseudo-code)
//!
//! ```rust,ignore
//! use tidec_builder::BuilderCtx;
//!
//! BuilderCtx::with_default(|ctx| {
//!     // Create types without manual interning
//!     let i32_ty = ctx.i32();
//!     let f64_ty = ctx.f64();
//!
//!     // Create a function
//!     let mut func = ctx.function_builder(metadata);
//!     func.declare_ret(i32_ty, false);
//!     let entry = func.create_block();
//!     func.set_terminator(entry, Terminator::Return);
//!
//!     // Create a module
//!     let mut unit = ctx.unit_builder("my_module");
//!     unit.add_body(func.build());
//!     let tir_unit = unit.build();
//! });
//! ```

pub mod basic_block_builder;
pub mod builder_ctx;
pub mod function_builder;
pub mod unit_builder;

pub use basic_block_builder::BasicBlockBuilder;
pub use builder_ctx::BuilderCtx;
pub use function_builder::{BuildError, FunctionBuilder};
pub use unit_builder::UnitBuilder;

// ─── Re-exports from tidec_tir ───────────────────────────────────────────────
//
// Downstream crates should only need to depend on
// `tidec_builder`. We re-export the subset of `tidec_tir` types that are part
// of the builder's public API surface.

/// Re-exported TIR syntax primitives.
pub mod syntax {
    pub use tidec_tir::syntax::{
        BasicBlock, BasicBlockData, BinaryOp, Local, LocalData, Operand, Place, RValue, Statement,
        SwitchTargets, Terminator, UnaryOp, ENTRY_BLOCK, RETURN_LOCAL,
    };
}

/// Re-exported TIR body / module types.
pub mod body {
    pub use tidec_tir::body::{DefId, TirBody, TirBodyMetadata, TirUnit, TirUnitMetadata};
}

/// Re-exported top-level TIR type.
pub use tidec_tir::TirTy;
