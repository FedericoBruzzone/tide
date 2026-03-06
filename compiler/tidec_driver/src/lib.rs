//! # tidec_driver — Driver crate for the Tide compiler.
//!
//! This crate orchestrates the full compilation pipeline from a [`TirUnit`]
//! to emitted output (object file, assembly, LLVM IR, bitcode, or executable).
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────┐
//! │  Caller           │   (nlgc compile pipeline, tidec CLI, tests, …)
//! │                   │
//! │  TirUnit + Config │
//! └────────┬─────────┘
//!          │
//!          ▼
//! ┌──────────────────┐
//! │  tidec_driver     │
//! │                   │
//! │  • creates arena  │
//! │  • builds TirCtx  │
//! │  • dispatches to  │
//! │    backend        │
//! │  • emits output   │
//! └────────┬─────────┘
//!          │
//!          ▼
//! ┌──────────────────┐
//! │ tidec_codegen_*   │   (llvm, cranelift, gcc)
//! └──────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use tidec_driver::{CompileConfig, EmitKind, BackendKind, compile_unit};
//!
//! // Build a TirUnit via tidec_builder (or nlgc_codegen_tide, etc.)
//! let tir_unit = /* ... */;
//!
//! let config = CompileConfig::default();
//! compile_unit(tir_unit, &config);
//! ```
//!
//! For callers that already have a live arena and `TirCtx` (e.g.
//! `nlgc_codegen_tide` which builds the `TirUnit` inside
//! `BuilderCtx::with_default`), use [`compile_unit_with_ctx`] instead to
//! avoid creating a second arena.

mod compile;

pub use compile::{
    compile_unit, compile_unit_to_ir_string, compile_unit_with_ctx, init_tidec_logger,
    CompileConfig, CompileError, CompileOutput,
};

// Re-export key types so callers don't need to depend on tidec_abi / tidec_tir
// directly for common configuration.
pub use tidec_abi::target::BackendKind;
pub use tidec_tir::body::TirUnit;
pub use tidec_tir::ctx::EmitKind;
