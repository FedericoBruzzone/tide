use crate::{
    tir::{OperandVal, PlaceRef},
    traits::{CodegenMethods, LayoutOf, FnAbiOf},
};
use tidec_abi::{
    calling_convention::function::PassMode,
    layout::TyAndLayout,
};
use tidec_tir::{
    body::TirBody, syntax::{BasicBlock, BasicBlockData, BinaryOp, Local, Operand, Place, RValue, Statement, Terminator, UnaryOp, RETURN_LOCAL}, TirTy
};
use tidec_utils::index_vec::IdxVec;
use tracing::{debug, info, instrument};

use crate::{
    tir::{LocalRef, OperandRef},
    traits::BuilderMethods,
};

pub struct FnCtx<'be, 'ctx, B: BuilderMethods<'be, 'ctx>> {
    /// The body of the function in TIR.
    pub lir_body: TirBody<'ctx>,

    /// The function value.
    /// This is the function that will be generated.
    pub fn_value: B::FunctionValue,

    /// The codegen context.
    pub ctx: &'be B::CodegenCtx,

    /// The allocated locals and temporaries for the function.
    ///
    /// Note that the `B::Value` type is used to represent the local references.
    pub locals: IdxVec<Local, LocalRef<'ctx, B::Value>>,

    /// A cache of the basic blocks in the function.
    /// This is also used to avoid creating multiple basic blocks for the same TIR basic block.
    pub cached_bbs: IdxVec<BasicBlock, Option<B::BasicBlock>>,
}

impl<'ll, 'ctx, B: BuilderMethods<'ll, 'ctx>> FnCtx<'ll, 'ctx, B> {
    /// Codegen the given TIR basic block.
    /// This creates a new builder for the basic block and generates the instructions in it.
    /// It also updates the `cached_bbs` field to avoid creating multiple basic blocks for the same TIR basic block.
    /// Note that this function does not handle unreachable blocks.
    pub fn codegen_basic_block(&mut self, bb: BasicBlock) {
        let be_bb = self.get_or_insert_bb(bb);
        let builder = &mut B::build(self.ctx, be_bb);
        let bb_data: BasicBlockData<'ctx> = self.lir_body.basic_blocks[bb].clone();
        debug!("Codegen basic block {:?}: {:?}", bb, bb_data);
        for stmt in &bb_data.statements {
            self.codegen_statement(builder, stmt);
        }
        let term = &bb_data.terminator;
        self.codegen_terminator(builder, term);
    }

    /// Get the backend basic block for the given TIR basic block.
    /// If it does not exist, create it and cache it.
    pub fn get_or_insert_bb(&mut self, bb: BasicBlock) -> B::BasicBlock {
        if let Some(Some(be_bb)) = self.cached_bbs.get(bb) {
            return *be_bb;
        }

        let be_bb = B::append_basic_block(self.ctx, self.fn_value, &format!("bb{:?}", bb));
        self.cached_bbs[bb] = Some(be_bb);
        be_bb
    }

    #[instrument(level = "debug", skip(self, builder))]
    /// Codegen the given TIR statement.
    /// This function is called by `codegen_basic_block` for each statement in the basic block.
    /// It generates the corresponding instructions in the backend.
    fn codegen_statement(&mut self, builder: &mut B, stmt: &Statement<'ctx>) {
        // TODO(bruzzone): handle span for debugging here
        match stmt {
            Statement::Assign(assig) => {
                let place = &assig.0;
                let rvalue = &assig.1;
                match place.try_local() {
                    Some(local) => {
                        debug!("Assigning to local {:?}", local);
                        match self.locals[local] {
                            LocalRef::PlaceRef(place_ref) => {
                                self.codegen_rvalue(builder, place_ref, rvalue)
                            }
                            LocalRef::OperandRef(operand_ref) => {
                                // We cannot assign to an operand ref that is not a ZST
                                // because operand refs are immutable. That is, we cannot change
                                // the value of an operand ref. However, we can assign to a ZST
                                // because it has no value.
                                if !operand_ref.ty_layout.is_zst() {
                                    // TODO: handle this error properly
                                    panic!("Cannot assign to non-ZST operand ref");
                                }

                                // For ZST, we can just ignore the assignment
                                // but we still need to codegen the rvalue
                                // to handle any side effects it may have.
                                // For example, if the rvalue is a function call
                                // that may panic, we need to codegen it.
                                self.codegen_rvalue_operand(builder, rvalue);
                            }
                            LocalRef::PendingOperandRef => {
                                let operand = self.codegen_rvalue_operand(builder, rvalue);
                                self.overwrite_local(local, LocalRef::OperandRef(operand));
                            }
                        }
                    }
                    None => {
                        todo!(
                            "Handle assignment to non-local places - we have to generate the place and the rvalue"
                        );
                        // let place_dest = self.codegen_place(bx, place.as_ref());
                        // self.codegen_rvalue(bx, place_dest, rvalue);
                    }
                }
            }
        }
    }

    pub fn codegen_rvalue(
        &mut self,
        _builder: &mut B,
        _place_ref: PlaceRef<'ctx, B::Value>,
        _rvalue: &RValue<'ctx>,
    ) {
        todo!("Implement codegen_rvalue");
    }

    #[instrument(level = "debug", skip(self, builder))]
    /// Codegen the given TIR rvalue and return the corresponding operand reference.
    /// It generates the code for the rvalue and returns the operand reference.
    pub fn codegen_rvalue_operand(
        &mut self,
        builder: &mut B,
        rvalue: &RValue<'ctx>,
    ) -> OperandRef<'ctx, B::Value> {
        match rvalue {
            RValue::Operand(operand) => self.codegen_operand(builder, operand),
            RValue::UnaryOp(unary_op, operand) => {
                let OperandRef {
                    operand_val,
                    ty_layout,
                } = self.codegen_operand(builder, operand);
                let is_float = ty_layout.ty.is_floating_point();

                // Only immediate operands are supported because we need to generate
                // a single instruction for the unary operation.
                assert!(operand_val.is_immediate());

                let operand_val = match unary_op {
                    UnaryOp::Pos => operand_val.immediate(), // No-op for positive
                    UnaryOp::Neg => {
                        if is_float {
                            builder.build_fneg(operand_val.immediate())
                        } else {
                            builder.build_neg(operand_val.immediate())
                        }
                    }
                };
                OperandRef::new_immediate(operand_val, ty_layout)
            }
            RValue::BinaryOp(bin_op, lhs, rhs) => {
                let lhs_ref = self.codegen_operand(builder, lhs);
                let rhs_ref = self.codegen_operand(builder, rhs);

                let operand_val = match (lhs_ref.operand_val, rhs_ref.operand_val) {
                    (OperandVal::Immediate(lhs_val), OperandVal::Immediate(rhs_val)) => self
                        .codegen_scalar_binary_op(
                            builder,
                            bin_op,
                            lhs_val,
                            rhs_val,
                            lhs_ref.ty_layout,
                        ),
                    (
                        OperandVal::Pair(_lhs_addr, _lhs_extra),
                        OperandVal::Pair(_rhs_addr, _rhs_extra),
                    ) => {
                        // Wide pointer binary operations are not supported yet.
                        todo!("Handle binary operations on pairs (wide pointers)")
                    }
                    _ => panic!(
                        "Binary operations on non-immediate or non-pair operands are not supported"
                    ),
                };

                let ctx = builder.ctx();
                let tir_ctx = ctx.tir_ctx();
                let result_ty = bin_op.ty(&tir_ctx, lhs_ref.ty_layout.ty, rhs_ref.ty_layout.ty);
                let result_layout = ctx.layout_of(result_ty);
                OperandRef::new_immediate(operand_val, result_layout)
            }
        }
    }

    /// Codegen a scalar binary operation.
    /// This function generates the code for the binary operation and returns the resulting value.
    ///
    /// Note that we only need the type layout of the lhs because
    /// in the TIR, both operands of a binary operation must have the same type.
    fn codegen_scalar_binary_op(
        &mut self,
        builder: &mut B,
        bin_op: &BinaryOp,
        lhs: B::Value,
        rhs: B::Value,
        lhs_ty_layout: TyAndLayout<TirTy>,
    ) -> B::Value {
        let is_float = lhs_ty_layout.ty.is_floating_point();
        let is_signed = lhs_ty_layout.ty.is_signed_integer();

        match bin_op {
            BinaryOp::Add => {
                if is_float {
                    builder.build_fadd(lhs, rhs)
                } else {
                    builder.build_add(lhs, rhs)
                }
            }
            BinaryOp::AddUnchecked => {
                if is_signed {
                    builder.build_sadd_unchecked(lhs, rhs)
                } else {
                    builder.build_uadd_unchecked(lhs, rhs)
                }
            }
            BinaryOp::Sub => {
                if is_float {
                    builder.build_fsub(lhs, rhs)
                } else {
                    builder.build_sub(lhs, rhs)
                }
            }
            BinaryOp::SubUnchecked => {
                if is_signed {
                    builder.build_ssub_unchecked(lhs, rhs)
                } else {
                    builder.build_usub_unchecked(lhs, rhs)
                }
            }
            BinaryOp::Mul => {
                if is_float {
                    builder.build_fmul(lhs, rhs)
                } else {
                    builder.build_mul(lhs, rhs)
                }
            }
            BinaryOp::MulUnchecked => {
                if is_signed {
                    builder.build_smul_unchecked(lhs, rhs)
                } else {
                    builder.build_umul_unchecked(lhs, rhs)
                }
            }
            BinaryOp::Div => {
                if is_float {
                    builder.build_fdiv(lhs, rhs)
                } else if is_signed {
                    builder.build_sdiv(lhs, rhs)
                } else {
                    builder.build_udiv(lhs, rhs)
                }
            }
        }
    }

    fn codegen_operand(&mut self, builder: &mut B, operand: &Operand<'ctx>) -> OperandRef<'ctx, B::Value> {
        match operand {
            Operand::Const(const_operand) => {
                OperandRef::from_const(builder, const_operand.value(), const_operand.ty())
            }
            Operand::Use(place) => self.codegen_consume(builder, place),
        }
    }

    /// Overwrite the local reference for the given local.
    /// This is used to update the local reference when we have a pending operand ref
    /// that needs to be replaced with a concrete operand ref.
    /// This is typically done when we first create a pending operand ref
    /// and then later we codegen the rvalue and get the actual operand ref.
    fn overwrite_local(&mut self, local: Local, new_ref: LocalRef<'ctx, B::Value>) {
        debug!("Overwriting local {:?} with {:?}", local, new_ref);
        self.locals[local] = new_ref;
    }

    /// Codegen the given TIR terminator.
    /// This function is called by `codegen_basic_block` for the terminator of the basic block.
    /// It generates the corresponding instructions in the backend.
    fn codegen_terminator(&mut self, builder: &mut B, term: &Terminator<'ctx>) {
        debug!("Codegen terminator: {:?}", term);
        match term {
            Terminator::Return => self.codegen_return_terminator(builder),
            Terminator::Call { func, args, destination, target } => self.codegen_call_terminator(builder, func, args, destination, *target),
        }
    }

    fn codegen_call_terminator(
        &mut self,
        builder: &mut B,
        func: &Operand<'ctx>,
        args: &[Operand<'ctx>],
        _destination: &Place,
        _target: BasicBlock,
    ) {
        // This is the callee function reference. `func` is either a function pointer or a direct function.
        let _func_ref = self.codegen_operand(builder, &func);
        let _arg_vals: Vec<B::Value> = args
            .iter()
            .map(|arg| {
                let arg_ref = self.codegen_operand(builder, arg);
                match arg_ref.operand_val {
                    OperandVal::Immediate(val) => val,
                    OperandVal::Pair(_, _) => {
                        todo!("Handle wide pointer arguments in function calls")
                    }
                    OperandVal::Ref(_) => {
                        panic!("Cannot pass argument by reference in function call")
                    }
                    OperandVal::Zst => {
                        panic!("Cannot pass ZST argument in function call")
                    }
                }
            })
            .collect();

        // let ret_val = match func_ref.operand_val {
        //     OperandVal::Immediate(func_val) => builder.build_call(func_val, &arg_vals),
        //     OperandVal::Pair(_, _) => {
        //         todo!("Handle wide pointer function calls")
        //     }
        //     OperandVal::Ref(_) => {
        //         panic!("Cannot call a function by reference");
        //     }
        //     OperandVal::Zst => {
        //         panic!("Cannot call a ZST function");
        //     }
        // };

        // // Handle the return value based on the function ABI
        // match self.fn_abi.ret.mode {
        //     PassMode::Ignore | PassMode::Indirect => {
        //         // Nothing to do for ignored or indirect returns
        //     }
        //     PassMode::Direct => {
        //         let dest_ref = self.codegen_place(destination);
        //         builder.store_operand(&dest_ref, &OperandRef::new_immediate(ret_val, self.fn_abi.ret.layout));
        //     }
        // }

        // // Jump to the target basic block
        // let be_target_bb = self.get_or_insert_bb(target);
        // builder.build_br(be_target_bb);
    }

    /// Codegen a return terminator.
    /// This function generates the return instruction for the function.
    /// It handles different return modes based on the function ABI.
    fn codegen_return_terminator(&mut self, builder: &mut B) {
        let fn_abi = self.ctx.fn_abi_of(&self.lir_body.ret_and_args);
        let be_val = match fn_abi.ret.mode {
            PassMode::Ignore | PassMode::Indirect => {
                info!("Handling ignored or indirect return");
                builder.build_return(None);
                return;
            }
            PassMode::Direct => {
                info!("Handling direct return");
                let operand_ref = self.codegen_consume(builder, &RETURN_LOCAL.into());
                match operand_ref.operand_val {
                    OperandVal::Zst => todo!("Handle return of ZST. Should be unreachable?"),
                    OperandVal::Ref(_) => todo!("Handle return by reference — load from place"),
                    OperandVal::Pair(_, _) => {
                        todo!("Handle return of pair. That is, create an LLVM pair and return it")
                    }
                    OperandVal::Immediate(val) => val,
                }
            }
        };

        builder.build_return(Some(be_val));
    }

    #[instrument(level = "debug", skip(self, builder))]
    /// Codegen a consume of the given TIR place.
    /// This function generates the code to load the value from the place.
    /// It handles different cases based on the layout of the place.
    /// For example, if the place is a ZST, it returns a ZST operand.
    /// If the place is already an operand ref, it returns it directly.
    /// Otherwise, it generates the place and loads the value from it.
    fn codegen_consume(&mut self, builder: &mut B, place: &Place) -> OperandRef<'ctx, B::Value> {
        let layout = builder
            .ctx()
            .layout_of(self.lir_body.ret_and_args[place.local].ty);

        if layout.is_zst() {
            return OperandRef::new_zst(layout);
        }

        if let Some(operand_ref) = self.try_codegen_consume_operand(place) {
            return operand_ref;
        }

        let place_ref = self.codegen_place(place);
        builder.load_operand(&place_ref)
    }

    fn try_codegen_consume_operand(&self, place: &Place) -> Option<OperandRef<'ctx, B::Value>> {
        match self.locals[place.local] {
            LocalRef::OperandRef(operand_ref) => {
                // TODO(bruzzone): we should handle projections here
                Some(operand_ref)
            }
            _ => None,
        }
    }

    #[instrument(level = "debug", skip(self))]
    /// Codegen the given TIR place.
    /// This function generates the place reference for the given TIR place.
    /// It currently only handles local places.
    // TODO(bruzzone): consider add the builder as parameter to this function
    fn codegen_place(&mut self, place: &Place) -> PlaceRef<'ctx, B::Value> {
        let local = place.local;
        match self.locals[local] {
            LocalRef::PlaceRef(place_ref) => place_ref,
            LocalRef::OperandRef(operand_ref) => {
                panic!(
                    "Cannot convert an operand ref {:?} to a place ref for local {:?}",
                    operand_ref, local
                );
            }
            LocalRef::PendingOperandRef => {
                panic!(
                    "Cannot consume a pending operand ref {:?} before it is defined",
                    local
                );
            }
        }

        // TODO(bruzzone): handle projections
    }
}
