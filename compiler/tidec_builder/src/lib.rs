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
