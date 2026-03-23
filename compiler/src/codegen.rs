pub mod native {
    use crate::ir::{Constant, FunctionIR, Instruction, Program, Terminator, ValueId};
    use crate::semantic::SemanticModel;
    use std::fmt::Write;

    #[derive(Debug, Clone, Copy)]
    pub enum Arch {
        X86_64,
        Aarch64,
    }

    pub fn emit_asm(program: &Program, _semantic: &SemanticModel, arch: Arch) -> String {
        let mut out = String::new();
        match arch {
            Arch::X86_64 => out.push_str("section .text\n"),
            Arch::Aarch64 => out.push_str(".text\n"),
        }
        for function in &program.functions {
            emit_function(function, arch, &mut out);
        }
        out
    }

    fn emit_function(function: &FunctionIR, arch: Arch, out: &mut String) {
        match arch {
            Arch::X86_64 => {
                let _ = writeln!(out, "global {}", function.name);
                let _ = writeln!(out, "{}:", function.name);
                let _ = writeln!(out, "    push rbp");
                let _ = writeln!(out, "    mov rbp, rsp");
            }
            Arch::Aarch64 => {
                let _ = writeln!(out, ".global {}", function.name);
                let _ = writeln!(out, "{}:", function.name);
            }
        }
        if let Some(value) = constant_return(function) {
            match (arch, value) {
                (Arch::X86_64, Constant::Int(v)) => {
                    let _ = writeln!(out, "    mov rax, {}", v);
                }
                (Arch::Aarch64, Constant::Int(v)) => {
                    let _ = writeln!(out, "    mov x0, #{}", v);
                }
                _ => {}
            }
        } else {
            let _ = writeln!(
                out,
                "    ; non-constant bodies currently execute through SNSX-VM/runtime bridge"
            );
        }
        match arch {
            Arch::X86_64 => {
                let _ = writeln!(out, "    pop rbp");
                let _ = writeln!(out, "    ret");
            }
            Arch::Aarch64 => {
                let _ = writeln!(out, "    ret");
            }
        }
        let _ = writeln!(out);
    }

    fn constant_return(function: &FunctionIR) -> Option<Constant> {
        for block in &function.blocks {
            let mut consts = std::collections::HashMap::<ValueId, Constant>::new();
            for instruction in &block.instructions {
                if let Instruction::Const { dst, value } = instruction {
                    consts.insert(*dst, value.clone());
                }
            }
            if let Terminator::Return(Some(value)) = &block.terminator {
                if let Some(constant) = consts.get(value).cloned() {
                    return Some(constant);
                }
            }
        }
        None
    }
}

pub mod llvm {
    use crate::ast::BinaryOp;
    use crate::ir::{Constant, FunctionIR, Instruction, Program, Terminator};
    use crate::semantic::SemanticModel;
    use std::fmt::Write;

    pub fn emit_llvm(program: &Program, _semantic: &SemanticModel) -> String {
        let mut out = String::new();
        out.push_str("; SNSX bootstrap LLVM IR\n");
        out.push_str("declare void @snsx_print_i64(i64)\n\n");
        for function in &program.functions {
            emit_function(function, &mut out);
        }
        out
    }

    fn emit_function(function: &FunctionIR, out: &mut String) {
        let _ = writeln!(out, "define i64 @{}() {{", function.name);
        for block in &function.blocks {
            let _ = writeln!(out, "block_{}:", block.id.0);
            for instruction in &block.instructions {
                match instruction {
                    Instruction::Const {
                        dst,
                        value: Constant::Int(v),
                    } => {
                        let _ = writeln!(out, "  %v{} = add i64 0, {}", dst.0, v);
                    }
                    Instruction::Binary { dst, op, lhs, rhs } => {
                        let op_name = match op {
                            BinaryOp::Add => "add",
                            BinaryOp::Sub => "sub",
                            BinaryOp::Mul => "mul",
                            BinaryOp::Div => "sdiv",
                            BinaryOp::Eq => "icmp eq",
                            BinaryOp::Ne => "icmp ne",
                            BinaryOp::Lt => "icmp slt",
                            BinaryOp::Le => "icmp sle",
                            BinaryOp::Gt => "icmp sgt",
                            BinaryOp::Ge => "icmp sge",
                            BinaryOp::Mod => "srem",
                        };
                        let _ = writeln!(
                            out,
                            "  %v{} = {} i64 %v{}, %v{}",
                            dst.0, op_name, lhs.0, rhs.0
                        );
                    }
                    _ => {
                        let _ = writeln!(out, "  ; {:?}", instruction);
                    }
                }
            }
            match &block.terminator {
                Terminator::Return(Some(value)) => {
                    let _ = writeln!(out, "  ret i64 %v{}", value.0);
                }
                Terminator::Return(None) => {
                    let _ = writeln!(out, "  ret i64 0");
                }
                Terminator::Jump(target) => {
                    let _ = writeln!(out, "  br label %block_{}", target.0);
                }
                Terminator::Branch {
                    cond,
                    then_block,
                    else_block,
                } => {
                    let _ = writeln!(
                        out,
                        "  br i1 %v{}, label %block_{}, label %block_{}",
                        cond.0, then_block.0, else_block.0
                    );
                }
                Terminator::Unreachable => {
                    let _ = writeln!(out, "  unreachable");
                }
            }
        }
        let _ = writeln!(out, "}}\n");
    }
}

pub mod wasm {
    use crate::ir::{Constant, FunctionIR, Instruction, Program, Terminator, ValueId};
    use crate::semantic::SemanticModel;
    use std::fmt::Write;

    pub fn emit_wat(program: &Program, _semantic: &SemanticModel) -> String {
        let mut out = String::new();
        out.push_str("(module\n");
        for function in &program.functions {
            emit_function(function, &mut out);
        }
        out.push_str(")\n");
        out
    }

    fn emit_function(function: &FunctionIR, out: &mut String) {
        let _ = writeln!(out, "  (func ${} (result i64)", function.name);
        if let Some(Constant::Int(value)) = constant_return(function) {
            let _ = writeln!(out, "    i64.const {}", value);
        } else {
            let _ = writeln!(out, "    i64.const 0");
            let _ = writeln!(
                out,
                "    ;; non-constant SNSX bodies lower to VM/runtime bridges in bootstrap mode"
            );
        }
        let _ = writeln!(out, "  )");
        if function.name == "main" {
            let _ = writeln!(out, "  (export \"_start\" (func $main))");
        }
    }

    fn constant_return(function: &FunctionIR) -> Option<Constant> {
        for block in &function.blocks {
            let mut consts = std::collections::HashMap::<ValueId, Constant>::new();
            for instruction in &block.instructions {
                if let Instruction::Const { dst, value } = instruction {
                    consts.insert(*dst, value.clone());
                }
            }
            if let Terminator::Return(Some(value)) = &block.terminator {
                if let Some(constant) = consts.get(value).cloned() {
                    return Some(constant);
                }
            }
        }
        None
    }
}

pub mod vm {
    use crate::ast::BinaryOp;
    use crate::ir::{
        Constant, FunctionIR, Instruction as IrInstruction, Program, Terminator, ValueId,
    };
    use crate::semantic::SemanticModel;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BytecodeModule {
        pub functions: Vec<BytecodeFunction>,
        pub entry: usize,
        pub symbols: Vec<String>,
        pub user_functions: HashMap<String, usize>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BytecodeFunction {
        pub name: String,
        pub params: usize,
        pub locals: usize,
        pub code: Vec<Instruction>,
        pub constants: Vec<Constant>,
        pub is_ai: bool,
        pub prompt: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Instruction {
        PushConst(usize),
        LoadLocal(usize),
        StoreLocal(usize),
        LoadFunction(usize),
        Add,
        Sub,
        Mul,
        Div,
        Mod,
        Eq,
        Ne,
        Lt,
        Le,
        Gt,
        Ge,
        Neg,
        MakeArray(usize),
        Index,
        Call(usize),
        Spawn(usize),
        Await,
        Pop,
        Jump(usize),
        JumpIfFalse(usize),
        Return,
    }

    pub fn emit_bytecode(program: &Program, semantic: &SemanticModel) -> BytecodeModule {
        let mut symbols = semantic.functions.keys().cloned().collect::<Vec<_>>();
        symbols.sort();
        let function_indices = symbols
            .iter()
            .enumerate()
            .map(|(idx, name)| (name.clone(), idx))
            .collect::<HashMap<_, _>>();
        let functions = program
            .functions
            .iter()
            .map(|function| emit_function(function, &function_indices))
            .collect::<Vec<_>>();
        let user_functions = functions
            .iter()
            .enumerate()
            .map(|(idx, function)| (function.name.clone(), idx))
            .collect::<HashMap<_, _>>();
        let entry = function_indices.get("main").copied().unwrap_or(0);
        BytecodeModule {
            functions,
            entry,
            symbols,
            user_functions,
        }
    }

    fn emit_function(
        function: &FunctionIR,
        function_indices: &HashMap<String, usize>,
    ) -> BytecodeFunction {
        let mut emitter = FunctionEmitter::new(function, function_indices);
        emitter.emit();
        BytecodeFunction {
            name: function.name.clone(),
            params: function.params.len(),
            locals: function.locals.len() + function.values,
            code: emitter.code,
            constants: emitter.constants,
            is_ai: function.is_ai,
            prompt: function.prompt.clone(),
        }
    }

    struct FunctionEmitter<'a> {
        function: &'a FunctionIR,
        function_indices: &'a HashMap<String, usize>,
        code: Vec<Instruction>,
        constants: Vec<Constant>,
        value_slots: HashMap<ValueId, usize>,
        block_offsets: HashMap<usize, usize>,
        patch_jumps: Vec<(usize, usize)>,
    }

    impl<'a> FunctionEmitter<'a> {
        fn new(function: &'a FunctionIR, function_indices: &'a HashMap<String, usize>) -> Self {
            Self {
                function,
                function_indices,
                code: Vec::new(),
                constants: Vec::new(),
                value_slots: HashMap::new(),
                block_offsets: HashMap::new(),
                patch_jumps: Vec::new(),
            }
        }

        fn emit(&mut self) {
            for block in &self.function.blocks {
                self.block_offsets.insert(block.id.0, self.code.len());
                for instruction in &block.instructions {
                    self.emit_instruction(instruction);
                }
                self.emit_terminator(&block.terminator);
            }
            for (at, block) in &self.patch_jumps {
                if let Some(offset) = self.block_offsets.get(block) {
                    match &mut self.code[*at] {
                        Instruction::Jump(target) | Instruction::JumpIfFalse(target) => {
                            *target = *offset;
                        }
                        _ => {}
                    }
                }
            }
        }

        fn emit_instruction(&mut self, instruction: &IrInstruction) {
            match instruction {
                IrInstruction::Const { dst, value } => {
                    let idx = self.intern_const(value.clone());
                    self.code.push(Instruction::PushConst(idx));
                    self.emit_store_value(*dst);
                }
                IrInstruction::FunctionRef { dst, name } => {
                    let idx = self.function_indices.get(name).copied().unwrap_or(0);
                    self.code.push(Instruction::LoadFunction(idx));
                    self.emit_store_value(*dst);
                }
                IrInstruction::LoadLocal { dst, slot } => {
                    self.code.push(Instruction::LoadLocal(*slot));
                    self.emit_store_value(*dst);
                }
                IrInstruction::StoreLocal { slot, value } => {
                    self.emit_load_value(*value);
                    self.code.push(Instruction::StoreLocal(*slot));
                }
                IrInstruction::Unary { dst, op, value } => {
                    self.emit_load_value(*value);
                    match op {
                        crate::ir::IrUnaryOp::Neg => self.code.push(Instruction::Neg),
                        crate::ir::IrUnaryOp::Move => {}
                    }
                    self.emit_store_value(*dst);
                }
                IrInstruction::Binary { dst, op, lhs, rhs } => {
                    self.emit_load_value(*lhs);
                    self.emit_load_value(*rhs);
                    self.code.push(match op {
                        BinaryOp::Add => Instruction::Add,
                        BinaryOp::Sub => Instruction::Sub,
                        BinaryOp::Mul => Instruction::Mul,
                        BinaryOp::Div => Instruction::Div,
                        BinaryOp::Mod => Instruction::Mod,
                        BinaryOp::Eq => Instruction::Eq,
                        BinaryOp::Ne => Instruction::Ne,
                        BinaryOp::Lt => Instruction::Lt,
                        BinaryOp::Le => Instruction::Le,
                        BinaryOp::Gt => Instruction::Gt,
                        BinaryOp::Ge => Instruction::Ge,
                    });
                    self.emit_store_value(*dst);
                }
                IrInstruction::Call { dst, callee, args } => {
                    self.emit_load_value(*callee);
                    for arg in args {
                        self.emit_load_value(*arg);
                    }
                    self.code.push(Instruction::Call(args.len()));
                    self.emit_store_value(*dst);
                }
                IrInstruction::Spawn { dst, callee, args } => {
                    self.emit_load_value(*callee);
                    for arg in args {
                        self.emit_load_value(*arg);
                    }
                    self.code.push(Instruction::Spawn(args.len()));
                    self.emit_store_value(*dst);
                }
                IrInstruction::Await { dst, task } => {
                    self.emit_load_value(*task);
                    self.code.push(Instruction::Await);
                    self.emit_store_value(*dst);
                }
                IrInstruction::MakeArray { dst, items } => {
                    for item in items {
                        self.emit_load_value(*item);
                    }
                    self.code.push(Instruction::MakeArray(items.len()));
                    self.emit_store_value(*dst);
                }
                IrInstruction::Index { dst, object, index } => {
                    self.emit_load_value(*object);
                    self.emit_load_value(*index);
                    self.code.push(Instruction::Index);
                    self.emit_store_value(*dst);
                }
            }
        }

        fn emit_terminator(&mut self, terminator: &Terminator) {
            match terminator {
                Terminator::Jump(block) => {
                    let at = self.code.len();
                    self.code.push(Instruction::Jump(usize::MAX));
                    self.patch_jumps.push((at, block.0));
                }
                Terminator::Branch {
                    cond,
                    then_block,
                    else_block,
                } => {
                    self.emit_load_value(*cond);
                    let jump_if_false = self.code.len();
                    self.code.push(Instruction::JumpIfFalse(usize::MAX));
                    self.patch_jumps.push((jump_if_false, else_block.0));
                    let jump_then = self.code.len();
                    self.code.push(Instruction::Jump(usize::MAX));
                    self.patch_jumps.push((jump_then, then_block.0));
                }
                Terminator::Return(Some(value)) => {
                    self.emit_load_value(*value);
                    self.code.push(Instruction::Return);
                }
                Terminator::Return(None) => {
                    let idx = self.intern_const(Constant::Int(0));
                    self.code.push(Instruction::PushConst(idx));
                    self.code.push(Instruction::Return);
                }
                Terminator::Unreachable => {}
            }
        }

        fn intern_const(&mut self, value: Constant) -> usize {
            if let Some(idx) = self
                .constants
                .iter()
                .position(|existing| *existing == value)
            {
                idx
            } else {
                let idx = self.constants.len();
                self.constants.push(value);
                idx
            }
        }

        fn value_slot(&mut self, value: ValueId) -> usize {
            let base = self.function.locals.len();
            if let Some(slot) = self.value_slots.get(&value).copied() {
                slot
            } else {
                let slot = base + self.value_slots.len();
                self.value_slots.insert(value, slot);
                slot
            }
        }

        fn emit_load_value(&mut self, value: ValueId) {
            let slot = self.value_slot(value);
            self.code.push(Instruction::LoadLocal(slot));
        }

        fn emit_store_value(&mut self, value: ValueId) {
            let slot = self.value_slot(value);
            self.code.push(Instruction::StoreLocal(slot));
        }
    }
}
