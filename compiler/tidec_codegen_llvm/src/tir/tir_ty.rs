use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use tidec_tir::{ty, TirTy};

use crate::context::CodegenCtx;

/// A trait to convert TirTy into LLVM BasicTypeEnum and BasicMetadataTypeEnum.
///
/// We need to do this due to the orphan rule in Rust. This could cause the
/// stop of the compilation process of an external crate.
pub trait BasicTypesUtils<'ctx, 'll> {
    fn into_basic_type_metadata(self, ctx: &CodegenCtx<'ctx, 'll>) -> BasicMetadataTypeEnum<'ll>;
    fn into_basic_type(self, ctx: &CodegenCtx<'ctx, 'll>) -> BasicTypeEnum<'ll>;
}

impl<'ctx, 'll> BasicTypesUtils<'ctx, 'll> for TirTy<'ctx> {
    fn into_basic_type_metadata(self, ctx: &CodegenCtx<'ctx, 'll>) -> BasicMetadataTypeEnum<'ll> {
        match &**self {
            ty::TirTy::I8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()).into(),
            ty::TirTy::I16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()).into(),
            ty::TirTy::I32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()).into(),
            ty::TirTy::I64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()).into(),
            ty::TirTy::I128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()).into(),
            ty::TirTy::U8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()).into(),
            ty::TirTy::U16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()).into(),
            ty::TirTy::U32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()).into(),
            ty::TirTy::U64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()).into(),
            ty::TirTy::U128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()).into(),
            ty::TirTy::F16 => BasicTypeEnum::FloatType(ctx.ll_context.f16_type()).into(),
            ty::TirTy::F32 => BasicTypeEnum::FloatType(ctx.ll_context.f32_type()).into(),
            ty::TirTy::F64 => BasicTypeEnum::FloatType(ctx.ll_context.f64_type()).into(),
            ty::TirTy::F128 => BasicTypeEnum::FloatType(ctx.ll_context.f128_type()).into(),
            ty::TirTy::RawPtr(_, _) => todo!(),
            ty::TirTy::Metadata => BasicMetadataTypeEnum::MetadataType(ctx.ll_context.metadata_type()),
        }
    }

    fn into_basic_type(self, ctx: &CodegenCtx<'ctx, 'll>) -> BasicTypeEnum<'ll> {
        match &**self {
            ty::TirTy::I8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()),
            ty::TirTy::I16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()),
            ty::TirTy::I32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()),
            ty::TirTy::I64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()),
            ty::TirTy::I128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()),
            ty::TirTy::U8 => BasicTypeEnum::IntType(ctx.ll_context.i8_type()),
            ty::TirTy::U16 => BasicTypeEnum::IntType(ctx.ll_context.i16_type()),
            ty::TirTy::U32 => BasicTypeEnum::IntType(ctx.ll_context.i32_type()),
            ty::TirTy::U64 => BasicTypeEnum::IntType(ctx.ll_context.i64_type()),
            ty::TirTy::U128 => BasicTypeEnum::IntType(ctx.ll_context.i128_type()),
            ty::TirTy::F16 => BasicTypeEnum::FloatType(ctx.ll_context.f16_type()),
            ty::TirTy::F32 => BasicTypeEnum::FloatType(ctx.ll_context.f32_type()),
            ty::TirTy::F64 => BasicTypeEnum::FloatType(ctx.ll_context.f64_type()),
            ty::TirTy::F128 => BasicTypeEnum::FloatType(ctx.ll_context.f128_type()),
            ty::TirTy::RawPtr(_, _) => todo!(),
            ty::TirTy::Metadata => panic!("Metadata type cannot be converted to BasicTypeEnum"),
        }
    }
}
