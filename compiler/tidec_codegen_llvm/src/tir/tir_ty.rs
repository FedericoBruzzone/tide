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
            ty::TirTy::Unit => panic!("Unit/void type cannot be converted to BasicMetadataTypeEnum; handle void returns separately"),
            ty::TirTy::Bool => BasicTypeEnum::IntType(ctx.ll_context.bool_type()).into(),
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
            ty::TirTy::RawPtr(_, _) => {
                // In LLVM's opaque pointer model, all pointers are just `ptr`
                BasicTypeEnum::PointerType(ctx.ll_context.ptr_type(Default::default())).into()
            }
            ty::TirTy::Struct { fields, packed } => {
                let basic_fields: Vec<BasicTypeEnum<'ll>> = fields
                    .as_slice()
                    .iter()
                    .map(|f| f.into_basic_type(ctx))
                    .collect();
                BasicTypeEnum::StructType(
                    ctx.ll_context.struct_type(&basic_fields, *packed),
                )
                .into()
            }
            ty::TirTy::Array(element_ty, count) => {
                let elem_llty = element_ty.into_basic_type(ctx);
                match elem_llty {
                    BasicTypeEnum::IntType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)).into(),
                    BasicTypeEnum::FloatType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)).into(),
                    BasicTypeEnum::PointerType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)).into(),
                    BasicTypeEnum::StructType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)).into(),
                    BasicTypeEnum::ArrayType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)).into(),
                    BasicTypeEnum::VectorType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)).into(),
                    #[allow(unreachable_patterns)]
                    _ => panic!("Unsupported array element type: {:?}", elem_llty),
                }
            }
            ty::TirTy::Metadata => {
                BasicMetadataTypeEnum::MetadataType(ctx.ll_context.metadata_type())
            }
        }
    }

    fn into_basic_type(self, ctx: &CodegenCtx<'ctx, 'll>) -> BasicTypeEnum<'ll> {
        match &**self {
            ty::TirTy::Unit => panic!("Unit/void type cannot be converted to BasicTypeEnum; handle void returns separately"),
            ty::TirTy::Bool => BasicTypeEnum::IntType(ctx.ll_context.bool_type()),
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
            ty::TirTy::RawPtr(_, _) => {
                // In LLVM's opaque pointer model, all pointers are just `ptr`
                BasicTypeEnum::PointerType(ctx.ll_context.ptr_type(Default::default()))
            }
            ty::TirTy::Struct { fields, packed } => {
                let basic_fields: Vec<BasicTypeEnum<'ll>> = fields
                    .as_slice()
                    .iter()
                    .map(|f| f.into_basic_type(ctx))
                    .collect();
                BasicTypeEnum::StructType(ctx.ll_context.struct_type(&basic_fields, *packed))
            }
            ty::TirTy::Array(element_ty, count) => {
                let elem_llty = element_ty.into_basic_type(ctx);
                match elem_llty {
                    BasicTypeEnum::IntType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)),
                    BasicTypeEnum::FloatType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)),
                    BasicTypeEnum::PointerType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)),
                    BasicTypeEnum::StructType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)),
                    BasicTypeEnum::ArrayType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)),
                    BasicTypeEnum::VectorType(t) => BasicTypeEnum::ArrayType(t.array_type(*count as u32)),
                    #[allow(unreachable_patterns)]
                    _ => panic!("Unsupported array element type: {:?}", elem_llty),
                }
            }
            ty::TirTy::Metadata => panic!("Metadata type cannot be converted to BasicTypeEnum"),
        }
    }
}
