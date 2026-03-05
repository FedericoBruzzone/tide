//! Builder for constructing a [`TirUnit`] (module).
//!
//! A [`UnitBuilder`] provides an ergonomic API for incrementally assembling a
//! complete TIR module. It collects global variables and function bodies and
//! produces a [`TirUnit`] when [`build`](UnitBuilder::build) is called.
//!
//! # Workflow
//!
//! 1. Create a [`UnitBuilder`] with [`UnitBuilder::new`], supplying the module
//!    name.
//! 2. Add global variables with [`add_global`](UnitBuilder::add_global).
//! 3. Add function bodies with [`add_body`](UnitBuilder::add_body).
//! 4. Call [`build`](UnitBuilder::build) to produce the final [`TirUnit`].
//!
//! # Example
//!
//! ```rust,ignore
//! use tidec_builder::UnitBuilder;
//! use tidec_tir::body::*;
//!
//! let mut unit = UnitBuilder::new("my_module");
//! let gid = unit.add_global(my_global);
//! unit.add_body(my_function_body);
//! let tir_unit = unit.build();
//! ```

use tidec_tir::body::{Body, GlobalId, TirBody, TirGlobal, TirUnit, TirUnitMetadata};
use tidec_utils::idx::Idx;
use tidec_utils::index_vec::IdxVec;

/// Builder for constructing a [`TirUnit`].
///
/// See the [module-level documentation](self) for usage details.
pub struct UnitBuilder<'ctx> {
    /// The name of the unit (module).
    unit_name: String,

    /// Accumulated global variables.
    globals: IdxVec<GlobalId, TirGlobal<'ctx>>,

    /// Accumulated function bodies.
    bodies: IdxVec<Body, TirBody<'ctx>>,
}

impl<'ctx> UnitBuilder<'ctx> {
    /// Create a new unit builder for a module with the given name.
    pub fn new(unit_name: impl Into<String>) -> Self {
        Self {
            unit_name: unit_name.into(),
            globals: IdxVec::new(),
            bodies: IdxVec::new(),
        }
    }

    // ────────────────────── Global variables ─────────────────────

    /// Add a global variable to the module.
    ///
    /// Returns the [`GlobalId`] assigned to the global, which can be used
    /// to reference it from function bodies (e.g. via
    /// [`TirCtx::intern_static`]).
    pub fn add_global(&mut self, global: TirGlobal<'ctx>) -> GlobalId {
        self.globals.push(global)
    }

    /// Returns the number of globals added so far.
    pub fn num_globals(&self) -> usize {
        self.globals.len()
    }

    /// Returns `true` if no globals have been added.
    pub fn has_globals(&self) -> bool {
        !self.globals.is_empty()
    }

    /// Returns a shared reference to a previously added global.
    ///
    /// # Panics
    ///
    /// Panics if `id` is out of bounds.
    pub fn get_global(&self, id: GlobalId) -> &TirGlobal<'ctx> {
        &self.globals.raw[id.idx()]
    }

    /// Returns a mutable reference to a previously added global, allowing
    /// in-place modification before the unit is finalized.
    ///
    /// # Panics
    ///
    /// Panics if `id` is out of bounds.
    pub fn get_global_mut(&mut self, id: GlobalId) -> &mut TirGlobal<'ctx> {
        &mut self.globals.raw[id.idx()]
    }

    // ──────────────────── Function bodies ────────────────────────

    /// Add a function body to the module.
    ///
    /// Returns the [`Body`] index assigned to this body within the unit.
    pub fn add_body(&mut self, body: TirBody<'ctx>) -> Body {
        self.bodies.push(body)
    }

    /// Returns the number of function bodies added so far.
    pub fn num_bodies(&self) -> usize {
        self.bodies.len()
    }

    /// Returns `true` if no bodies have been added.
    pub fn has_bodies(&self) -> bool {
        !self.bodies.is_empty()
    }

    /// Returns a shared reference to a previously added body.
    ///
    /// # Panics
    ///
    /// Panics if `id` is out of bounds.
    pub fn get_body(&self, id: &Body) -> &TirBody<'ctx> {
        &self.bodies.raw[id.idx()]
    }

    /// Returns a mutable reference to a previously added body, allowing
    /// in-place modification before the unit is finalized.
    ///
    /// # Panics
    ///
    /// Panics if `id` is out of bounds.
    pub fn get_body_mut(&mut self, id: &Body) -> &mut TirBody<'ctx> {
        &mut self.bodies.raw[id.idx()]
    }

    // ──────────────────── Introspection ──────────────────────────

    /// Returns the module name.
    pub fn unit_name(&self) -> &str {
        &self.unit_name
    }

    // ──────────────────── Finalization ───────────────────────────

    /// Consume the builder and produce the finished [`TirUnit`].
    pub fn build(self) -> TirUnit<'ctx> {
        TirUnit {
            metadata: TirUnitMetadata {
                unit_name: self.unit_name,
            },
            globals: self.globals,
            bodies: self.bodies,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BuilderCtx;
    use tidec_abi::target::{BackendKind, TirTarget};
    use tidec_tir::body::*;
    use tidec_tir::ctx::{EmitKind, InternCtx, TirArena, TirArgs, TirCtx};
    use tidec_tir::syntax::*;
    use tidec_utils::idx::Idx;

    /// Helper to create a `TirCtx` for interning types in tests.
    fn with_ctx<F, R>(f: F) -> R
    where
        F: for<'ctx> FnOnce(TirCtx<'ctx>) -> R,
    {
        let target = TirTarget::new(BackendKind::Llvm);
        let args = TirArgs {
            emit_kind: EmitKind::Object,
        };
        let arena = TirArena::default();
        let intern_ctx = InternCtx::new(&arena);
        let tir_ctx = TirCtx::new(&target, &args, &intern_ctx);
        f(tir_ctx)
    }

    fn make_metadata(name: &str) -> TirBodyMetadata {
        TirBodyMetadata {
            def_id: DefId(0),
            name: name.to_string(),
            kind: TirBodyKind::Item(TirItemKind::Function),
            inlined: false,
            linkage: Linkage::External,
            visibility: Visibility::Default,
            unnamed_address: UnnamedAddress::None,
            call_conv: CallConv::C,
            is_varargs: false,
            is_declaration: false,
        }
    }

    fn make_simple_body<'ctx>(name: &str, ret_ty: tidec_tir::TirTy<'ctx>) -> TirBody<'ctx> {
        use crate::FunctionBuilder;

        let mut fb = FunctionBuilder::new(make_metadata(name));
        fb.declare_ret(ret_ty, false);
        let entry = fb.create_block();
        fb.set_terminator(entry, Terminator::Return);
        fb.build()
    }

    #[test]
    fn empty_unit() {
        let unit = UnitBuilder::new("empty_mod").build();
        assert_eq!(unit.metadata.unit_name, "empty_mod");
        assert!(unit.globals.is_empty());
        assert!(unit.bodies.is_empty());
    }

    #[test]
    fn unit_name_preserved() {
        let ub = UnitBuilder::<'_>::new("my_module");
        assert_eq!(ub.unit_name(), "my_module");
        let unit = ub.build();
        assert_eq!(unit.metadata.unit_name, "my_module");
    }

    #[test]
    fn add_single_body() {
        with_ctx(|ctx| {
            let builder_ctx = BuilderCtx::new(ctx);
            let i32_ty = builder_ctx.i32();

            let mut ub = UnitBuilder::new("one_fn");
            let body = make_simple_body("my_fn", i32_ty);
            let body_id = ub.add_body(body);
            assert!(body_id.idx() == 0);
            assert_eq!(ub.num_bodies(), 1);
            assert!(ub.has_bodies());

            let unit = ub.build();
            assert_eq!(unit.bodies.len(), 1);
            assert_eq!(unit.bodies.raw[0].metadata.name, "my_fn");
        });
    }

    #[test]
    fn add_multiple_bodies() {
        with_ctx(|ctx| {
            let builder_ctx = BuilderCtx::new(ctx);
            let i32_ty = builder_ctx.i32();
            let unit_ty = builder_ctx.unit();

            let mut ub = UnitBuilder::new("multi_fn");
            let b0 = ub.add_body(make_simple_body("fn_a", i32_ty));
            let b1 = ub.add_body(make_simple_body("fn_b", unit_ty));
            let b2 = ub.add_body(make_simple_body("fn_c", i32_ty));

            assert!(b0.idx() == 0);
            assert!(b1.idx() == 1);
            assert!(b2.idx() == 2);
            assert_eq!(ub.num_bodies(), 3);

            let unit = ub.build();
            assert_eq!(unit.bodies.len(), 3);
            assert_eq!(unit.bodies.raw[0].metadata.name, "fn_a");
            assert_eq!(unit.bodies.raw[1].metadata.name, "fn_b");
            assert_eq!(unit.bodies.raw[2].metadata.name, "fn_c");
        });
    }

    #[test]
    fn add_global_scalar() {
        with_ctx(|ctx| {
            let builder_ctx = BuilderCtx::new(ctx);
            let i32_ty = builder_ctx.i32();

            let mut ub = UnitBuilder::new("with_global");

            let scalar = ConstScalar::Value(RawScalarValue {
                data: 42,
                size: std::num::NonZero::new(4).unwrap(),
            });
            let global = TirGlobal {
                name: "counter".to_string(),
                ty: i32_ty,
                initializer: Some(ConstValue::Scalar(scalar)),
                mutable: true,
                linkage: Linkage::External,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::None,
            };

            let gid = ub.add_global(global);
            assert_eq!(gid, GlobalId::new(0));
            assert_eq!(ub.num_globals(), 1);
            assert!(ub.has_globals());

            let unit = ub.build();
            assert_eq!(unit.globals.len(), 1);
            assert_eq!(unit.globals[GlobalId::new(0)].name, "counter");
            assert!(unit.globals[GlobalId::new(0)].mutable);
        });
    }

    #[test]
    fn add_global_declaration_no_initializer() {
        with_ctx(|ctx| {
            let builder_ctx = BuilderCtx::new(ctx);
            let i32_ty = builder_ctx.i32();

            let mut ub = UnitBuilder::new("extern_mod");
            let global = TirGlobal {
                name: "ext_var".to_string(),
                ty: i32_ty,
                initializer: None,
                mutable: false,
                linkage: Linkage::External,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::None,
            };

            let gid = ub.add_global(global);
            let unit = ub.build();
            assert!(unit.globals[gid].initializer.is_none());
        });
    }

    #[test]
    fn get_global_by_id() {
        with_ctx(|ctx| {
            let builder_ctx = BuilderCtx::new(ctx);
            let i32_ty = builder_ctx.i32();

            let mut ub = UnitBuilder::new("get_global");
            let global = TirGlobal {
                name: "my_global".to_string(),
                ty: i32_ty,
                initializer: None,
                mutable: false,
                linkage: Linkage::Internal,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::None,
            };

            let gid = ub.add_global(global);
            assert_eq!(ub.get_global(gid).name, "my_global");
        });
    }

    #[test]
    fn get_global_mut_by_id() {
        with_ctx(|ctx| {
            let builder_ctx = BuilderCtx::new(ctx);
            let i32_ty = builder_ctx.i32();

            let mut ub = UnitBuilder::new("mutate_global");
            let global = TirGlobal {
                name: "orig".to_string(),
                ty: i32_ty,
                initializer: None,
                mutable: false,
                linkage: Linkage::External,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::None,
            };

            let gid = ub.add_global(global);
            ub.get_global_mut(gid).name = "renamed".to_string();
            assert_eq!(ub.get_global(gid).name, "renamed");
        });
    }

    #[test]
    fn get_body_by_id() {
        with_ctx(|ctx| {
            let builder_ctx = BuilderCtx::new(ctx);
            let i32_ty = builder_ctx.i32();

            let mut ub = UnitBuilder::new("get_body");
            let body_id = ub.add_body(make_simple_body("fn_x", i32_ty));
            assert_eq!(ub.get_body(&body_id).metadata.name, "fn_x");
        });
    }

    #[test]
    fn get_body_mut_by_id() {
        with_ctx(|ctx| {
            let builder_ctx = BuilderCtx::new(ctx);
            let i32_ty = builder_ctx.i32();

            let mut ub = UnitBuilder::new("mutate_body");
            let body_id = ub.add_body(make_simple_body("fn_orig", i32_ty));
            ub.get_body_mut(&body_id).metadata.name = "fn_renamed".to_string();
            assert_eq!(ub.get_body(&body_id).metadata.name, "fn_renamed");
        });
    }

    #[test]
    fn unit_with_globals_and_bodies() {
        with_ctx(|ctx| {
            let builder_ctx = BuilderCtx::new(ctx);
            let i32_ty = builder_ctx.i32();
            let unit_ty = builder_ctx.unit();

            let mut ub = UnitBuilder::new("full_module");

            // Add globals
            let g0 = ub.add_global(TirGlobal {
                name: "g0".to_string(),
                ty: i32_ty,
                initializer: Some(ConstValue::ZST),
                mutable: false,
                linkage: Linkage::Internal,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::Local,
            });
            let g1 = ub.add_global(TirGlobal {
                name: "g1".to_string(),
                ty: unit_ty,
                initializer: None,
                mutable: true,
                linkage: Linkage::External,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::None,
            });

            // Add bodies
            let b0 = ub.add_body(make_simple_body("main", unit_ty));
            let b1 = ub.add_body(make_simple_body("helper", i32_ty));

            assert_eq!(ub.num_globals(), 2);
            assert_eq!(ub.num_bodies(), 2);

            let unit = ub.build();
            assert_eq!(unit.metadata.unit_name, "full_module");
            assert_eq!(unit.globals.len(), 2);
            assert_eq!(unit.bodies.len(), 2);
            assert_eq!(unit.globals[g0].name, "g0");
            assert_eq!(unit.globals[g1].name, "g1");
            assert_eq!(unit.bodies.raw[b0.idx()].metadata.name, "main");
            assert_eq!(unit.bodies.raw[b1.idx()].metadata.name, "helper");
        });
    }

    #[test]
    fn has_globals_and_has_bodies_on_empty() {
        let ub = UnitBuilder::<'_>::new("empty");
        assert!(!ub.has_globals());
        assert!(!ub.has_bodies());
        assert_eq!(ub.num_globals(), 0);
        assert_eq!(ub.num_bodies(), 0);
    }

    #[test]
    fn unit_name_from_string() {
        let name = String::from("dynamic_name");
        let ub = UnitBuilder::<'_>::new(name);
        assert_eq!(ub.unit_name(), "dynamic_name");
    }

    #[test]
    fn multiple_globals_indices_are_sequential() {
        with_ctx(|ctx| {
            let builder_ctx = BuilderCtx::new(ctx);
            let i32_ty = builder_ctx.i32();

            let mut ub = UnitBuilder::new("sequential");
            let make_global = |name: &str| TirGlobal {
                name: name.to_string(),
                ty: i32_ty,
                initializer: None,
                mutable: false,
                linkage: Linkage::External,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::None,
            };

            let g0 = ub.add_global(make_global("a"));
            let g1 = ub.add_global(make_global("b"));
            let g2 = ub.add_global(make_global("c"));

            assert_eq!(g0, GlobalId::new(0));
            assert_eq!(g1, GlobalId::new(1));
            assert_eq!(g2, GlobalId::new(2));
        });
    }

    #[test]
    fn global_with_null_ptr_initializer() {
        with_ctx(|ctx| {
            let builder_ctx = BuilderCtx::new(ctx);
            let i32_ty = builder_ctx.i32();
            let ptr_ty = builder_ctx.ptr_imm(i32_ty);

            let mut ub = UnitBuilder::new("null_ptr_mod");
            let gid = ub.add_global(TirGlobal {
                name: "null_g".to_string(),
                ty: ptr_ty,
                initializer: Some(ConstValue::NullPtr),
                mutable: false,
                linkage: Linkage::External,
                visibility: Visibility::Default,
                unnamed_address: UnnamedAddress::None,
            });

            let unit = ub.build();
            assert_eq!(unit.globals[gid].initializer, Some(ConstValue::NullPtr));
        });
    }
}
