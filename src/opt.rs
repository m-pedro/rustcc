use std::collections::{HashMap, HashSet};

use crate::ast::{BinOp, UnaryOp};
use crate::ir::{IrFunction, IrInstr, IrProgram, Operand};

pub fn optimize_program(prog: &IrProgram) -> IrProgram {
    let functions = prog.functions.iter().map(optimize_function).collect();
    IrProgram { functions }
}

pub fn optimize_function(func: &IrFunction) -> IrFunction {
    let mut body = func.body.clone();
    // Run fixed-point constant folding + propagation + dead-temp elimination (up to 4 iterations)
    for _ in 0..4 {
        let folded = fold_and_propagate(&body);
        let cleaned = eliminate_dead_temps(&folded);
        if cleaned == body {
            break;
        }
        body = cleaned;
    }
    IrFunction {
        name: func.name.clone(),
        params: func.params.clone(),
        body,
    }
}

fn fold_and_propagate(instrs: &[IrInstr]) -> Vec<IrInstr> {
    let mut known_temps: HashMap<usize, Operand> = HashMap::new();
    let mut out = Vec::with_capacity(instrs.len());

    let resolve = |op: &Operand, map: &HashMap<usize, Operand>| -> Operand {
        if let Operand::Temp(id) = op {
            if let Some(repl) = map.get(id) {
                return repl.clone();
            }
        }
        op.clone()
    };

    for instr in instrs {
        match instr {
            IrInstr::Label(_) | IrInstr::Jump(_) => {
                // Control-flow boundaries invalidate basic-block temp propagation
                known_temps.clear();
                out.push(instr.clone());
            }
            IrInstr::Copy { dst, src } => {
                let s = resolve(src, &known_temps);
                if let Operand::Temp(id) = dst {
                    known_temps.insert(*id, s.clone());
                }
                out.push(IrInstr::Copy {
                    dst: dst.clone(),
                    src: s,
                });
            }
            IrInstr::Unary { dst, op, src } => {
                let s = resolve(src, &known_temps);
                if let Operand::Imm(v) = s {
                    let folded = match op {
                        UnaryOp::Neg => Operand::Imm(-v),
                        UnaryOp::Not => Operand::Imm(if v == 0 { 1 } else { 0 }),
                        _ => unreachable!(),
                    };
                    if let Operand::Temp(id) = dst {
                        known_temps.insert(*id, folded.clone());
                    }
                    out.push(IrInstr::Copy {
                        dst: dst.clone(),
                        src: folded,
                    });
                } else {
                    out.push(IrInstr::Unary {
                        dst: dst.clone(),
                        op: *op,
                        src: s,
                    });
                }
            }
            IrInstr::Binary { dst, op, lhs, rhs } => {
                let l = resolve(lhs, &known_temps);
                let r = resolve(rhs, &known_temps);

                if let Some(folded) = eval_binary(*op, &l, &r) {
                    if let Operand::Temp(id) = dst {
                        known_temps.insert(*id, folded.clone());
                    }
                    out.push(IrInstr::Copy {
                        dst: dst.clone(),
                        src: folded,
                    });
                } else {
                    out.push(IrInstr::Binary {
                        dst: dst.clone(),
                        op: *op,
                        lhs: l,
                        rhs: r,
                    });
                }
            }
            IrInstr::AddrOf { dst, var } => {
                out.push(IrInstr::AddrOf {
                    dst: dst.clone(),
                    var: var.clone(),
                });
            }
            IrInstr::Load { dst, addr } => {
                out.push(IrInstr::Load {
                    dst: dst.clone(),
                    addr: resolve(addr, &known_temps),
                });
            }
            IrInstr::Store { addr, src } => {
                out.push(IrInstr::Store {
                    addr: resolve(addr, &known_temps),
                    src: resolve(src, &known_temps),
                });
            }
            IrInstr::BranchZero { cond, label } => {
                let c = resolve(cond, &known_temps);
                out.push(IrInstr::BranchZero {
                    cond: c,
                    label: label.clone(),
                });
            }
            IrInstr::Call { dst, callee, args } => {
                let resolved_args = args.iter().map(|a| resolve(a, &known_temps)).collect();
                out.push(IrInstr::Call {
                    dst: dst.clone(),
                    callee: callee.clone(),
                    args: resolved_args,
                });
            }
            IrInstr::Return(op_opt) => {
                let resolved = op_opt.as_ref().map(|o| resolve(o, &known_temps));
                out.push(IrInstr::Return(resolved));
            }
        }
    }

    out
}

fn eval_binary(op: BinOp, lhs: &Operand, rhs: &Operand) -> Option<Operand> {
    if let (Operand::Imm(a), Operand::Imm(b)) = (lhs, rhs) {
        let val = match op {
            BinOp::Add => a.wrapping_add(*b),
            BinOp::Sub => a.wrapping_sub(*b),
            BinOp::Mul => a.wrapping_mul(*b),
            BinOp::Div => {
                if *b == 0 {
                    return None;
                }
                a.wrapping_div(*b)
            }
            BinOp::Mod => {
                if *b == 0 {
                    return None;
                }
                a.wrapping_rem(*b)
            }
            BinOp::Eq => i64::from(a == b),
            BinOp::Ne => i64::from(a != b),
            BinOp::Lt => i64::from(a < b),
            BinOp::Le => i64::from(a <= b),
            BinOp::Gt => i64::from(a > b),
            BinOp::Ge => i64::from(a >= b),
            BinOp::LogicalAnd => i64::from(*a != 0 && *b != 0),
            BinOp::LogicalOr => i64::from(*a != 0 || *b != 0),
        };
        return Some(Operand::Imm(val));
    }

    // Algebraic identities
    match (op, lhs, rhs) {
        (BinOp::Add, x, Operand::Imm(0)) | (BinOp::Add, Operand::Imm(0), x) => Some(x.clone()),
        (BinOp::Sub, x, Operand::Imm(0)) => Some(x.clone()),
        (BinOp::Mul, x, Operand::Imm(1)) | (BinOp::Mul, Operand::Imm(1), x) => Some(x.clone()),
        (BinOp::Mul, _, Operand::Imm(0)) | (BinOp::Mul, Operand::Imm(0), _) => {
            Some(Operand::Imm(0))
        }
        (BinOp::Div, x, Operand::Imm(1)) => Some(x.clone()),
        _ => None,
    }
}

fn eliminate_dead_temps(instrs: &[IrInstr]) -> Vec<IrInstr> {
    let mut used_temps: HashSet<usize> = HashSet::new();
    let mut mark_used = |op: &Operand| {
        if let Operand::Temp(id) = op {
            used_temps.insert(*id);
        }
    };

    for instr in instrs {
        match instr {
            IrInstr::Copy { src, .. } | IrInstr::Unary { src, .. } => mark_used(src),
            IrInstr::Binary { lhs, rhs, .. } => {
                mark_used(lhs);
                mark_used(rhs);
            }
            IrInstr::Load { addr, .. } => mark_used(addr),
            IrInstr::Store { addr, src } => {
                mark_used(addr);
                mark_used(src);
            }
            IrInstr::BranchZero { cond, .. } => mark_used(cond),
            IrInstr::Call { args, .. } => {
                for a in args {
                    mark_used(a);
                }
            }
            IrInstr::Return(Some(op)) => mark_used(op),
            _ => {}
        }
    }

    instrs
        .iter()
        .filter(|instr| match instr {
            // Never eliminate Calls since they may have side effects
            IrInstr::Copy {
                dst: Operand::Temp(id),
                ..
            }
            | IrInstr::Unary {
                dst: Operand::Temp(id),
                ..
            }
            | IrInstr::Binary {
                dst: Operand::Temp(id),
                ..
            }
            | IrInstr::AddrOf {
                dst: Operand::Temp(id),
                ..
            } => used_temps.contains(id),
            _ => true,
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::IrLowerer;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    #[test]
    fn test_constant_folding_and_dead_temp_elimination() {
        let src = r#"
            int main(void) {
                int x = (3 + 4) * 2 + 0;
                return x;
            }
        "#;
        let tokens = Lexer::new(src).tokenize().unwrap();
        let prog = Parser::new(tokens).parse_program().unwrap();
        let ir = IrLowerer::new().lower_program(&prog);
        let opt = optimize_program(&ir);

        let body = &opt.functions[0].body;
        // (3 + 4) * 2 + 0 should fold completely into Copy { dst: Var("x_0"), src: Imm(14) }
        assert!(body.iter().any(|i| matches!(
            i,
            IrInstr::Copy {
                dst: Operand::Var(_),
                src: Operand::Imm(14)
            }
        )));
        assert!(!body.iter().any(|i| matches!(i, IrInstr::Binary { .. })));
    }
}
