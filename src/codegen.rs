use std::collections::HashMap;
use std::fmt::Write;

use crate::ast::{BinOp, UnaryOp};
use crate::ir::{IrFunction, IrInstr, IrProgram, Operand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetArch {
    X86_64,
    Aarch64,
}

impl TargetArch {
    pub fn host() -> Self {
        if cfg!(target_arch = "aarch64") {
            TargetArch::Aarch64
        } else {
            TargetArch::X86_64
        }
    }
}

struct StackFrame {
    slots: HashMap<Operand, i32>,
    frame_size: i32,
}

impl StackFrame {
    fn build(func: &IrFunction) -> Self {
        let mut slots = HashMap::new();
        let mut offset = 0i32;

        let mut assign_slot = |op: &Operand| {
            if matches!(op, Operand::Var(_) | Operand::Temp(_)) && !slots.contains_key(op) {
                offset += 8;
                slots.insert(op.clone(), offset);
            }
        };

        for p in &func.params {
            assign_slot(&Operand::Var(p.clone()));
        }

        for instr in &func.body {
            match instr {
                IrInstr::Copy { dst, src } | IrInstr::Unary { dst, src, .. } => {
                    assign_slot(dst);
                    assign_slot(src);
                }
                IrInstr::Binary { dst, lhs, rhs, .. } => {
                    assign_slot(dst);
                    assign_slot(lhs);
                    assign_slot(rhs);
                }
                IrInstr::AddrOf { dst, var } => {
                    assign_slot(dst);
                    assign_slot(&Operand::Var(var.clone()));
                }
                IrInstr::Load { dst, addr } => {
                    assign_slot(dst);
                    assign_slot(addr);
                }
                IrInstr::Store { addr, src } => {
                    assign_slot(addr);
                    assign_slot(src);
                }
                IrInstr::BranchZero { cond, .. } => assign_slot(cond),
                IrInstr::Call { dst, args, .. } => {
                    assign_slot(dst);
                    for a in args {
                        assign_slot(a);
                    }
                }
                IrInstr::Return(Some(op)) => assign_slot(op),
                _ => {}
            }
        }

        // Round frame size up to a multiple of 16 bytes for ABI stack alignment
        let frame_size = ((offset + 15) / 16) * 16;
        Self { slots, frame_size }
    }

    fn slot(&self, op: &Operand) -> i32 {
        *self
            .slots
            .get(op)
            .unwrap_or_else(|| panic!("missing stack slot for {:?}", op))
    }
}

pub fn emit_assembly(prog: &IrProgram, arch: TargetArch) -> String {
    match arch {
        TargetArch::X86_64 => emit_x86_64(prog),
        TargetArch::Aarch64 => emit_aarch64(prog),
    }
}

fn sym_name(name: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("_{}", name)
    } else {
        name.to_string()
    }
}

pub fn emit_x86_64(prog: &IrProgram) -> String {
    let mut out = String::new();
    writeln!(out, "    .text").unwrap();
    for func in &prog.functions {
        emit_x86_64_function(&mut out, func);
    }
    out
}

fn emit_x86_64_function(out: &mut String, func: &IrFunction) {
    let frame = StackFrame::build(func);
    let sym = sym_name(&func.name);
    let ret_label = format!(".Lret_{}", func.name);

    writeln!(out, "    .globl {}", sym).unwrap();
    writeln!(out, "{}:", sym).unwrap();
    writeln!(out, "    pushq %rbp").unwrap();
    writeln!(out, "    movq %rsp, %rbp").unwrap();
    if frame.frame_size > 0 {
        writeln!(out, "    subq ${}, %rsp", frame.frame_size).unwrap();
    }

    let arg_regs = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];
    for (i, param) in func.params.iter().enumerate().take(6) {
        let slot = frame.slot(&Operand::Var(param.clone()));
        writeln!(out, "    movq {}, -{}(%rbp)", arg_regs[i], slot).unwrap();
    }

    let load_x86 = |out: &mut String, op: &Operand, reg: &str| match op {
        Operand::Imm(v) => writeln!(out, "    movq ${}, {}", v, reg).unwrap(),
        _ => writeln!(out, "    movq -{}(%rbp), {}", frame.slot(op), reg).unwrap(),
    };

    let store_x86 = |out: &mut String, reg: &str, dst: &Operand| {
        writeln!(out, "    movq {}, -{}(%rbp)", reg, frame.slot(dst)).unwrap();
    };

    for instr in &func.body {
        match instr {
            IrInstr::Copy { dst, src } => {
                load_x86(out, src, "%rax");
                store_x86(out, "%rax", dst);
            }
            IrInstr::Unary { dst, op, src } => {
                load_x86(out, src, "%rax");
                match op {
                    UnaryOp::Neg => writeln!(out, "    negq %rax").unwrap(),
                    UnaryOp::Not => {
                        writeln!(out, "    cmpq $0, %rax").unwrap();
                        writeln!(out, "    sete %al").unwrap();
                        writeln!(out, "    movzbq %al, %rax").unwrap();
                    }
                    _ => unreachable!(),
                }
                store_x86(out, "%rax", dst);
            }
            IrInstr::Binary { dst, op, lhs, rhs } => {
                load_x86(out, lhs, "%rax");
                load_x86(out, rhs, "%rcx");
                match op {
                    BinOp::Add => writeln!(out, "    addq %rcx, %rax").unwrap(),
                    BinOp::Sub => writeln!(out, "    subq %rcx, %rax").unwrap(),
                    BinOp::Mul => writeln!(out, "    imulq %rcx, %rax").unwrap(),
                    BinOp::Div => {
                        writeln!(out, "    cqto").unwrap();
                        writeln!(out, "    idivq %rcx").unwrap();
                    }
                    BinOp::Mod => {
                        writeln!(out, "    cqto").unwrap();
                        writeln!(out, "    idivq %rcx").unwrap();
                        writeln!(out, "    movq %rdx, %rax").unwrap();
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        let set_instr = match op {
                            BinOp::Eq => "sete",
                            BinOp::Ne => "setne",
                            BinOp::Lt => "setl",
                            BinOp::Le => "setle",
                            BinOp::Gt => "setg",
                            BinOp::Ge => "setge",
                            _ => unreachable!(),
                        };
                        writeln!(out, "    cmpq %rcx, %rax").unwrap();
                        writeln!(out, "    {} %al", set_instr).unwrap();
                        writeln!(out, "    movzbq %al, %rax").unwrap();
                    }
                    _ => unreachable!(),
                }
                store_x86(out, "%rax", dst);
            }
            IrInstr::AddrOf { dst, var } => {
                let slot = frame.slot(&Operand::Var(var.clone()));
                writeln!(out, "    leaq -{}(%rbp), %rax", slot).unwrap();
                store_x86(out, "%rax", dst);
            }
            IrInstr::Load { dst, addr } => {
                load_x86(out, addr, "%rax");
                writeln!(out, "    movq (%rax), %rax").unwrap();
                store_x86(out, "%rax", dst);
            }
            IrInstr::Store { addr, src } => {
                load_x86(out, addr, "%rcx");
                load_x86(out, src, "%rax");
                writeln!(out, "    movq %rax, (%rcx)").unwrap();
            }
            IrInstr::Label(l) => writeln!(out, ".{}:", l).unwrap(),
            IrInstr::Jump(l) => writeln!(out, "    jmp .{}", l).unwrap(),
            IrInstr::BranchZero { cond, label } => {
                load_x86(out, cond, "%rax");
                writeln!(out, "    cmpq $0, %rax").unwrap();
                writeln!(out, "    je .{}", label).unwrap();
            }
            IrInstr::Call { dst, callee, args } => {
                for (i, a) in args.iter().enumerate().take(6) {
                    load_x86(out, a, arg_regs[i]);
                }
                writeln!(out, "    callq {}", sym_name(callee)).unwrap();
                store_x86(out, "%rax", dst);
            }
            IrInstr::Return(op_opt) => {
                if let Some(op) = op_opt {
                    load_x86(out, op, "%rax");
                } else {
                    writeln!(out, "    movq $0, %rax").unwrap();
                }
                writeln!(out, "    jmp {}", ret_label).unwrap();
            }
        }
    }

    writeln!(out, "{}:", ret_label).unwrap();
    writeln!(out, "    movq %rbp, %rsp").unwrap();
    writeln!(out, "    popq %rbp").unwrap();
    writeln!(out, "    retq").unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::IrLowerer;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    #[test]
    fn test_x86_64_codegen_emits_stack_frame_and_instructions() {
        let src = "int add(int a, int b) { return a + b; }";
        let tokens = Lexer::new(src).tokenize().unwrap();
        let prog = Parser::new(tokens).parse_program().unwrap();
        let ir = IrLowerer::new().lower_program(&prog);
        let asm = emit_x86_64(&ir);
        assert!(asm.contains("pushq %rbp"));
        assert!(asm.contains("addq %rcx, %rax"));
        assert!(asm.contains("retq"));
    }
}

pub fn emit_aarch64(prog: &IrProgram) -> String {
    let mut out = String::new();
    writeln!(out, "    .section __TEXT,__text,regular,pure_instructions").unwrap();
    for func in &prog.functions {
        emit_aarch64_function(&mut out, func);
    }
    out
}

fn emit_aarch64_function(out: &mut String, func: &IrFunction) {
    let frame = StackFrame::build(func);
    let sym = sym_name(&func.name);
    let ret_label = format!("Lret_{}", func.name);

    writeln!(out, "    .globl {}", sym).unwrap();
    writeln!(out, "    .p2align 2").unwrap();
    writeln!(out, "{}:", sym).unwrap();
    writeln!(out, "    stp x29, x30, [sp, #-16]!").unwrap();
    writeln!(out, "    mov x29, sp").unwrap();
    if frame.frame_size > 0 {
        writeln!(out, "    sub sp, sp, #{}", frame.frame_size).unwrap();
    }

    for (i, param) in func.params.iter().enumerate().take(8) {
        let slot = frame.slot(&Operand::Var(param.clone()));
        writeln!(out, "    sub x11, x29, #{}", slot).unwrap();
        writeln!(out, "    str x{}, [x11]", i).unwrap();
    }

    let emit_mov_imm = |out: &mut String, reg: &str, val: i64| {
        if (0..=65535).contains(&val) {
            writeln!(out, "    mov {}, #{}", reg, val).unwrap();
        } else if (-65535..0).contains(&val) {
            writeln!(out, "    mov {}, #{}", reg, val).unwrap();
        } else {
            let u = val as u64;
            writeln!(out, "    movz {}, #{}", reg, u & 0xFFFF).unwrap();
            if (u >> 16) & 0xFFFF != 0 {
                writeln!(out, "    movk {}, #{}, lsl #16", reg, (u >> 16) & 0xFFFF).unwrap();
            }
            if (u >> 32) & 0xFFFF != 0 {
                writeln!(out, "    movk {}, #{}, lsl #32", reg, (u >> 32) & 0xFFFF).unwrap();
            }
            if (u >> 48) & 0xFFFF != 0 {
                writeln!(out, "    movk {}, #{}, lsl #48", reg, (u >> 48) & 0xFFFF).unwrap();
            }
        }
    };

    let load_arm = |out: &mut String, op: &Operand, reg: &str| match op {
        Operand::Imm(v) => emit_mov_imm(out, reg, *v),
        _ => {
            let slot = frame.slot(op);
            writeln!(out, "    sub x11, x29, #{}", slot).unwrap();
            writeln!(out, "    ldr {}, [x11]", reg).unwrap();
        }
    };

    let store_arm = |out: &mut String, reg: &str, dst: &Operand| {
        let slot = frame.slot(dst);
        writeln!(out, "    sub x11, x29, #{}", slot).unwrap();
        writeln!(out, "    str {}, [x11]", reg).unwrap();
    };

    for instr in &func.body {
        match instr {
            IrInstr::Copy { dst, src } => {
                load_arm(out, src, "x8");
                store_arm(out, "x8", dst);
            }
            IrInstr::Unary { dst, op, src } => {
                load_arm(out, src, "x8");
                match op {
                    UnaryOp::Neg => writeln!(out, "    neg x8, x8").unwrap(),
                    UnaryOp::Not => {
                        writeln!(out, "    cmp x8, #0").unwrap();
                        writeln!(out, "    cset x8, eq").unwrap();
                    }
                    _ => unreachable!(),
                }
                store_arm(out, "x8", dst);
            }
            IrInstr::Binary { dst, op, lhs, rhs } => {
                load_arm(out, lhs, "x8");
                load_arm(out, rhs, "x9");
                match op {
                    BinOp::Add => writeln!(out, "    add x8, x8, x9").unwrap(),
                    BinOp::Sub => writeln!(out, "    sub x8, x8, x9").unwrap(),
                    BinOp::Mul => writeln!(out, "    mul x8, x8, x9").unwrap(),
                    BinOp::Div => writeln!(out, "    sdiv x8, x8, x9").unwrap(),
                    BinOp::Mod => {
                        writeln!(out, "    sdiv x10, x8, x9").unwrap();
                        writeln!(out, "    msub x8, x10, x9, x8").unwrap();
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        let cond = match op {
                            BinOp::Eq => "eq",
                            BinOp::Ne => "ne",
                            BinOp::Lt => "lt",
                            BinOp::Le => "le",
                            BinOp::Gt => "gt",
                            BinOp::Ge => "ge",
                            _ => unreachable!(),
                        };
                        writeln!(out, "    cmp x8, x9").unwrap();
                        writeln!(out, "    cset x8, {}", cond).unwrap();
                    }
                    _ => unreachable!(),
                }
                store_arm(out, "x8", dst);
            }
            IrInstr::AddrOf { dst, var } => {
                let slot = frame.slot(&Operand::Var(var.clone()));
                writeln!(out, "    sub x8, x29, #{}", slot).unwrap();
                store_arm(out, "x8", dst);
            }
            IrInstr::Load { dst, addr } => {
                load_arm(out, addr, "x8");
                writeln!(out, "    ldr x8, [x8]").unwrap();
                store_arm(out, "x8", dst);
            }
            IrInstr::Store { addr, src } => {
                load_arm(out, addr, "x9");
                load_arm(out, src, "x8");
                writeln!(out, "    str x8, [x9]").unwrap();
            }
            IrInstr::Label(l) => writeln!(out, "{}:", l).unwrap(),
            IrInstr::Jump(l) => writeln!(out, "    b {}", l).unwrap(),
            IrInstr::BranchZero { cond, label } => {
                load_arm(out, cond, "x8");
                writeln!(out, "    cbz x8, {}", label).unwrap();
            }
            IrInstr::Call { dst, callee, args } => {
                for (i, a) in args.iter().enumerate().take(8) {
                    let reg = format!("x{}", i);
                    load_arm(out, a, &reg);
                }
                writeln!(out, "    bl {}", sym_name(callee)).unwrap();
                store_arm(out, "x0", dst);
            }
            IrInstr::Return(op_opt) => {
                if let Some(op) = op_opt {
                    load_arm(out, op, "x0");
                } else {
                    writeln!(out, "    mov x0, #0").unwrap();
                }
                writeln!(out, "    b {}", ret_label).unwrap();
            }
        }
    }

    writeln!(out, "{}:", ret_label).unwrap();
    writeln!(out, "    mov sp, x29").unwrap();
    writeln!(out, "    ldp x29, x30, [sp], #16").unwrap();
    writeln!(out, "    ret").unwrap();
}
