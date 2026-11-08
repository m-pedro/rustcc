use std::collections::HashMap;
use std::fmt;

use crate::ast::{BinOp, Expr, FunctionDecl, Program, Stmt, UnaryOp};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Operand {
    Imm(i64),
    Var(String),
    Temp(usize),
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operand::Imm(v) => write!(f, "{}", v),
            Operand::Var(name) => write!(f, "${}", name),
            Operand::Temp(id) => write!(f, "%t{}", id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrInstr {
    Copy {
        dst: Operand,
        src: Operand,
    },
    Unary {
        dst: Operand,
        op: UnaryOp,
        src: Operand,
    },
    Binary {
        dst: Operand,
        op: BinOp,
        lhs: Operand,
        rhs: Operand,
    },
    AddrOf {
        dst: Operand,
        var: String,
    },
    Load {
        dst: Operand,
        addr: Operand,
    },
    Store {
        addr: Operand,
        src: Operand,
    },
    Label(String),
    Jump(String),
    BranchZero {
        cond: Operand,
        label: String,
    },
    Call {
        dst: Operand,
        callee: String,
        args: Vec<Operand>,
    },
    Return(Option<Operand>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<IrInstr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
}

pub struct IrLowerer {
    temp_counter: usize,
    label_counter: usize,
    var_counter: usize,
    scopes: Vec<HashMap<String, String>>,
}

impl Default for IrLowerer {
    fn default() -> Self {
        Self::new()
    }
}

impl IrLowerer {
    pub fn new() -> Self {
        Self {
            temp_counter: 0,
            label_counter: 0,
            var_counter: 0,
            scopes: Vec::new(),
        }
    }

    pub fn lower_program(&mut self, prog: &Program) -> IrProgram {
        let mut functions = Vec::new();
        for func in &prog.functions {
            functions.push(self.lower_function(func));
        }
        IrProgram { functions }
    }

    fn lower_function(&mut self, func: &FunctionDecl) -> IrFunction {
        self.temp_counter = 0;
        self.scopes.clear();
        self.push_scope();

        let mut params = Vec::new();
        for p in &func.params {
            let unique = self.bind_var(&p.name);
            params.push(unique);
        }

        let mut body = Vec::new();
        for stmt in &func.body {
            self.lower_stmt(stmt, &mut body);
        }
        if !matches!(body.last(), Some(IrInstr::Return(_))) {
            body.push(IrInstr::Return(Some(Operand::Imm(0))));
        }

        self.pop_scope();
        IrFunction {
            name: func.name.clone(),
            params,
            body,
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt, out: &mut Vec<IrInstr>) {
        match stmt {
            Stmt::VarDecl { name, init, .. } => {
                let rhs = if let Some(expr) = init {
                    self.lower_expr(expr, out)
                } else {
                    Operand::Imm(0)
                };
                let unique = self.bind_var(name);
                out.push(IrInstr::Copy {
                    dst: Operand::Var(unique),
                    src: rhs,
                });
            }
            Stmt::Return(expr_opt) => {
                let op = expr_opt.as_ref().map(|e| self.lower_expr(e, out));
                out.push(IrInstr::Return(op));
            }
            Stmt::Expr(e) => {
                self.lower_expr(e, out);
            }
            Stmt::Block(stmts) => {
                self.push_scope();
                for s in stmts {
                    self.lower_stmt(s, out);
                }
                self.pop_scope();
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_op = self.lower_expr(cond, out);
                let else_label = self.new_label("L_else");
                let end_label = self.new_label("L_ifend");

                if let Some(else_stmt) = else_branch {
                    out.push(IrInstr::BranchZero {
                        cond: cond_op,
                        label: else_label.clone(),
                    });
                    self.lower_stmt(then_branch, out);
                    out.push(IrInstr::Jump(end_label.clone()));
                    out.push(IrInstr::Label(else_label));
                    self.lower_stmt(else_stmt, out);
                    out.push(IrInstr::Label(end_label));
                } else {
                    out.push(IrInstr::BranchZero {
                        cond: cond_op,
                        label: end_label.clone(),
                    });
                    self.lower_stmt(then_branch, out);
                    out.push(IrInstr::Label(end_label));
                }
            }
            Stmt::While { cond, body } => {
                let start_label = self.new_label("L_while_cond");
                let end_label = self.new_label("L_while_end");

                out.push(IrInstr::Label(start_label.clone()));
                let cond_op = self.lower_expr(cond, out);
                out.push(IrInstr::BranchZero {
                    cond: cond_op,
                    label: end_label.clone(),
                });
                self.lower_stmt(body, out);
                out.push(IrInstr::Jump(start_label));
                out.push(IrInstr::Label(end_label));
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => {
                self.push_scope();
                if let Some(init_stmt) = init {
                    self.lower_stmt(init_stmt, out);
                }
                let start_label = self.new_label("L_for_cond");
                let end_label = self.new_label("L_for_end");

                out.push(IrInstr::Label(start_label.clone()));
                if let Some(cond_expr) = cond {
                    let cond_op = self.lower_expr(cond_expr, out);
                    out.push(IrInstr::BranchZero {
                        cond: cond_op,
                        label: end_label.clone(),
                    });
                }
                self.lower_stmt(body, out);
                if let Some(step_expr) = step {
                    self.lower_expr(step_expr, out);
                }
                out.push(IrInstr::Jump(start_label));
                out.push(IrInstr::Label(end_label));
                self.pop_scope();
            }
        }
    }

    fn lower_expr(&mut self, expr: &Expr, out: &mut Vec<IrInstr>) -> Operand {
        match expr {
            Expr::IntLit(v) => Operand::Imm(*v),
            Expr::Var(name) => Operand::Var(self.resolve_var(name)),
            Expr::Assign { target, value } => {
                let rhs = self.lower_expr(value, out);
                match target.as_ref() {
                    Expr::Var(name) => {
                        let dst = Operand::Var(self.resolve_var(name));
                        out.push(IrInstr::Copy {
                            dst: dst.clone(),
                            src: rhs.clone(),
                        });
                        rhs
                    }
                    Expr::Unary {
                        op: UnaryOp::Deref,
                        expr: ptr_expr,
                    } => {
                        let addr = self.lower_expr(ptr_expr, out);
                        out.push(IrInstr::Store {
                            addr,
                            src: rhs.clone(),
                        });
                        rhs
                    }
                    _ => unreachable!("validated by sema"),
                }
            }
            Expr::Unary { op, expr: inner } => match op {
                UnaryOp::AddrOf => {
                    if let Expr::Var(name) = inner.as_ref() {
                        let dst = self.new_temp();
                        let resolved = self.resolve_var(name);
                        out.push(IrInstr::AddrOf {
                            dst: dst.clone(),
                            var: resolved,
                        });
                        dst
                    } else {
                        unreachable!("validated by sema")
                    }
                }
                UnaryOp::Deref => {
                    let addr = self.lower_expr(inner, out);
                    let dst = self.new_temp();
                    out.push(IrInstr::Load {
                        dst: dst.clone(),
                        addr,
                    });
                    dst
                }
                UnaryOp::Neg | UnaryOp::Not => {
                    let src = self.lower_expr(inner, out);
                    let dst = self.new_temp();
                    out.push(IrInstr::Unary {
                        dst: dst.clone(),
                        op: *op,
                        src,
                    });
                    dst
                }
            },
            Expr::Binary {
                op: BinOp::LogicalAnd,
                lhs,
                rhs,
            } => {
                let dst = self.new_temp();
                let false_label = self.new_label("L_and_false");
                let end_label = self.new_label("L_and_end");

                let lhs_op = self.lower_expr(lhs, out);
                out.push(IrInstr::BranchZero {
                    cond: lhs_op,
                    label: false_label.clone(),
                });
                let rhs_op = self.lower_expr(rhs, out);
                out.push(IrInstr::BranchZero {
                    cond: rhs_op,
                    label: false_label.clone(),
                });
                out.push(IrInstr::Copy {
                    dst: dst.clone(),
                    src: Operand::Imm(1),
                });
                out.push(IrInstr::Jump(end_label.clone()));
                out.push(IrInstr::Label(false_label));
                out.push(IrInstr::Copy {
                    dst: dst.clone(),
                    src: Operand::Imm(0),
                });
                out.push(IrInstr::Label(end_label));
                dst
            }
            Expr::Binary {
                op: BinOp::LogicalOr,
                lhs,
                rhs,
            } => {
                let dst = self.new_temp();
                let eval_rhs = self.new_label("L_or_rhs");
                let false_label = self.new_label("L_or_false");
                let end_label = self.new_label("L_or_end");

                let lhs_op = self.lower_expr(lhs, out);
                out.push(IrInstr::BranchZero {
                    cond: lhs_op,
                    label: eval_rhs.clone(),
                });
                out.push(IrInstr::Copy {
                    dst: dst.clone(),
                    src: Operand::Imm(1),
                });
                out.push(IrInstr::Jump(end_label.clone()));
                out.push(IrInstr::Label(eval_rhs));
                let rhs_op = self.lower_expr(rhs, out);
                out.push(IrInstr::BranchZero {
                    cond: rhs_op,
                    label: false_label.clone(),
                });
                out.push(IrInstr::Copy {
                    dst: dst.clone(),
                    src: Operand::Imm(1),
                });
                out.push(IrInstr::Jump(end_label.clone()));
                out.push(IrInstr::Label(false_label));
                out.push(IrInstr::Copy {
                    dst: dst.clone(),
                    src: Operand::Imm(0),
                });
                out.push(IrInstr::Label(end_label));
                dst
            }
            Expr::Binary { op, lhs, rhs } => {
                let l = self.lower_expr(lhs, out);
                let r = self.lower_expr(rhs, out);
                let dst = self.new_temp();
                out.push(IrInstr::Binary {
                    dst: dst.clone(),
                    op: *op,
                    lhs: l,
                    rhs: r,
                });
                dst
            }
            Expr::Call { callee, args } => {
                let mut lowered_args = Vec::with_capacity(args.len());
                for a in args {
                    lowered_args.push(self.lower_expr(a, out));
                }
                let dst = self.new_temp();
                out.push(IrInstr::Call {
                    dst: dst.clone(),
                    callee: callee.clone(),
                    args: lowered_args,
                });
                dst
            }
        }
    }

    fn new_temp(&mut self) -> Operand {
        let id = self.temp_counter;
        self.temp_counter += 1;
        Operand::Temp(id)
    }

    fn new_label(&mut self, prefix: &str) -> String {
        let id = self.label_counter;
        self.label_counter += 1;
        format!("{}_{}", prefix, id)
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn bind_var(&mut self, name: &str) -> String {
        let id = self.var_counter;
        self.var_counter += 1;
        let unique = format!("{}_{}", name, id);
        self.scopes
            .last_mut()
            .expect("active scope")
            .insert(name.to_string(), unique.clone());
        unique
    }

    fn resolve_var(&self, name: &str) -> String {
        for s in self.scopes.iter().rev() {
            if let Some(u) = s.get(name) {
                return u.clone();
            }
        }
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    #[test]
    fn test_ir_lowers_arithmetic_and_branches() {
        let src = r#"
            int main(void) {
                int a = 2 + 3 * 4;
                if (a > 10) { return a; }
                return 0;
            }
        "#;
        let tokens = Lexer::new(src).tokenize().unwrap();
        let prog = Parser::new(tokens).parse_program().unwrap();
        let ir = IrLowerer::new().lower_program(&prog);
        assert_eq!(ir.functions.len(), 1);
        assert!(ir.functions[0]
            .body
            .iter()
            .any(|i| matches!(i, IrInstr::BranchZero { .. })));
    }
}
