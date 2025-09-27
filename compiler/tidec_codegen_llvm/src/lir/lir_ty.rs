use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use tidec_lir::syntax::LirTy;

use crate::context::CodegenCtx;

/// A trait to convert LirTy into LLVM BasicTypeEnum and BasicMetadataTypeEnum.
///
/// We need to do this due to the orphan rule in Rust. This could cause the
/// stop of the compilation process of an external crate.
pub trait BasicTypesUtils<'ll> {
    fn into_basic_type_metadata(self, ctx: &CodegenCtx<'ll>) -> BasicMetadataTypeEnum<'ll>;
    fn into_basic_type(self, ctx: &CodegenCtx<'ll>) -> BasicTypeEnum<'ll>;
}

impl<'ll> BasicTypesUtils<'ll> for LirTy {
    fn into_basic_type_metadata(self, ctx: &CodegenCtx<'ll>) -> BasicMetadataTypeEnum<'ll> {
        match self {
            LirTy::I8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()).into(),
            LirTy::I16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()).into(),
            LirTy::I32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()).into(),
            LirTy::I64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()).into(),
            LirTy::I128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()).into(),
            LirTy::U8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()).into(),
            LirTy::U16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()).into(),
            LirTy::U32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()).into(),
            LirTy::U64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()).into(),
            LirTy::U128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()).into(),
            LirTy::F16 => BasicTypeEnum::FloatType(ctx.ll_context.f16_type()).into(),
            LirTy::F32 => BasicTypeEnum::FloatType(ctx.ll_context.f32_type()).into(),
            LirTy::F64 => BasicTypeEnum::FloatType(ctx.ll_context.f64_type()).into(),
            LirTy::F128 => BasicTypeEnum::FloatType(ctx.ll_context.f128_type()).into(),
            LirTy::Metadata => BasicMetadataTypeEnum::MetadataType(ctx.ll_context.metadata_type()),
        }
    }

    fn into_basic_type(self, ctx: &CodegenCtx<'ll>) -> BasicTypeEnum<'ll> {
        match self {
            LirTy::I8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()),
            LirTy::I16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()),
            LirTy::I32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()),
            LirTy::I64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()),
            LirTy::I128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()),
            LirTy::U8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()),
            LirTy::U16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()),
            LirTy::U32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()),
            LirTy::U64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()),
            LirTy::U128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()),
            LirTy::F16 => BasicTypeEnum::FloatType(ctx.ll_context.f16_type()),
            LirTy::F32 => BasicTypeEnum::FloatType(ctx.ll_context.f32_type()),
            LirTy::F64 => BasicTypeEnum::FloatType(ctx.ll_context.f64_type()),
            LirTy::F128 => BasicTypeEnum::FloatType(ctx.ll_context.f128_type()),
            LirTy::Metadata => panic!("Metadata type cannot be converted to BasicTypeEnum"),
        }
    }
}
