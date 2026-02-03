use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::process::Command;

use inkwell::basic_block::BasicBlock;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetData, TargetMachine,
    TargetTriple,
};
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum, FunctionType};
use inkwell::values::{AnyValueEnum, BasicMetadataValueEnum, BasicValueEnum, FunctionValue};
use inkwell::OptimizationLevel;
use tidec_abi::calling_convention::function::{ArgAbi, FnAbi, PassMode};
use tidec_abi::layout::{BackendRepr, TyAndLayout};
use tidec_codegen_ssa::tir;
use tidec_tir::alloc::{AllocId, Allocation, GlobalAlloc};
use tidec_tir::ctx::{EmitKind, TirCtx};
use tidec_tir::TirTy;
use tidec_utils::index_vec::IdxVec;
use tracing::{debug, instrument};

use crate::tir::tir_body_metadata::{
    CallConvUtils, LinkageUtils, UnnamedAddressUtils, VisibilityUtils,
};
use crate::tir::tir_ty::BasicTypesUtils;
use tidec_codegen_ssa::traits::{
    BuilderMethods, CodegenBackend, CodegenBackendTypes, CodegenMethods, DefineCodegenMethods,
    FnAbiOf, LayoutOf, PreDefineCodegenMethods,
};
use tidec_tir::body::{DefId, TirBody, TirBodyMetadata, TirUnit};
use tidec_tir::syntax::{Local, LocalData, RETURN_LOCAL};

// TODO: Add filelds from rustc/compiler/rustc_codegen_llvm/src/context.rs
pub struct CodegenCtx<'ctx, 'll> {
    // FIXME: Make this private
    pub ll_context: &'ll Context,
    // FIXME: Make this private
    pub ll_module: Module<'ll>,
    /// The TIR type context.
    pub lir_ctx: TirCtx<'ctx>,
    /// A map from DefId to the LLVM value (usually a function value).
    //
    // FIXME: Consider removing RefCell and using &mut
    //
    // TODO: Probably we could remove this and use only the module to find functions (more efficient?).
    // Something like: `self.ll_module.get_function(<name>)` (see `get_fn`).
    pub instances: RefCell<HashMap<DefId, AnyValueEnum<'ll>>>,
}

impl<'ll, 'ctx> Deref for CodegenCtx<'ctx, 'll> {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        self.ll_context
    }
}

impl<'ctx, 'll> CodegenBackendTypes for CodegenCtx<'ctx, 'll> {
    type BasicBlock = BasicBlock<'ll>;
    type FunctionType = FunctionType<'ll>;
    type FunctionValue = FunctionValue<'ll>;
    type Type = BasicTypeEnum<'ll>;
    type Value = BasicValueEnum<'ll>;
    type MetadataType = BasicMetadataTypeEnum<'ll>;
    type MetadataValue = BasicMetadataValueEnum<'ll>;
}

impl<'ll> CodegenBackend for CodegenCtx<'_, 'll> {
    type Context = Context;
    type Module = Module<'ll>;
}

impl<'ctx> PreDefineCodegenMethods<'ctx> for CodegenCtx<'ctx, '_> {
    fn predefine_body(
        &self,
        lir_body_metadata: &TirBodyMetadata,
        lir_body_ret_and_args: &IdxVec<Local, LocalData<'ctx>>,
    ) {
        let name = lir_body_metadata.name.as_str();

        let ret_ty = lir_body_ret_and_args[RETURN_LOCAL].ty.into_basic_type(self);
        let formal_param_tys = lir_body_ret_and_args.as_slice()[RETURN_LOCAL.next()..]
            .iter()
            .map(|local_data| local_data.ty.into_basic_type_metadata(self))
            .collect::<Vec<_>>();
        let fn_ty = self.declare_fn(
            ret_ty,
            formal_param_tys.as_slice(),
            lir_body_metadata.is_varargs,
        );
        let linkage = lir_body_metadata.linkage.into_linkage();
        let calling_convention = lir_body_metadata.call_conv.into_call_conv();
        let fn_val = self.ll_module.add_function(name, fn_ty, Some(linkage));
        fn_val.set_call_conventions(calling_convention);

        let fn_global_value = fn_val.as_global_value();
        let visibility = lir_body_metadata.visibility.into_visibility();
        fn_global_value.set_visibility(visibility);
        let unnamed_addr = lir_body_metadata.unnamed_address.into_unnamed_address();
        fn_global_value.set_unnamed_address(unnamed_addr);

        debug!(
            "get_or_declare_fn((name: {}, ret_ty: {:?}, param_tys: {:?}, linkage: {:?}, visibility: {:?}, calling_convention: {:?}, unnamed_addr: {:?})) delared",
            name, ret_ty, formal_param_tys, linkage, visibility, calling_convention, unnamed_addr
        );

        self.instances.borrow_mut().insert(
            lir_body_metadata.def_id,
            AnyValueEnum::FunctionValue(fn_val),
        );
    }
}

impl<'ll, 'ctx> DefineCodegenMethods<'ctx> for CodegenCtx<'ctx, 'll> {
    /// For LLVM, we are able to reuse the generic implementation of `define_lir_body`
    /// provided in the `lir` module, as it is generic over the `BuilderMethods` trait.
    fn define_body(&self, lir_body: TirBody<'ctx>) {
        tir::codegen_tir_body::<crate::builder::CodegenBuilder<'_, 'll, 'ctx>>(self, lir_body);
    }
}

impl<'ctx, 'll> LayoutOf<'ctx> for CodegenCtx<'ctx, 'll> {
    fn layout_of(&self, lir_ty: TirTy<'ctx>) -> TyAndLayout<'ctx, TirTy<'ctx>> {
        self.lir_ctx.layout_of(lir_ty)
    }
}

impl<'ctx, 'll> FnAbiOf<'ctx> for CodegenCtx<'ctx, 'll> {
    #[instrument(level = "debug", skip(self))]
    fn fn_abi_of(
        &self,
        lir_ret_and_args: &IdxVec<Local, LocalData<'ctx>>,
    ) -> FnAbi<'ctx, TirTy<'ctx>> {
        let ty_ctx = self.lir_ctx;

        let argument_of = |ty: TirTy<'ctx>| -> ArgAbi<TirTy<'ctx>> {
            let layout = ty_ctx.layout_of(ty);
            let pass_mode = match layout.backend_repr {
                BackendRepr::Scalar(_) => PassMode::Direct,
                BackendRepr::Memory => PassMode::Indirect,
            };
            let mut arg = ArgAbi::new(layout, pass_mode);
            if arg.layout.is_zst() {
                arg.mode = PassMode::Ignore;
            }
            arg
        };

        let ret_arg_abi = argument_of(lir_ret_and_args[RETURN_LOCAL].ty);
        let arg_abis = lir_ret_and_args.as_slice()[RETURN_LOCAL.next()..]
            .iter()
            .map(|local_data| argument_of(local_data.ty))
            .collect();

        FnAbi {
            ret: ret_arg_abi,
            args: arg_abis,
        }
    }
}

impl<'ctx, 'll> CodegenCtx<'ctx, 'll> {
    #[instrument(skip(lir_ctx, ll_context, ll_module))]
    pub fn new(
        lir_ctx: TirCtx<'ctx>,
        ll_context: &'ll Context,
        ll_module: Module<'ll>,
    ) -> CodegenCtx<'ctx, 'll> {
        let internal_target = lir_ctx.target();
        {
            let target_triple_string = internal_target.target_triple_string();
            match target_triple_string {
                Some(ref s) => {
                    ll_module.set_triple(&TargetTriple::create(s));
                    debug!("Using specified target triple: {:?}", s);
                }
                None => {
                    let default_triple = TargetMachine::get_default_triple();
                    ll_module.set_triple(&default_triple);
                    debug!(
                        "No target triple specified, using default: {:?}",
                        default_triple.as_str()
                    );
                }
            }
        }
        {
            // TODO: As TargetData contains methods to know the size, align, etc... for each LLVM type
            // we could consider to store it in a context
            let data_layout_string = internal_target.data_layout_string();
            ll_module.set_data_layout(&TargetData::create(&data_layout_string).get_data_layout());
        }

        CodegenCtx {
            ll_context,
            ll_module,
            lir_ctx,
            instances: RefCell::new(HashMap::new()),
        }
    }

    fn declare_fn(
        &self,
        ret_ty: BasicTypeEnum<'ll>,
        param_tys: &[BasicMetadataTypeEnum<'ll>],
        is_varargs: bool,
    ) -> FunctionType<'ll> {
        let fn_ty = match ret_ty {
            BasicTypeEnum::IntType(int_type) => int_type.fn_type(param_tys, is_varargs),
            BasicTypeEnum::ArrayType(array_type) => array_type.fn_type(param_tys, is_varargs),
            BasicTypeEnum::FloatType(float_type) => float_type.fn_type(param_tys, is_varargs),
            BasicTypeEnum::PointerType(pointer_type) => pointer_type.fn_type(param_tys, is_varargs),
            BasicTypeEnum::StructType(struct_type) => struct_type.fn_type(param_tys, is_varargs),
            BasicTypeEnum::VectorType(vector_type) => vector_type.fn_type(param_tys, is_varargs),
            BasicTypeEnum::ScalableVectorType(scalable_vector_type) => {
                scalable_vector_type.fn_type(param_tys, is_varargs)
            }
        };

        fn_ty
    }

    /// Creates a target machine for code generation.
    ///
    /// Initializes all LLVM targets and creates a target machine based on
    /// the module's target triple, host CPU, and CPU features.
    fn create_target_machine(&self) -> TargetMachine {
        Target::initialize_all(&InitializationConfig::default());
        let triple = self.ll_module.get_triple();
        let features = TargetMachine::get_host_cpu_features().to_string();
        let cpu = TargetMachine::get_host_cpu_name().to_string();
        let target = Target::from_triple(&triple).expect("Failed to get target from triple");
        target
            .create_target_machine(
                &triple,
                &cpu,
                &features,
                OptimizationLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .expect("Failed to create target machine")
    }

    /// Returns the module name as a string.
    fn module_name(&self) -> &str {
        self.ll_module.get_name().to_str().unwrap()
    }

    /// Emits an object file (`.o`) from the LLVM module.
    fn emit_object(&self) {
        self.emit_object_to_path(&format!("{}.o", self.module_name()));
    }

    /// Emits an object file to the specified path.
    ///
    /// This is a helper function used by both `emit_object` and `emit_executable`.
    fn emit_object_to_path(&self, obj_path: &str) {
        let target_machine = self.create_target_machine();
        target_machine
            .write_to_file(&self.ll_module, FileType::Object, Path::new(obj_path))
            .expect("Failed to write object file");
        debug!("Wrote object file to {}", obj_path);
    }

    /// Emits an assembly file (`.s`) from the LLVM module.
    fn emit_assembly(&self) {
        let target_machine = self.create_target_machine();
        let asm_path = format!("{}.s", self.module_name());
        target_machine
            .write_to_file(&self.ll_module, FileType::Assembly, Path::new(&asm_path))
            .expect("Failed to write assembly file");
        debug!("Wrote assembly file to {}", asm_path);
    }

    /// Emits an LLVM IR file (`.ll`) from the LLVM module.
    fn emit_llvm_ir(&self) {
        let ir_path = format!("{}.ll", self.module_name());
        std::fs::write(&ir_path, self.ll_module.print_to_string().to_string())
            .expect("Failed to write LLVM IR file");
        debug!("Wrote LLVM IR file to {}", ir_path);
    }

    /// Emits an LLVM bitcode file (`.bc`) from the LLVM module.
    fn emit_llvm_bitcode(&self) {
        let bc_path = format!("{}.bc", self.module_name());
        if !self.ll_module.write_bitcode_to_path(&bc_path) {
            panic!("Failed to write LLVM bitcode file");
        }
        debug!("Wrote LLVM bitcode file to {}", bc_path);
    }

    /// Emits an executable by first generating an object file and then linking it.
    ///
    /// The linker is determined at compile time based on the host OS:
    /// - Windows: `link.exe`
    /// - macOS/Linux: `cc`
    fn emit_executable(&self) {
        let module_name = self.module_name();
        let obj_path = format!("{}.o", module_name);

        #[cfg(target_os = "windows")]
        let exe_path = format!("{}.exe", module_name);
        #[cfg(not(target_os = "windows"))]
        let exe_path = module_name.to_string();

        // First, generate the object file
        self.emit_object_to_path(&obj_path);
        debug!("Wrote intermediate object file to {}", obj_path);

        // Link the object file into an executable
        self.link_object_to_executable(&obj_path, &exe_path);

        // Clean up the intermediate object file
        if let Err(e) = std::fs::remove_file(&obj_path) {
            debug!("Warning: failed to remove intermediate object file: {}", e);
        }
    }

    /// Links an object file into an executable.
    ///
    /// The linker command is determined at compile time based on the host OS.
    fn link_object_to_executable(&self, obj_path: &str, exe_path: &str) {
        #[cfg(target_os = "windows")]
        let mut linker_cmd = {
            let mut cmd = Command::new("link.exe");
            cmd.arg(format!("/OUT:{}", exe_path)).arg(obj_path);
            cmd
        };

        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let mut linker_cmd = {
            let mut cmd = Command::new("cc");
            cmd.arg("-o").arg(exe_path).arg(obj_path);
            cmd
        };

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        let mut linker_cmd = {
            // Fallback for other Unix-like systems
            let mut cmd = Command::new("cc");
            cmd.arg("-o").arg(exe_path).arg(obj_path);
            cmd
        };

        // Invoke the linker
        let output = linker_cmd.output().expect("Failed to execute linker");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("Linker failed: {}", stderr);
        }

        debug!("Linked executable to {}", exe_path);
    }
}

impl<'ctx, 'll> CodegenMethods<'ctx> for CodegenCtx<'ctx, 'll> {
    fn tir_ctx(&self) -> TirCtx<'ctx> {
        self.lir_ctx
    }

    #[instrument(skip(self, lir_unit))]
    // TODO: Move as a method of `CodegenCtx`?
    fn compile_tir_unit<'a, B: BuilderMethods<'a, 'ctx>>(&self, lir_unit: TirUnit<'ctx>) {
        // Predefine the functions. That is, create the function declarations.
        for lir_body in &lir_unit.bodies {
            self.predefine_body(&lir_body.metadata, &lir_body.ret_and_args);
        }

        // Destructure the TirUnit to get the bodies
        let TirUnit { bodies, .. } = lir_unit;

        // Now that all functions are pre-defined, we can compile the bodies.
        for lir_body in bodies {
            // Skip external declarations (like libc functions) that have no body.
            if lir_body.metadata.is_declaration {
                debug!(
                    "Skipping body definition for declaration: {}",
                    lir_body.metadata.name
                );
                continue;
            }
            // It corresponds to:
            // ```rust
            // for &(mono_item, item_data) in &mono_items {
            //     mono_item.define::<Builder<'_, '_, '_>>(&mut cx, cgu_name.as_str(), item_data);
            // }
            // ```
            // in rustc_codegen_llvm/src/base.rs
            // lir::define_lir_body::<B>(ctx, lir_body);
            self.define_body(lir_body);
        }

        debug!("\n{}", self.ll_module.print_to_string().to_string());
    }

    fn emit_output(&self) {
        assert_ne!(self.ll_module.get_triple(), TargetTriple::create(""));

        match self.tir_ctx().emit_kind() {
            EmitKind::Object => self.emit_object(),
            EmitKind::Assembly => self.emit_assembly(),
            EmitKind::LlvmIr => self.emit_llvm_ir(),
            EmitKind::LlvmBitcode => self.emit_llvm_bitcode(),
            EmitKind::Executable => self.emit_executable(),
        }
    }

    fn get_fn(&self, lir_body_metadata: &TirBodyMetadata) -> Option<FunctionValue<'ll>> {
        let name = lir_body_metadata.name.as_str();

        if let Some(instance) = self.instances.borrow().get(&lir_body_metadata.def_id) {
            debug!("get_fn(name: {}) found in instances", name);
            return Some((*instance).into_function_value());
        }

        if let Some(f) = self.ll_module.get_function(name) {
            debug!("get_fn(name: {}) found in module", name);
            return Some(f);
        }

        debug!("get_fn(name: {}) not found", name);
        None
    }

    /// TODO(bruzzone): We expect this function returns a function value.
    fn get_or_define_fn(
        &self,
        lir_body_metadata: &TirBodyMetadata,
        lir_body_ret_and_args: &IdxVec<Local, LocalData<'ctx>>,
    ) -> FunctionValue<'ll> {
        let name = lir_body_metadata.name.as_str();

        if let Some(fn_val) = self.get_fn(lir_body_metadata) {
            debug!("get_or_define_fn(name: {}) found", name);
            return fn_val;
        }

        // TODO: fallback by declaring the function
        self.predefine_body(lir_body_metadata, lir_body_ret_and_args);
        let fn_val = self
            .get_fn(lir_body_metadata)
            .expect("function should be defined after predefine_body");

        debug!("get_or_define_fn(name: {}) defined", name);
        fn_val
    }

    fn get_fn_by_name(&self, name: &str) -> Option<FunctionValue<'ll>> {
        if let Some(f) = self.ll_module.get_function(name) {
            debug!("get_fn_by_name(name: {}) found in module", name);
            return Some(f);
        }

        debug!("get_fn_by_name(name: {}) not found", name);
        None
    }

    fn global_alloc(&self, alloc_id: AllocId) -> GlobalAlloc<'ctx> {
        self.lir_ctx.get_global_alloc_unwrap(alloc_id)
    }

    fn const_alloc_to_value(&self, alloc: &Allocation) -> BasicValueEnum<'ll> {
        // Create a global constant from the allocation bytes
        let bytes = alloc.bytes();
        let i8_type = self.ll_context.i8_type();
        let array_type = i8_type.array_type(bytes.len() as u32);

        // Create constant values for each byte
        let byte_values: Vec<_> = bytes
            .iter()
            .map(|&b| i8_type.const_int(b as u64, false))
            .collect();
        let const_array = i8_type.const_array(&byte_values);

        // Create a global variable with the constant array
        let global = self.ll_module.add_global(array_type, None, "const_data");
        global.set_initializer(&const_array);
        global.set_constant(true);
        global.set_linkage(inkwell::module::Linkage::Private);
        global.set_unnamed_addr(true);

        // Return a pointer to the global
        global.as_pointer_value().into()
    }

    fn get_fn_from_alloc(&self, alloc_id: AllocId) -> FunctionValue<'ll> {
        let global_alloc = self.global_alloc(alloc_id);
        match global_alloc {
            GlobalAlloc::Function(def_id) => {
                // Look up the function by its DefId
                if let Some(instance) = self.instances.borrow().get(&def_id) {
                    return (*instance).into_function_value();
                }
                panic!("Function with DefId {:?} not found in instances", def_id);
            }
            _ => panic!("Expected Function allocation, got {:?}", global_alloc),
        }
    }
}
