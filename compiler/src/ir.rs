use crate::ast::{BinaryOp, Expr, Function, Item, MatchArm, Module, Pattern, Stmt, UnaryOp};
use crate::diag::Diagnostic;
use crate::semantic::SemanticModel;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValueId(pub usize);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub functions: Vec<FunctionIR>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionIR {
    pub name: String,
    pub params: Vec<String>,
    pub locals: Vec<String>,
    pub ret: String,
    pub blocks: Vec<BasicBlock>,
    pub values: usize,
    pub is_async: bool,
    pub is_ai: bool,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Instruction {
    Const {
        dst: ValueId,
        value: Constant,
    },
    FunctionRef {
        dst: ValueId,
        name: String,
    },
    LoadLocal {
        dst: ValueId,
        slot: usize,
    },
    StoreLocal {
        slot: usize,
        value: ValueId,
    },
    Unary {
        dst: ValueId,
        op: IrUnaryOp,
        value: ValueId,
    },
    Binary {
        dst: ValueId,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    },
    Call {
        dst: ValueId,
        callee: ValueId,
        args: Vec<ValueId>,
    },
    Spawn {
        dst: ValueId,
        callee: ValueId,
        args: Vec<ValueId>,
    },
    Await {
        dst: ValueId,
        task: ValueId,
    },
    MakeArray {
        dst: ValueId,
        items: Vec<ValueId>,
    },
    Index {
        dst: ValueId,
        object: ValueId,
        index: ValueId,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum IrUnaryOp {
    Neg,
    Move,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Terminator {
    Jump(BlockId),
    Branch {
        cond: ValueId,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return(Option<ValueId>),
    Unreachable,
}

pub fn lower_module(module: &Module, semantic: &SemanticModel) -> Result<Program, Vec<Diagnostic>> {
    let mut functions = Vec::new();
    let mut diagnostics = Vec::new();
    for item in &module.items {
        if let Item::Function(function) = item {
            match Lowerer::new(function, semantic).lower() {
                Ok(ir) => functions.push(ir),
                Err(errs) => diagnostics.extend(errs),
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(Program { functions })
    } else {
        Err(diagnostics)
    }
}

pub fn optimize_program(program: &mut Program) {
    for function in &mut program.functions {
        constant_fold(function);
        dead_code_eliminate(function);
    }
}

struct Lowerer<'a> {
    function: &'a Function,
    semantic: &'a SemanticModel,
    blocks: Vec<BasicBlock>,
    current: BlockId,
    locals: HashMap<String, usize>,
    local_names: Vec<String>,
    next_slot: usize,
    values: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lowerer<'a> {
    fn new(function: &'a Function, semantic: &'a SemanticModel) -> Self {
        let entry = BlockId(0);
        Self {
            function,
            semantic,
            blocks: vec![BasicBlock {
                id: entry,
                instructions: Vec::new(),
                terminator: Terminator::Unreachable,
            }],
            current: entry,
            locals: HashMap::new(),
            local_names: Vec::new(),
            next_slot: 0,
            values: 0,
            diagnostics: Vec::new(),
        }
    }

    fn lower(mut self) -> Result<FunctionIR, Vec<Diagnostic>> {
        for (slot, param) in self.function.params.iter().enumerate() {
            self.locals.insert(param.name.clone(), slot);
            self.local_names.push(param.name.clone());
        }
        self.next_slot = self.function.params.len();
        for stmt in &self.function.body {
            self.lower_stmt(stmt);
        }
        if matches!(self.current_block().terminator, Terminator::Unreachable) {
            self.current_block_mut().terminator = Terminator::Return(None);
        }
        if self.diagnostics.is_empty() {
            let ret = self
                .function
                .return_type
                .as_ref()
                .map(|ty| format!("{:?}", ty.kind))
                .unwrap_or_else(|| "Unit".to_string());
            Ok(FunctionIR {
                name: self.function.name.clone(),
                params: self
                    .function
                    .params
                    .iter()
                    .map(|p| p.name.clone())
                    .collect(),
                locals: self.local_names,
                ret,
                blocks: self.blocks,
                values: self.values,
                is_async: self.function.is_async,
                is_ai: self.function.is_ai,
                prompt: self.function.prompt.clone(),
            })
        } else {
            Err(self.diagnostics)
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt) {
        if !matches!(self.current_block().terminator, Terminator::Unreachable) {
            return;
        }
        match stmt {
            Stmt::Let { name, expr, .. } => {
                let value = self.lower_expr(expr);
                let slot = self.next_slot;
                self.next_slot += 1;
                self.locals.insert(name.clone(), slot);
                self.local_names.push(name.clone());
                self.emit(Instruction::StoreLocal { slot, value });
            }
            Stmt::Assign { target, expr, span } => {
                let value = self.lower_expr(expr);
                match self.locals.get(target).copied() {
                    Some(slot) => self.emit(Instruction::StoreLocal { slot, value }),
                    None => self.diagnostics.push(Diagnostic::error(
                        format!("unknown local '{}'", target),
                        Some(*span),
                    )),
                }
            }
            Stmt::Expr(expr) => {
                let _ = self.lower_expr(expr);
            }
            Stmt::Return { expr, .. } => {
                let value = expr.as_ref().map(|expr| self.lower_expr(expr));
                self.current_block_mut().terminator = Terminator::Return(value);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => self.lower_if(condition, then_branch, else_branch),
            Stmt::While {
                condition, body, ..
            } => self.lower_while(condition, body),
            Stmt::Match { expr, arms, .. } => self.lower_match(expr, arms),
        }
    }

    fn lower_if(&mut self, condition: &Expr, then_branch: &[Stmt], else_branch: &[Stmt]) {
        let then_block = self.new_block();
        let else_block = self.new_block();
        let merge_block = self.new_block();
        let cond = self.lower_expr(condition);
        self.current_block_mut().terminator = Terminator::Branch {
            cond,
            then_block,
            else_block,
        };

        self.current = then_block;
        for stmt in then_branch {
            self.lower_stmt(stmt);
        }
        if matches!(self.current_block().terminator, Terminator::Unreachable) {
            self.current_block_mut().terminator = Terminator::Jump(merge_block);
        }

        self.current = else_block;
        for stmt in else_branch {
            self.lower_stmt(stmt);
        }
        if matches!(self.current_block().terminator, Terminator::Unreachable) {
            self.current_block_mut().terminator = Terminator::Jump(merge_block);
        }

        self.current = merge_block;
    }

    fn lower_while(&mut self, condition: &Expr, body: &[Stmt]) {
        let cond_block = self.new_block();
        let body_block = self.new_block();
        let exit_block = self.new_block();
        self.current_block_mut().terminator = Terminator::Jump(cond_block);

        self.current = cond_block;
        let cond = self.lower_expr(condition);
        self.current_block_mut().terminator = Terminator::Branch {
            cond,
            then_block: body_block,
            else_block: exit_block,
        };

        self.current = body_block;
        for stmt in body {
            self.lower_stmt(stmt);
        }
        if matches!(self.current_block().terminator, Terminator::Unreachable) {
            self.current_block_mut().terminator = Terminator::Jump(cond_block);
        }
        self.current = exit_block;
    }

    fn lower_match(&mut self, expr: &Expr, arms: &[MatchArm]) {
        let subject = self.lower_expr(expr);
        let exit = self.new_block();
        let mut next_check = self.current;
        for (index, arm) in arms.iter().enumerate() {
            self.current = next_check;
            let body_block = self.new_block();
            let fallback = if index + 1 == arms.len() {
                exit
            } else {
                self.new_block()
            };
            match &arm.pattern {
                Pattern::Wildcard(_) | Pattern::Ident(_, _) => {
                    self.current_block_mut().terminator = Terminator::Jump(body_block);
                }
                _ => {
                    let pattern_value = self.pattern_to_value(&arm.pattern);
                    let compare = self.next_value();
                    self.emit(Instruction::Binary {
                        dst: compare,
                        op: BinaryOp::Eq,
                        lhs: subject,
                        rhs: pattern_value,
                    });
                    self.current_block_mut().terminator = Terminator::Branch {
                        cond: compare,
                        then_block: body_block,
                        else_block: fallback,
                    };
                }
            }
            self.current = body_block;
            for stmt in &arm.body {
                self.lower_stmt(stmt);
            }
            if matches!(self.current_block().terminator, Terminator::Unreachable) {
                self.current_block_mut().terminator = Terminator::Jump(exit);
            }
            next_check = fallback;
        }
        self.current = exit;
    }

    fn lower_expr(&mut self, expr: &Expr) -> ValueId {
        match expr {
            Expr::Int(value, _) => self.emit_const(Constant::Int(*value)),
            Expr::Float(value, _) => self.emit_const(Constant::Float(*value)),
            Expr::Bool(value, _) => self.emit_const(Constant::Bool(*value)),
            Expr::String(value, _) => self.emit_const(Constant::String(value.clone())),
            Expr::Ident(name, span) => {
                if let Some(slot) = self.locals.get(name).copied() {
                    let dst = self.next_value();
                    self.emit(Instruction::LoadLocal { dst, slot });
                    dst
                } else if self.semantic.functions.contains_key(name) {
                    let dst = self.next_value();
                    self.emit(Instruction::FunctionRef {
                        dst,
                        name: name.clone(),
                    });
                    dst
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        format!("unknown identifier '{}'", name),
                        Some(*span),
                    ));
                    self.emit_const(Constant::Int(0))
                }
            }
            Expr::Array(items, _) => {
                let values = items.iter().map(|item| self.lower_expr(item)).collect();
                let dst = self.next_value();
                self.emit(Instruction::MakeArray { dst, items: values });
                dst
            }
            Expr::Unary { op, expr, .. } => match op {
                UnaryOp::Await => {
                    let task = self.lower_expr(expr);
                    let dst = self.next_value();
                    self.emit(Instruction::Await { dst, task });
                    dst
                }
                UnaryOp::Spawn | UnaryOp::Parallel => {
                    if let Expr::Call { callee, args, .. } = &**expr {
                        let callee = self.lower_expr(callee);
                        let args = args.iter().map(|arg| self.lower_expr(arg)).collect();
                        let dst = self.next_value();
                        self.emit(Instruction::Spawn { dst, callee, args });
                        dst
                    } else {
                        self.lower_expr(expr)
                    }
                }
                UnaryOp::Move => {
                    let input = self.lower_expr(expr);
                    let dst = self.next_value();
                    self.emit(Instruction::Unary {
                        dst,
                        op: IrUnaryOp::Move,
                        value: input,
                    });
                    dst
                }
                UnaryOp::Neg => {
                    let input = self.lower_expr(expr);
                    let dst = self.next_value();
                    self.emit(Instruction::Unary {
                        dst,
                        op: IrUnaryOp::Neg,
                        value: input,
                    });
                    dst
                }
                UnaryOp::Not => self.lower_expr(expr),
            },
            Expr::Binary {
                op, left, right, ..
            } => {
                let lhs = self.lower_expr(left);
                let rhs = self.lower_expr(right);
                let dst = self.next_value();
                self.emit(Instruction::Binary {
                    dst,
                    op: *op,
                    lhs,
                    rhs,
                });
                dst
            }
            Expr::Call { callee, args, .. } => {
                let callee = self.lower_expr(callee);
                let args = args.iter().map(|arg| self.lower_expr(arg)).collect();
                let dst = self.next_value();
                self.emit(Instruction::Call { dst, callee, args });
                dst
            }
            Expr::Member { object, field, .. } => {
                if let Expr::Ident(root, _) = &**object {
                    let dst = self.next_value();
                    self.emit(Instruction::FunctionRef {
                        dst,
                        name: format!("{root}.{field}"),
                    });
                    dst
                } else {
                    self.lower_expr(object)
                }
            }
            Expr::Index { object, index, .. } => {
                let object = self.lower_expr(object);
                let index = self.lower_expr(index);
                let dst = self.next_value();
                self.emit(Instruction::Index { dst, object, index });
                dst
            }
        }
    }

    fn pattern_to_value(&mut self, pattern: &Pattern) -> ValueId {
        match pattern {
            Pattern::Int(value, _) => self.emit_const(Constant::Int(*value)),
            Pattern::Float(value, _) => self.emit_const(Constant::Float(*value)),
            Pattern::Bool(value, _) => self.emit_const(Constant::Bool(*value)),
            Pattern::String(value, _) => self.emit_const(Constant::String(value.clone())),
            Pattern::Wildcard(_) | Pattern::Ident(_, _) => self.emit_const(Constant::Bool(true)),
        }
    }

    fn emit_const(&mut self, value: Constant) -> ValueId {
        let dst = self.next_value();
        self.emit(Instruction::Const { dst, value });
        dst
    }

    fn next_value(&mut self) -> ValueId {
        let value = ValueId(self.values);
        self.values += 1;
        value
    }

    fn emit(&mut self, instruction: Instruction) {
        self.current_block_mut().instructions.push(instruction);
    }

    fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len());
        self.blocks.push(BasicBlock {
            id,
            instructions: Vec::new(),
            terminator: Terminator::Unreachable,
        });
        id
    }

    fn current_block(&self) -> &BasicBlock {
        &self.blocks[self.current.0]
    }

    fn current_block_mut(&mut self) -> &mut BasicBlock {
        &mut self.blocks[self.current.0]
    }
}

fn constant_fold(function: &mut FunctionIR) {
    for block in &mut function.blocks {
        let mut constants = HashMap::new();
        for index in 0..block.instructions.len() {
            let instruction = block.instructions[index].clone();
            match instruction {
                Instruction::Const { dst, value } => {
                    constants.insert(dst, value);
                }
                Instruction::Unary {
                    dst,
                    op: IrUnaryOp::Neg,
                    value,
                } => {
                    if let Some(constant) = constants.get(&value).cloned() {
                        let folded = match constant {
                            Constant::Int(v) => Some(Constant::Int(-v)),
                            Constant::Float(v) => Some(Constant::Float(-v)),
                            _ => None,
                        };
                        if let Some(value) = folded {
                            block.instructions[index] = Instruction::Const {
                                dst,
                                value: value.clone(),
                            };
                            constants.insert(dst, value);
                        }
                    }
                }
                Instruction::Binary { dst, op, lhs, rhs } => {
                    let Some(left) = constants.get(&lhs).cloned() else {
                        continue;
                    };
                    let Some(right) = constants.get(&rhs).cloned() else {
                        continue;
                    };
                    if let Some(value) = fold_binary(op, left, right) {
                        block.instructions[index] = Instruction::Const {
                            dst,
                            value: value.clone(),
                        };
                        constants.insert(dst, value);
                    }
                }
                _ => {}
            }
        }
    }
}

fn fold_binary(op: BinaryOp, left: Constant, right: Constant) -> Option<Constant> {
    match (op, left, right) {
        (BinaryOp::Add, Constant::Int(a), Constant::Int(b)) => Some(Constant::Int(a + b)),
        (BinaryOp::Sub, Constant::Int(a), Constant::Int(b)) => Some(Constant::Int(a - b)),
        (BinaryOp::Mul, Constant::Int(a), Constant::Int(b)) => Some(Constant::Int(a * b)),
        (BinaryOp::Div, Constant::Int(a), Constant::Int(b)) if b != 0 => Some(Constant::Int(a / b)),
        (BinaryOp::Eq, Constant::Int(a), Constant::Int(b)) => Some(Constant::Bool(a == b)),
        (BinaryOp::Eq, Constant::Bool(a), Constant::Bool(b)) => Some(Constant::Bool(a == b)),
        (BinaryOp::Add, Constant::String(a), Constant::String(b)) => {
            Some(Constant::String(format!("{a}{b}")))
        }
        _ => None,
    }
}

fn dead_code_eliminate(function: &mut FunctionIR) {
    let mut used = HashSet::new();
    for block in &function.blocks {
        match &block.terminator {
            Terminator::Branch { cond, .. } => {
                used.insert(*cond);
            }
            Terminator::Return(Some(value)) => {
                used.insert(*value);
            }
            _ => {}
        }
        for instruction in &block.instructions {
            match instruction {
                Instruction::StoreLocal { value, .. } => {
                    used.insert(*value);
                }
                Instruction::Unary { value, .. } => {
                    used.insert(*value);
                }
                Instruction::Binary { lhs, rhs, .. } => {
                    used.insert(*lhs);
                    used.insert(*rhs);
                }
                Instruction::Call { callee, args, .. }
                | Instruction::Spawn { callee, args, .. } => {
                    used.insert(*callee);
                    used.extend(args.iter().copied());
                }
                Instruction::Await { task, .. } => {
                    used.insert(*task);
                }
                Instruction::MakeArray { items, .. } => {
                    used.extend(items.iter().copied());
                }
                Instruction::Index { object, index, .. } => {
                    used.insert(*object);
                    used.insert(*index);
                }
                _ => {}
            }
        }
    }

    for block in &mut function.blocks {
        block.instructions.retain(|instruction| match instruction {
            Instruction::Const { dst, .. }
            | Instruction::FunctionRef { dst, .. }
            | Instruction::LoadLocal { dst, .. }
            | Instruction::Unary { dst, .. }
            | Instruction::Binary { dst, .. }
            | Instruction::MakeArray { dst, .. }
            | Instruction::Index { dst, .. } => used.contains(dst),
            Instruction::StoreLocal { .. }
            | Instruction::Call { .. }
            | Instruction::Spawn { .. }
            | Instruction::Await { .. } => true,
        });
    }
}
