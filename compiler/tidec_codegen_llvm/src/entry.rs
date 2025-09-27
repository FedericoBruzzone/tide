use crate::{builder::CodegenBuilder, context::CodegenCtx};
use inkwell::context::Context;
use tidec_codegen_ssa::traits::CodegenMethods;
use tidec_lir::lir::{LirCtx, LirUnit};
use tracing::instrument;

#[instrument(level = "info", skip(lir_ctx, lir_unit), fields(unit = %lir_unit.metadata.unit_name))]
// TODO(bruzzone): try to move it to `tidec_codegen_ssa`
pub fn llvm_codegen_lir_unit(lir_ctx: LirCtx, lir_unit: LirUnit) {
    let ll_context = Context::create();
    let ll_module = ll_context.create_module(&lir_unit.metadata.unit_name);
    let ctx = CodegenCtx::new(lir_ctx, &ll_context, ll_module);

    ctx.compile_lir_unit::<CodegenBuilder>(lir_unit);
    ctx.emit_output();
}
