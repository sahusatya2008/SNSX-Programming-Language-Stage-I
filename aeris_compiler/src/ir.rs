use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ast::{BinaryOp, Block, Expr, FunctionDecl, Program, Stmt, UnaryOp};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IrFunction {
    pub name: String,
    pub effect: String,
    pub params: Vec<String>,
    pub blocks: Vec<IrBlock>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IrBlock {
    pub label: String,
    pub instructions: Vec<IrInstruction>,
    pub terminator: IrTerminator,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum IrInstruction {
    Param {
        dst: String,
        name: String,
    },
    ConstInt {
        dst: String,
        value: i64,
    },
    ConstBool {
        dst: String,
        value: bool,
    },
    ConstText {
        dst: String,
        value: String,
    },
    Binary {
        dst: String,
        op: String,
        left: String,
        right: String,
    },
    Unary {
        dst: String,
        op: String,
        value: String,
    },
    Call {
        dst: Option<String>,
        callee: String,
        args: Vec<String>,
    },
    Phi {
        dst: String,
        incoming: Vec<(String, String)>,
    },
    Guard {
        cond: String,
        message: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum IrTerminator {
    Return(Option<String>),
    Jump(String),
    Branch {
        cond: String,
        then_label: String,
        else_label: String,
    },
    Trap(String),
}

pub fn lower(program: &Program) -> IrProgram {
    let functions = program
        .functions
        .iter()
        .map(lower_function)
        .collect::<Vec<_>>();
    IrProgram { functions }
}

fn lower_function(function: &FunctionDecl) -> IrFunction {
    let mut lowerer = Lowerer::new();
    let entry = lowerer.begin_block("entry");
    for param in &function.params {
        let reg = lowerer.temp();
        lowerer
            .current_block_mut()
            .instructions
            .push(IrInstruction::Param {
                dst: reg.clone(),
                name: param.name.clone(),
            });
        lowerer.env.insert(param.name.clone(), reg);
    }
    lowerer.lower_block(&function.body);
    if !lowerer.current_terminated {
        let ret = function
            .body
            .tail
            .as_ref()
            .map(|expr| lowerer.lower_expr(expr))
            .or_else(|| Some(lowerer.const_unit()));
        lowerer.current_block_mut().terminator = IrTerminator::Return(ret);
    }
    lowerer.blocks[entry].label = "entry".to_string();
    IrFunction {
        name: function.name.clone(),
        effect: function.effect.clone(),
        params: function
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect(),
        blocks: lowerer.blocks,
    }
}

struct Lowerer {
    blocks: Vec<IrBlock>,
    current: usize,
    temp_index: usize,
    block_index: usize,
    env: HashMap<String, String>,
    current_terminated: bool,
}

impl Lowerer {
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            current: 0,
            temp_index: 0,
            block_index: 0,
            env: HashMap::new(),
            current_terminated: false,
        }
    }

    fn begin_block(&mut self, label_prefix: &str) -> usize {
        let label = format!("{label_prefix}_{}", self.block_index);
        self.block_index += 1;
        self.blocks.push(IrBlock {
            label,
            instructions: Vec::new(),
            terminator: IrTerminator::Trap("unterminated block".to_string()),
        });
        let index = self.blocks.len() - 1;
        self.current = index;
        self.current_terminated = false;
        index
    }

    fn temp(&mut self) -> String {
        let name = format!("%{}", self.temp_index);
        self.temp_index += 1;
        name
    }

    fn current_block_mut(&mut self) -> &mut IrBlock {
        &mut self.blocks[self.current]
    }

    fn lower_block(&mut self, block: &Block) {
        for stmt in &block.statements {
            self.lower_stmt(stmt);
            if self.current_terminated {
                return;
            }
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let reg = if let Some(expr) = value {
                    self.lower_expr(expr)
                } else {
                    self.const_unit()
                };
                self.env.insert(name.clone(), reg);
            }
            Stmt::Mut { name, value, .. } => {
                let reg = self.lower_expr(value);
                self.env.insert(name.clone(), reg);
            }
            Stmt::Require {
                condition, message, ..
            } => {
                let cond = self.lower_expr(condition);
                self.current_block_mut()
                    .instructions
                    .push(IrInstruction::Guard {
                        cond,
                        message: message.clone(),
                    });
            }
            Stmt::Return { value, .. } => {
                let reg = self.lower_expr(value);
                self.current_block_mut().terminator = IrTerminator::Return(Some(reg));
                self.current_terminated = true;
            }
            Stmt::Expr { value, .. } => {
                let _ = self.lower_expr(value);
            }
        }
    }

    fn lower_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Int(value) => {
                let dst = self.temp();
                self.current_block_mut()
                    .instructions
                    .push(IrInstruction::ConstInt {
                        dst: dst.clone(),
                        value: *value,
                    });
                dst
            }
            Expr::Bool(value) => {
                let dst = self.temp();
                self.current_block_mut()
                    .instructions
                    .push(IrInstruction::ConstBool {
                        dst: dst.clone(),
                        value: *value,
                    });
                dst
            }
            Expr::Text(value) => {
                let dst = self.temp();
                self.current_block_mut()
                    .instructions
                    .push(IrInstruction::ConstText {
                        dst: dst.clone(),
                        value: value.clone(),
                    });
                dst
            }
            Expr::Variable(name) => self
                .env
                .get(name)
                .cloned()
                .unwrap_or_else(|| format!("@{name}")),
            Expr::Unary { op, value } => {
                let src = self.lower_expr(value);
                let dst = self.temp();
                self.current_block_mut()
                    .instructions
                    .push(IrInstruction::Unary {
                        dst: dst.clone(),
                        op: match op {
                            UnaryOp::Neg => "neg",
                            UnaryOp::Not => "not",
                        }
                        .to_string(),
                        value: src,
                    });
                dst
            }
            Expr::Binary { op, left, right } => {
                let left_reg = self.lower_expr(left);
                let right_reg = self.lower_expr(right);
                let dst = self.temp();
                self.current_block_mut()
                    .instructions
                    .push(IrInstruction::Binary {
                        dst: dst.clone(),
                        op: binary_name(op).to_string(),
                        left: left_reg,
                        right: right_reg,
                    });
                dst
            }
            Expr::Call { callee, args } => {
                let callee_name = match &**callee {
                    Expr::Variable(name) => name.clone(),
                    _ => self.lower_expr(callee),
                };
                let lowered_args = args.iter().map(|arg| self.lower_expr(arg)).collect();
                let dst = self.temp();
                self.current_block_mut()
                    .instructions
                    .push(IrInstruction::Call {
                        dst: Some(dst.clone()),
                        callee: callee_name,
                        args: lowered_args,
                    });
                dst
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.lower_expr(condition);
                let then_label = format!("then_{}", self.block_index);
                let else_label = format!("else_{}", self.block_index);
                let merge_label = format!("merge_{}", self.block_index);
                self.block_index += 1;
                self.current_block_mut().terminator = IrTerminator::Branch {
                    cond,
                    then_label: then_label.clone(),
                    else_label: else_label.clone(),
                };
                self.current_terminated = true;

                let saved_env = self.env.clone();

                let then_index = self.begin_block(&then_label);
                self.env = saved_env.clone();
                self.lower_block(then_branch);
                let then_value = then_branch
                    .tail
                    .as_ref()
                    .map(|expr| self.lower_expr(expr))
                    .unwrap_or_else(|| self.const_unit());
                if !self.current_terminated {
                    self.current_block_mut().terminator = IrTerminator::Jump(merge_label.clone());
                    self.current_terminated = true;
                }
                let _ = then_index;

                let else_index = self.begin_block(&else_label);
                self.env = saved_env.clone();
                self.lower_block(else_branch);
                let else_value = else_branch
                    .tail
                    .as_ref()
                    .map(|expr| self.lower_expr(expr))
                    .unwrap_or_else(|| self.const_unit());
                if !self.current_terminated {
                    self.current_block_mut().terminator = IrTerminator::Jump(merge_label.clone());
                    self.current_terminated = true;
                }
                let _ = else_index;

                let merge_index = self.begin_block(&merge_label);
                self.env = saved_env;
                let dst = self.temp();
                self.current_block_mut()
                    .instructions
                    .push(IrInstruction::Phi {
                        dst: dst.clone(),
                        incoming: vec![(then_label, then_value), (else_label, else_value)],
                    });
                self.blocks[merge_index].terminator =
                    IrTerminator::Trap("merge continues".to_string());
                self.current_terminated = false;
                dst
            }
            Expr::Match { .. } => {
                let dst = self.temp();
                self.current_block_mut()
                    .instructions
                    .push(IrInstruction::ConstText {
                        dst: dst.clone(),
                        value: "<match-lowered-in-runtime>".to_string(),
                    });
                dst
            }
            Expr::Lambda { .. } => {
                let dst = self.temp();
                self.current_block_mut()
                    .instructions
                    .push(IrInstruction::ConstText {
                        dst: dst.clone(),
                        value: "<lambda>".to_string(),
                    });
                dst
            }
        }
    }

    fn const_unit(&mut self) -> String {
        let dst = self.temp();
        self.current_block_mut()
            .instructions
            .push(IrInstruction::ConstText {
                dst: dst.clone(),
                value: "<unit>".to_string(),
            });
        dst
    }
}

fn binary_name(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "add",
        BinaryOp::Sub => "sub",
        BinaryOp::Mul => "mul",
        BinaryOp::Div => "div",
        BinaryOp::Mod => "mod",
        BinaryOp::Eq => "eq",
        BinaryOp::Ne => "ne",
        BinaryOp::Lt => "lt",
        BinaryOp::Le => "le",
        BinaryOp::Gt => "gt",
        BinaryOp::Ge => "ge",
        BinaryOp::And => "and",
        BinaryOp::Or => "or",
    }
}
