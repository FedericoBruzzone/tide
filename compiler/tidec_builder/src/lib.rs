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
//! | [`UnitBuilder`] | Construct a [`TirUnit`] (module) with globals and function bodies. |
//! | [`FunctionBuilder`] | Construct a [`TirBody`] (function / closure / coroutine). |
//! | [`BasicBlockBuilder`] | Append statements and set the terminator of a single basic block. |
//!
//! ## Example (pseudo-code)
//!
//! ```rust,ignore
//! use tidec_builder::{UnitBuilder, FunctionBuilder};
//!
//! let mut unit = UnitBuilder::new("my_module");
//!
//! let mut func = FunctionBuilder::new("main", &tir_ctx);
//! let entry = func.create_block();
//! // ... emit statements and terminators via the function builder ...
//! func.set_terminator(entry, Terminator::Return);
//!
//! unit.add_body(func.build());
//! let tir_unit = unit.build();
//! ```

pub mod basic_block_builder;
pub mod function_builder;
pub mod unit_builder;

pub use basic_block_builder::BasicBlockBuilder;
pub use function_builder::FunctionBuilder;
pub use unit_builder::UnitBuilder;
