use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use tidec_tir::syntax::TirTy;

use crate::context::CodegenCtx;

/// A trait to convert TirTy into LLVM BasicTypeEnum and BasicMetadataTypeEnum.
///
/// We need to do this due to the orphan rule in Rust. This could cause the
/// stop of the compilation process of an external crate.
pub trait BasicTypesUtils<'ll> {
    fn into_basic_type_metadata(self, ctx: &CodegenCtx<'ll>) -> BasicMetadataTypeEnum<'ll>;
    fn into_basic_type(self, ctx: &CodegenCtx<'ll>) -> BasicTypeEnum<'ll>;
}

impl<'ll> BasicTypesUtils<'ll> for TirTy {
    fn into_basic_type_metadata(self, ctx: &CodegenCtx<'ll>) -> BasicMetadataTypeEnum<'ll> {
        match self {
            TirTy::I8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()).into(),
            TirTy::I16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()).into(),
            TirTy::I32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()).into(),
            TirTy::I64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()).into(),
            TirTy::I128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()).into(),
            TirTy::U8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()).into(),
            TirTy::U16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()).into(),
            TirTy::U32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()).into(),
            TirTy::U64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()).into(),
            TirTy::U128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()).into(),
            TirTy::F16 => BasicTypeEnum::FloatType(ctx.ll_context.f16_type()).into(),
            TirTy::F32 => BasicTypeEnum::FloatType(ctx.ll_context.f32_type()).into(),
            TirTy::F64 => BasicTypeEnum::FloatType(ctx.ll_context.f64_type()).into(),
            TirTy::F128 => BasicTypeEnum::FloatType(ctx.ll_context.f128_type()).into(),
            TirTy::Metadata => BasicMetadataTypeEnum::MetadataType(ctx.ll_context.metadata_type()),
        }
    }

    fn into_basic_type(self, ctx: &CodegenCtx<'ll>) -> BasicTypeEnum<'ll> {
        match self {
            TirTy::I8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()),
            TirTy::I16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()),
            TirTy::I32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()),
            TirTy::I64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()),
            TirTy::I128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()),
            TirTy::U8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()),
            TirTy::U16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()),
            TirTy::U32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()),
            TirTy::U64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()),
            TirTy::U128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()),
            TirTy::F16 => BasicTypeEnum::FloatType(ctx.ll_context.f16_type()),
            TirTy::F32 => BasicTypeEnum::FloatType(ctx.ll_context.f32_type()),
            TirTy::F64 => BasicTypeEnum::FloatType(ctx.ll_context.f64_type()),
            TirTy::F128 => BasicTypeEnum::FloatType(ctx.ll_context.f128_type()),
            TirTy::Metadata => panic!("Metadata type cannot be converted to BasicTypeEnum"),
        }
    }
}
