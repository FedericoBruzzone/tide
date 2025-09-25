use crate::{
    lir::{OperandVal, PlaceRef},
    traits::LayoutOf,
};
use tidec_abi::calling_convention::function::{FnAbi, PassMode};
use tidec_tir::{
    basic_blocks::{BasicBlock, BasicBlockData},
    lir::LirBody,
    syntax::{LirTy, Local, RETURN_LOCAL, RValue, Statement, Terminator},
};
use tidec_utils::index_vec::IdxVec;
use tracing::{debug, info, instrument};

use crate::{
    lir::{LocalRef, OperandRef},
    traits::BuilderMethods,
};

pub struct FnCtx<'a, 'be, B: BuilderMethods<'a, 'be>> {
    /// The function ABI.
    /// This contains information about the calling convention,
    /// argument types, return type, etc.
    pub fn_abi: FnAbi<LirTy>,

    /// The body of the function in LIR.
    pub lir_body: &'a LirBody,

    /// The function value.
    /// This is the function that will be generated.
    pub fn_value: B::FunctionValue,

    /// The codegen context.
    pub ctx: &'a B::CodegenCtx,

    /// The allocated locals and temporaries for the function.
    ///
    /// Note that the `B::Value` type is used to represent the local references.
    pub locals: IdxVec<Local, LocalRef<B::Value>>,

    /// A cache of the basic blocks in the function.
    /// This is also used to avoid creating multiple basic blocks for the same LIR basic block.
    pub cached_bbs: IdxVec<BasicBlock, Option<B::BasicBlock>>,
}

impl<'ctx, 'll, B: BuilderMethods<'ctx, 'll>> FnCtx<'ctx, 'll, B> {
    /// Codegen the given LIR basic block.
    /// This creates a new builder for the basic block and generates the instructions in it.
    /// It also updates the `cached_bbs` field to avoid creating multiple basic blocks for the same LIR basic block.
    /// Note that this function does not handle unreachable blocks.
    pub fn codegen_basic_block(&mut self, bb: BasicBlock) {
        let be_bb = self.get_or_insert_bb(bb);
        let mut builder = B::build(self.ctx, be_bb);
        let bb_data: &BasicBlockData = &self.lir_body.basic_blocks[bb];
        debug!("Codegen basic block {:?}: {:?}", bb, bb_data);
        for stmt in &bb_data.statements {
            self.codegen_statement(&mut builder, stmt);
        }
        let term = &bb_data.terminator;
        self.codegen_terminator(&mut builder, term);
    }

    /// Get the backend basic block for the given LIR basic block.
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
    /// Codegen the given LIR statement.
    /// This function is called by `codegen_basic_block` for each statement in the basic block.
    /// It generates the corresponding instructions in the backend.
    fn codegen_statement(&mut self, builder: &mut B, stmt: &Statement) {
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
        _place_ref: PlaceRef<B::Value>,
        _rvalue: &RValue,
    ) {
        todo!("Implement codegen_rvalue");
    }

    pub fn codegen_rvalue_operand(
        &mut self,
        builder: &mut B,
        rvalue: &RValue,
    ) -> OperandRef<B::Value> {
        match rvalue {
            RValue::Const(const_operand) => {
                OperandRef::from_const(builder, const_operand.value(), const_operand.ty())
            }
        }
    }

    fn overwrite_local(&mut self, local: Local, new_ref: LocalRef<B::Value>) {
        self.locals[local] = new_ref;
    }

    /// Codegen the given LIR terminator.
    /// This function is called by `codegen_basic_block` for the terminator of the basic block.
    /// It generates the corresponding instructions in the backend.
    fn codegen_terminator(&mut self, builder: &mut B, term: &Terminator) {
        debug!("Codegen terminator: {:?}", term);
        match term {
            Terminator::Return => self.codegen_return_terminator(builder),
        }
    }

    /// Codegen a return terminator.
    /// This function generates the return instruction for the function.
    /// It handles different return modes based on the function ABI.
    fn codegen_return_terminator(&mut self, builder: &mut B) {
        let be_val = match self.fn_abi.ret.mode {
            PassMode::Ignore | PassMode::Indirect => {
                info!("Handling ignored or indirect return");
                builder.build_return(None);
                return;
            }
            PassMode::Direct => {
                info!("Handling direct return");
                let operand_ref = self.codegen_consume(builder, RETURN_LOCAL);
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

    fn codegen_consume(&mut self, builder: &mut B, local: Local) -> OperandRef<B::Value> {
        let layout = builder
            .ctx()
            .layout_of(self.lir_body.ret_and_args[local].ty);

        if layout.is_zst() {
            return OperandRef::new_zst(layout);
        }

        let local_ref = &self.locals[local];
        match local_ref {
            LocalRef::OperandRef(operand_ref) => {
                // TODO(bruzzone): we should handle projections here
                *operand_ref
            }
            LocalRef::PlaceRef(place_ref) => builder.load_operand(place_ref),
            LocalRef::PendingOperandRef => {
                panic!(
                    "Cannot consume a pending operand ref {:?} before it is defined",
                    local_ref
                );
            }
        }

        // for most places, to consume them we just load them
        // out from their home
        // let place = self.codegen_place(bx, place_ref);
        // bx.load_operand(place)
    }
}
