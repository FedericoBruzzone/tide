//! Core compilation driver.
//!
//! This module provides the main entry points for compiling a [`TirUnit`]
//! through a configurable backend to produce output artifacts (object files,
//! assembly, LLVM IR, bitcode, or executables).
//!
//! There are two entry points:
//!
//! - [`compile_unit`]: Creates an arena + `TirCtx` internally, then compiles.
//!   Use this when the caller hands over an already-built `TirUnit` and does
//!   not need to keep the arena alive afterwards.
//!
//! - [`compile_unit_with_ctx`]: Takes an existing `TirCtx` (the caller owns
//!   the arena). Use this when the `TirUnit` was built inside a
//!   `BuilderCtx::with_default` closure and the arena is still live.

use std::fmt;

use tidec_abi::target::{BackendKind, TirTarget};
use tidec_codegen_llvm::entry::{llvm_codegen_lir_unit, llvm_codegen_to_ir_string};
use tidec_tir::body::TirUnit;
use tidec_tir::ctx::{EmitKind, InternCtx, TirArena, TirArgs, TirCtx};
use tracing::{debug, info, instrument};

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for a single compilation run.
#[derive(Debug, Clone, Copy)]
pub struct CompileConfig {
    /// Which codegen backend to use.
    pub backend: BackendKind,

    /// What kind of output to emit.
    pub emit: EmitKind,
}

impl Default for CompileConfig {
    /// Defaults: LLVM backend, object file output.
    fn default() -> Self {
        Self {
            backend: BackendKind::Llvm,
            emit: EmitKind::Object,
        }
    }
}

impl CompileConfig {
    /// Create a new configuration with the given backend and emit kind.
    pub fn new(backend: BackendKind, emit: EmitKind) -> Self {
        Self { backend, emit }
    }

    /// Shorthand: LLVM backend emitting an object file.
    pub fn llvm_object() -> Self {
        Self::new(BackendKind::Llvm, EmitKind::Object)
    }

    /// Shorthand: LLVM backend emitting assembly.
    pub fn llvm_assembly() -> Self {
        Self::new(BackendKind::Llvm, EmitKind::Assembly)
    }

    /// Shorthand: LLVM backend emitting LLVM IR.
    pub fn llvm_ir() -> Self {
        Self::new(BackendKind::Llvm, EmitKind::LlvmIr)
    }

    /// Shorthand: LLVM backend emitting LLVM bitcode.
    pub fn llvm_bitcode() -> Self {
        Self::new(BackendKind::Llvm, EmitKind::LlvmBitcode)
    }

    /// Shorthand: LLVM backend emitting an executable.
    pub fn llvm_executable() -> Self {
        Self::new(BackendKind::Llvm, EmitKind::Executable)
    }
}

// =============================================================================
// Output
// =============================================================================

/// The result of a successful compilation.
///
/// For file-based outputs (object, assembly, executable, bitcode) the output
/// is written to disk and this struct records metadata about what was produced.
/// For in-memory outputs (e.g. `EmitKind::LlvmIr` when requested via
/// [`compile_unit_to_ir_string`]) the IR string is available in
/// [`CompileOutput::ir_string`].
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// The emit kind that was actually used.
    pub emit_kind: EmitKind,

    /// For `EmitKind::LlvmIr` when using [`compile_unit_to_ir_string`], this
    /// contains the textual LLVM IR. `None` for file-based outputs.
    pub ir_string: Option<String>,
}

// =============================================================================
// Errors
// =============================================================================

/// Errors that can occur during compilation.
#[derive(Debug)]
pub enum CompileError {
    /// The requested backend is not yet implemented.
    UnsupportedBackend(String),

    /// A codegen-internal error.
    CodegenError(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::UnsupportedBackend(backend) => {
                write!(f, "unsupported backend: {backend}")
            }
            CompileError::CodegenError(msg) => {
                write!(f, "codegen error: {msg}")
            }
        }
    }
}

impl std::error::Error for CompileError {}

// =============================================================================
// Entry points
// =============================================================================

/// Compile a [`TirUnit`] using a freshly created arena and context.
///
/// This is the simplest entry point. It:
/// 1. Creates a [`TirArena`], [`InternCtx`], and [`TirCtx`].
/// 2. Dispatches to the appropriate codegen backend.
/// 3. Emits the output (file on disk).
///
/// Because the arena is created inside this function and must outlive the
/// `TirUnit`, we accept a **callback** that receives the live `TirCtx`
/// and must produce the `TirUnit` within it.
///
/// # Example
///
/// ```rust,ignore
/// use tidec_driver::{compile_unit, CompileConfig};
///
/// let config = CompileConfig::llvm_object();
/// compile_unit(&config, |tir_ctx| {
///     // Build the TirUnit using tir_ctx / BuilderCtx ...
///     build_tir_unit(tir_ctx)
/// });
/// ```
#[instrument(level = "info", skip(config, build_unit), fields(backend = ?config.backend, emit = ?config.emit))]
pub fn compile_unit<F>(config: &CompileConfig, build_unit: F) -> Result<CompileOutput, CompileError>
where
    F: for<'ctx> FnOnce(&TirCtx<'ctx>) -> TirUnit<'ctx>,
{
    info!("compile_unit: creating arena and context");

    let target = TirTarget::new(config.backend);
    let arguments = TirArgs {
        emit_kind: config.emit,
    };
    let tir_arena = TirArena::default();
    let intern_ctx = InternCtx::new(&tir_arena);
    let tir_ctx = TirCtx::new(&target, &arguments, &intern_ctx);

    let tir_unit = build_unit(&tir_ctx);

    compile_unit_with_ctx(tir_ctx, tir_unit, config)
}

/// Compile a [`TirUnit`] using an already-existing [`TirCtx`].
///
/// Use this when the caller already has a live arena (e.g. inside a
/// `BuilderCtx::with_default` closure) and has built the `TirUnit` using
/// that arena.
///
/// The `config` parameter controls what output is produced; the backend
/// and emit kind in `config` must be consistent with the `TirCtx` (in
/// particular, `tir_ctx.backend_kind()` should match `config.backend`).
/// If they differ, the `TirCtx`'s backend is authoritative for the actual
/// codegen dispatch, but `config.emit` is respected by the backend's
/// `emit_output` implementation (via `TirArgs`).
#[instrument(level = "info", skip(tir_ctx, tir_unit), fields(unit = %tir_unit.metadata.unit_name))]
pub fn compile_unit_with_ctx<'ctx>(
    tir_ctx: TirCtx<'ctx>,
    tir_unit: TirUnit<'ctx>,
    config: &CompileConfig,
) -> Result<CompileOutput, CompileError> {
    info!(
        "compile_unit_with_ctx: dispatching to backend {:?}, emit {:?}",
        config.backend, config.emit
    );

    match tir_ctx.backend_kind() {
        BackendKind::Llvm => {
            debug!("Using LLVM backend");
            llvm_codegen_lir_unit(tir_ctx, tir_unit);
            Ok(CompileOutput {
                emit_kind: config.emit,
                ir_string: None,
            })
        }
        BackendKind::Cranelift => Err(CompileError::UnsupportedBackend("cranelift".to_string())),
        BackendKind::Gcc => Err(CompileError::UnsupportedBackend("gcc".to_string())),
    }
}

/// Compile a [`TirUnit`] to an LLVM IR string (in-memory, no file output).
///
/// This is useful for testing and for pipelines that need to inspect the
/// generated IR without writing to disk.
#[instrument(level = "info", skip(tir_ctx, tir_unit), fields(unit = %tir_unit.metadata.unit_name))]
pub fn compile_unit_to_ir_string<'ctx>(
    tir_ctx: TirCtx<'ctx>,
    tir_unit: TirUnit<'ctx>,
) -> Result<CompileOutput, CompileError> {
    info!("compile_unit_to_ir_string: generating LLVM IR string");

    match tir_ctx.backend_kind() {
        BackendKind::Llvm => {
            let ir = llvm_codegen_to_ir_string(tir_ctx, tir_unit);
            Ok(CompileOutput {
                emit_kind: EmitKind::LlvmIr,
                ir_string: Some(ir),
            })
        }
        BackendKind::Cranelift => Err(CompileError::UnsupportedBackend("cranelift".to_string())),
        BackendKind::Gcc => Err(CompileError::UnsupportedBackend("gcc".to_string())),
    }
}

// =============================================================================
// Logger initialization
// =============================================================================

/// Initialize the Tide compiler logger.
///
/// This is a convenience function for CLI entry points. Library callers
/// (e.g. nlgc) should initialize their own logger.
pub fn init_tidec_logger() {
    if let Err(err) = tidec_log::Logger::init_logger(
        tidec_log::LoggerConfig::from_prefix("TIDEC").unwrap(),
        tidec_log::FallbackDefaultEnv::No,
    ) {
        eprintln!("Error initializing tidec logger: {:?}", err);
        std::process::exit(1);
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_llvm_object() {
        let config = CompileConfig::default();
        assert!(matches!(config.backend, BackendKind::Llvm));
        assert!(matches!(config.emit, EmitKind::Object));
    }

    #[test]
    fn shorthand_constructors() {
        let c = CompileConfig::llvm_ir();
        assert!(matches!(c.backend, BackendKind::Llvm));
        assert!(matches!(c.emit, EmitKind::LlvmIr));

        let c = CompileConfig::llvm_assembly();
        assert!(matches!(c.backend, BackendKind::Llvm));
        assert!(matches!(c.emit, EmitKind::Assembly));

        let c = CompileConfig::llvm_bitcode();
        assert!(matches!(c.backend, BackendKind::Llvm));
        assert!(matches!(c.emit, EmitKind::LlvmBitcode));

        let c = CompileConfig::llvm_executable();
        assert!(matches!(c.backend, BackendKind::Llvm));
        assert!(matches!(c.emit, EmitKind::Executable));

        let c = CompileConfig::llvm_object();
        assert!(matches!(c.backend, BackendKind::Llvm));
        assert!(matches!(c.emit, EmitKind::Object));
    }

    #[test]
    fn config_is_copy() {
        let c1 = CompileConfig::llvm_ir();
        let c2 = c1; // Copy
        let c3 = c1; // Still valid — c1 was copied, not moved.
        assert!(matches!(c2.emit, EmitKind::LlvmIr));
        assert!(matches!(c3.emit, EmitKind::LlvmIr));
    }

    #[test]
    fn unsupported_backend_error_display() {
        let err = CompileError::UnsupportedBackend("cranelift".into());
        assert_eq!(err.to_string(), "unsupported backend: cranelift");
    }

    #[test]
    fn codegen_error_display() {
        let err = CompileError::CodegenError("something went wrong".into());
        assert_eq!(err.to_string(), "codegen error: something went wrong");
    }
}
