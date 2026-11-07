use std::collections::HashMap;

use crate::ast::{Expr, FunctionDecl, Program, Stmt, Type, UnaryOp};
use crate::error::{CompileError, Result, Span};

#[derive(Debug, Clone)]
struct FuncSig {
    return_type: Type,
    param_types: Vec<Type>,
}

pub struct TypeChecker {
    functions: HashMap<String, FuncSig>,
    scopes: Vec<HashMap<String, Type>>,
    current_return_type: Type,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            scopes: Vec::new(),
            current_return_type: Type::Int,
        }
    }

    pub fn check_program(&mut self, prog: &Program) -> Result<()> {
        for func in &prog.functions {
            if self.functions.contains_key(&func.name) {
                return Err(CompileError::new(
                    "sema",
                    format!("duplicate function definition '{}'", func.name),
                    Span { line: 1, col: 1 },
                ));
            }
            self.functions.insert(
                func.name.clone(),
                FuncSig {
                    return_type: func.return_type.clone(),
                    param_types: func.params.iter().map(|p| p.ty.clone()).collect(),
                },
            );
        }

        for func in &prog.functions {
            self.check_function(func)?;
        }
        Ok(())
    }

    fn check_function(&mut self, func: &FunctionDecl) -> Result<()> {
        self.current_return_type = func.return_type.clone();
        self.push_scope();
        for param in &func.params {
            self.declare_var(&param.name, param.ty.clone())?;
        }
        for stmt in &func.body {
            self.check_stmt(stmt)?;
        }
        self.pop_scope();
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::VarDecl { ty, name, init } => {
                if *ty == Type::Void {
                    return Err(CompileError::new(
                        "sema",
                        format!("variable '{}' cannot have type 'void'", name),
                        Span { line: 1, col: 1 },
                    ));
                }
                if let Some(expr) = init {
                    let init_ty = self.check_expr(expr)?;
                    if &init_ty != ty {
                        return Err(CompileError::new(
                            "sema",
                            format!(
                                "type mismatch initializing '{}': expected {}, got {}",
                                name, ty, init_ty
                            ),
                            Span { line: 1, col: 1 },
                        ));
                    }
                }
                self.declare_var(name, ty.clone())
            }
            Stmt::Return(expr_opt) => match (&self.current_return_type, expr_opt) {
                (Type::Void, None) => Ok(()),
                (Type::Void, Some(_)) => Err(CompileError::new(
                    "sema",
                    "void function cannot return a value",
                    Span { line: 1, col: 1 },
                )),
                (expected_ty, None) => Err(CompileError::new(
                    "sema",
                    format!("non-void function must return a value of type {}", expected_ty),
                    Span { line: 1, col: 1 },
                )),
                (expected_ty, Some(e)) => {
                    let expected = expected_ty.clone();
                    let actual = self.check_expr(e)?;
                    if actual != expected {
                        return Err(CompileError::new(
                            "sema",
                            format!("return type mismatch: expected {}, got {}", expected, actual),
                            Span { line: 1, col: 1 },
                        ));
                    }
                    Ok(())
                }
            },
            Stmt::Expr(e) => {
                self.check_expr(e)?;
                Ok(())
            }
            Stmt::Block(stmts) => {
                self.push_scope();
                for s in stmts {
                    self.check_stmt(s)?;
                }
                self.pop_scope();
                Ok(())
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.check_expr(cond)?;
                self.check_stmt(then_branch)?;
                if let Some(eb) = else_branch {
                    self.check_stmt(eb)?;
                }
                Ok(())
            }
            Stmt::While { cond, body } => {
                self.check_expr(cond)?;
                self.check_stmt(body)
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => {
                self.push_scope();
                if let Some(i) = init {
                    self.check_stmt(i)?;
                }
                if let Some(c) = cond {
                    self.check_expr(c)?;
                }
                if let Some(s) = step {
                    self.check_expr(s)?;
                }
                self.check_stmt(body)?;
                self.pop_scope();
                Ok(())
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Type> {
        match expr {
            Expr::IntLit(_) => Ok(Type::Int),
            Expr::Var(name) => self.lookup_var(name),
            Expr::Assign { target, value } => {
                let lhs_ty = match target.as_ref() {
                    Expr::Var(name) => self.lookup_var(name)?,
                    Expr::Unary {
                        op: UnaryOp::Deref,
                        expr: inner,
                    } => match self.check_expr(inner)? {
                        Type::Pointer(pointee) => *pointee,
                        other => {
                            return Err(CompileError::new(
                                "sema",
                                format!("cannot dereference non-pointer type {}", other),
                                Span { line: 1, col: 1 },
                            ))
                        }
                    },
                    _ => {
                        return Err(CompileError::new(
                            "sema",
                            "left-hand side of assignment must be a variable or dereferenced pointer",
                            Span { line: 1, col: 1 },
                        ))
                    }
                };
                let rhs_ty = self.check_expr(value)?;
                if lhs_ty != rhs_ty {
                    return Err(CompileError::new(
                        "sema",
                        format!("assignment type mismatch: cannot assign {} to {}", rhs_ty, lhs_ty),
                        Span { line: 1, col: 1 },
                    ));
                }
                Ok(lhs_ty)
            }
            Expr::Unary { op, expr: inner } => {
                let inner_ty = self.check_expr(inner)?;
                match op {
                    UnaryOp::Neg | UnaryOp::Not => {
                        if inner_ty != Type::Int {
                            return Err(CompileError::new(
                                "sema",
                                format!("unary operator requires int operand, got {}", inner_ty),
                                Span { line: 1, col: 1 },
                            ));
                        }
                        Ok(Type::Int)
                    }
                    UnaryOp::AddrOf => {
                        if !matches!(inner.as_ref(), Expr::Var(_)) {
                            return Err(CompileError::new(
                                "sema",
                                "address-of '&' requires a local variable lvalue",
                                Span { line: 1, col: 1 },
                            ));
                        }
                        Ok(Type::Pointer(Box::new(inner_ty)))
                    }
                    UnaryOp::Deref => match inner_ty {
                        Type::Pointer(pointee) => Ok(*pointee),
                        other => Err(CompileError::new(
                            "sema",
                            format!("cannot dereference non-pointer type {}", other),
                            Span { line: 1, col: 1 },
                        )),
                    },
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                let lt = self.check_expr(lhs)?;
                let rt = self.check_expr(rhs)?;
                if lt != Type::Int || rt != Type::Int {
                    return Err(CompileError::new(
                        "sema",
                        format!("binary arithmetic requires int operands, got {} and {}", lt, rt),
                        Span { line: 1, col: 1 },
                    ));
                }
                Ok(Type::Int)
            }
            Expr::Call { callee, args } => {
                let sig = self.functions.get(callee).cloned().ok_or_else(|| {
                    CompileError::new(
                        "sema",
                        format!("call to undeclared function '{}'", callee),
                        Span { line: 1, col: 1 },
                    )
                })?;
                if args.len() != sig.param_types.len() {
                    return Err(CompileError::new(
                        "sema",
                        format!(
                            "function '{}' expects {} arguments, got {}",
                            callee,
                            sig.param_types.len(),
                            args.len()
                        ),
                        Span { line: 1, col: 1 },
                    ));
                }
                for (arg, expected) in args.iter().zip(sig.param_types.iter()) {
                    let actual = self.check_expr(arg)?;
                    if &actual != expected {
                        return Err(CompileError::new(
                            "sema",
                            format!(
                                "argument type mismatch in call to '{}': expected {}, got {}",
                                callee, expected, actual
                            ),
                            Span { line: 1, col: 1 },
                        ));
                    }
                }
                Ok(sig.return_type)
            }
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_var(&mut self, name: &str, ty: Type) -> Result<()> {
        let current = self.scopes.last_mut().expect("active scope");
        if current.contains_key(name) {
            return Err(CompileError::new(
                "sema",
                format!("redeclaration of variable '{}' in the same scope", name),
                Span { line: 1, col: 1 },
            ));
        }
        current.insert(name.to_string(), ty);
        Ok(())
    }

    fn lookup_var(&self, name: &str) -> Result<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Ok(ty.clone());
            }
        }
        Err(CompileError::new(
            "sema",
            format!("use of undeclared variable '{}'", name),
            Span { line: 1, col: 1 },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn check(src: &str) -> Result<()> {
        let tokens = Lexer::new(src).tokenize()?;
        let prog = Parser::new(tokens).parse_program()?;
        TypeChecker::new().check_program(&prog)
    }

    #[test]
    fn test_sema_accepts_valid_pointers_and_calls() {
        let src = r#"
            int add(int a, int b) { return a + b; }
            int main(void) {
                int x = 10;
                int *p = &x;
                *p = add(x, 5);
                return *p;
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_sema_rejects_undeclared_var_and_bad_deref() {
        let bad_undef = "int main(void) { return missing_var; }";
        assert!(check(bad_undef).is_err());

        let bad_deref = "int main(void) { int x = 5; return *x; }";
        assert!(check(bad_deref).is_err());
    }
}
