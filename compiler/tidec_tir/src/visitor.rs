use crate::{
    basic_blocks::BasicBlockData,
    syntax::{Operand, Place, RValue, Statement, Terminator},
    tir::{TirBody, TirUnit},
};

/// A trait for visiting a TIR.
///
/// This trait is inspired by the `rustc_middle::mir::visit::Visitor` trait.
/// Each method of the form `visit_foo` builds on the `super_foo` method.
/// You can override the `visit_foo` methods to implement your custom logic.
///
/// NOTE: It is not a good idea to have a mutable visitor.
pub trait Visitor<'tir> {
    fn visit_unit(&mut self, unit: &'tir TirUnit) {
        self.super_unit(unit);
    }

    fn super_unit(&mut self, unit: &'tir TirUnit) {
        for body in unit.bodies.iter() {
            self.visit_body(body);
        }
    }

    fn visit_body(&mut self, body: &'tir TirBody) {
        self.super_body(body);
    }

    fn super_body(&mut self, body: &'tir TirBody) {
        for block in body.basic_blocks.iter() {
            self.visit_basic_block(block);
        }
    }

    fn visit_basic_block(&mut self, block: &'tir BasicBlockData) {
        self.super_basic_block(block);
    }

    fn super_basic_block(&mut self, block: &'tir BasicBlockData) {
        for statement in &block.statements {
            self.visit_statement(statement);
        }
        self.visit_terminator(&block.terminator);
    }

    fn visit_statement(&mut self, statement: &'tir Statement) {
        self.super_statement(statement);
    }

    fn super_statement(&mut self, statement: &'tir Statement) {
        match statement {
            Statement::Assign(assign) => {
                let (place, rvalue) = &**assign;
                self.visit_place(place);
                self.visit_rvalue(rvalue);
            }
        }
    }

    fn visit_rvalue(&mut self, rvalue: &'tir RValue) {
        self.super_rvalue(rvalue);
    }

    fn super_rvalue(&mut self, rvalue: &'tir RValue) {
        match rvalue {
            RValue::Operand(operand) => self.visit_operand(operand),
            RValue::UnaryOp(_, operand) => self.visit_operand(operand),
            RValue::BinaryOp(_, lhs, rhs) => {
                self.visit_operand(lhs);
                self.visit_operand(rhs);
            }
        }
    }



    fn visit_operand(&mut self, operand: &'tir Operand) {
        self.super_operand(operand);
    }

    fn super_operand(&mut self, operand: &'tir Operand) {
        match operand {
            Operand::Use(place) => self.visit_place(place),
            Operand::Const(_) => {}
        }
    }

    fn visit_place(&mut self, place: &'tir Place) {
        self.super_place(place);
    }

    fn super_place(&mut self, _place: &'tir Place) {}

    fn visit_terminator(&mut self, terminator: &'tir Terminator) {
        self.super_terminator(terminator);
    }

    fn super_terminator(&mut self, _terminator: &'tir Terminator) {}
}