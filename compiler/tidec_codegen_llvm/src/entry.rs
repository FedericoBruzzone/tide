use crate::{builder::CodegenBuilder, context::CodegenCtx};
use inkwell::context::Context;
use tidec_codegen_ssa::traits::CodegenMethods;
use tidec_tir::{body::TirUnit, ctx::TirCtx};
use tracing::instrument;

#[instrument(level = "info", skip(tir_ctx, lir_unit), fields(unit = %lir_unit.metadata.unit_name))]
// TODO(bruzzone): try to move it to `tidec_codegen_ssa`
pub fn llvm_codegen_lir_unit<'ctx>(tir_ctx: TirCtx<'ctx>, lir_unit: TirUnit<'ctx>) {
    let ll_context = Context::create();
    let ll_module = ll_context.create_module(&lir_unit.metadata.unit_name);
    let ctx = CodegenCtx::new(tir_ctx, &ll_context, ll_module);

    ctx.compile_tir_unit::<CodegenBuilder<'_, '_, 'ctx>>(lir_unit);
    ctx.emit_output();
}
