use crate::ast::{BinOp, Expr, FunctionDecl, Param, Program, Stmt, Type, UnaryOp};
use crate::error::{CompileError, Result, Span};
use crate::lexer::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Program> {
        let mut functions = Vec::new();
        while !self.is_at_end() {
            functions.push(self.parse_function()?);
        }
        Ok(Program { functions })
    }

    fn parse_function(&mut self) -> Result<FunctionDecl> {
        let return_type = self.parse_type()?;
        let name = self.consume_ident("expected function name")?;
        self.consume(&TokenKind::LParen, "expected '(' after function name")?;

        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            if self.check(&TokenKind::Void) && self.peek_next_kind() == Some(&TokenKind::RParen) {
                self.advance(); // handle `int main(void)`
            } else {
                loop {
                    let ty = self.parse_type()?;
                    let param_name = self.consume_ident("expected parameter name")?;
                    params.push(Param {
                        ty,
                        name: param_name,
                    });
                    if !self.match_kind(&TokenKind::Comma) {
                        break;
                    }
                }
            }
        }
        self.consume(&TokenKind::RParen, "expected ')' after parameters")?;
        self.consume(&TokenKind::LBrace, "expected '{' before function body")?;

        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            body.push(self.parse_stmt()?);
        }
        self.consume(&TokenKind::RBrace, "expected '}' after function body")?;

        Ok(FunctionDecl {
            return_type,
            name,
            params,
            body,
        })
    }

    fn parse_type(&mut self) -> Result<Type> {
        let mut base = match &self.peek().kind {
            TokenKind::Int => {
                self.advance();
                Type::Int
            }
            TokenKind::Void => {
                self.advance();
                Type::Void
            }
            _ => {
                return Err(CompileError::new(
                    "parser",
                    format!("expected type specifier, found {:?}", self.peek().kind),
                    self.peek().span,
                ))
            }
        };
        while self.match_kind(&TokenKind::Star) {
            base = Type::Pointer(Box::new(base));
        }
        Ok(base)
    }

    fn parse_stmt(&mut self) -> Result<Stmt> {
        if self.check(&TokenKind::Int) || self.check(&TokenKind::Void) {
            let decl = self.parse_var_decl()?;
            self.consume(&TokenKind::Semicolon, "expected ';' after variable declaration")?;
            return Ok(decl);
        }
        if self.match_kind(&TokenKind::Return) {
            if self.match_kind(&TokenKind::Semicolon) {
                return Ok(Stmt::Return(None));
            }
            let expr = self.parse_expr(0)?;
            self.consume(&TokenKind::Semicolon, "expected ';' after return value")?;
            return Ok(Stmt::Return(Some(expr)));
        }
        if self.match_kind(&TokenKind::LBrace) {
            let mut stmts = Vec::new();
            while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                stmts.push(self.parse_stmt()?);
            }
            self.consume(&TokenKind::RBrace, "expected '}' after block")?;
            return Ok(Stmt::Block(stmts));
        }
        if self.match_kind(&TokenKind::If) {
            self.consume(&TokenKind::LParen, "expected '(' after 'if'")?;
            let cond = self.parse_expr(0)?;
            self.consume(&TokenKind::RParen, "expected ')' after if condition")?;
            let then_branch = Box::new(self.parse_stmt()?);
            let else_branch = if self.match_kind(&TokenKind::Else) {
                Some(Box::new(self.parse_stmt()?))
            } else {
                None
            };
            return Ok(Stmt::If {
                cond,
                then_branch,
                else_branch,
            });
        }
        if self.match_kind(&TokenKind::While) {
            self.consume(&TokenKind::LParen, "expected '(' after 'while'")?;
            let cond = self.parse_expr(0)?;
            self.consume(&TokenKind::RParen, "expected ')' after while condition")?;
            let body = Box::new(self.parse_stmt()?);
            return Ok(Stmt::While { cond, body });
        }
        if self.match_kind(&TokenKind::For) {
            self.consume(&TokenKind::LParen, "expected '(' after 'for'")?;
            let init = if self.match_kind(&TokenKind::Semicolon) {
                None
            } else if self.check(&TokenKind::Int) {
                let d = self.parse_var_decl()?;
                self.consume(&TokenKind::Semicolon, "expected ';' after for-init")?;
                Some(Box::new(d))
            } else {
                let e = self.parse_expr(0)?;
                self.consume(&TokenKind::Semicolon, "expected ';' after for-init")?;
                Some(Box::new(Stmt::Expr(e)))
            };

            let cond = if self.match_kind(&TokenKind::Semicolon) {
                None
            } else {
                let c = self.parse_expr(0)?;
                self.consume(&TokenKind::Semicolon, "expected ';' after for-condition")?;
                Some(c)
            };

            let step = if self.match_kind(&TokenKind::RParen) {
                None
            } else {
                let s = self.parse_expr(0)?;
                self.consume(&TokenKind::RParen, "expected ')' after for-step")?;
                Some(s)
            };

            let body = Box::new(self.parse_stmt()?);
            return Ok(Stmt::For {
                init,
                cond,
                step,
                body,
            });
        }

        let expr = self.parse_expr(0)?;
        self.consume(&TokenKind::Semicolon, "expected ';' after expression")?;
        Ok(Stmt::Expr(expr))
    }

    fn parse_var_decl(&mut self) -> Result<Stmt> {
        let ty = self.parse_type()?;
        let name = self.consume_ident("expected variable name")?;
        let init = if self.match_kind(&TokenKind::Assign) {
            Some(self.parse_expr(0)?)
        } else {
            None
        };
        Ok(Stmt::VarDecl { ty, name, init })
    }

    /// Pratt operator-precedence parser for expressions.
    pub fn parse_expr(&mut self, min_bp: u8) -> Result<Expr> {
        let mut lhs = self.parse_prefix()?;

        loop {
            if self.check(&TokenKind::Assign) {
                let (l_bp, r_bp) = (2, 1); // Right-associative assignment
                if l_bp < min_bp {
                    break;
                }
                self.advance();
                let rhs = self.parse_expr(r_bp)?;
                lhs = Expr::Assign {
                    target: Box::new(lhs),
                    value: Box::new(rhs),
                };
                continue;
            }

            let Some((op, l_bp, r_bp)) = self.infix_binding_power() else {
                break;
            };
            if l_bp < min_bp {
                break;
            }
            self.advance();
            let rhs = self.parse_expr(r_bp)?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::IntLit(v) => {
                self.advance();
                Ok(Expr::IntLit(v))
            }
            TokenKind::Ident(name) => {
                self.advance();
                if self.match_kind(&TokenKind::LParen) {
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr(0)?);
                            if !self.match_kind(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.consume(&TokenKind::RParen, "expected ')' after arguments")?;
                    Ok(Expr::Call { callee: name, args })
                } else {
                    Ok(Expr::Var(name))
                }
            }
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_expr(25)?;
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Bang => {
                self.advance();
                let expr = self.parse_expr(25)?;
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Ampersand => {
                self.advance();
                let expr = self.parse_expr(25)?;
                Ok(Expr::Unary {
                    op: UnaryOp::AddrOf,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Star => {
                self.advance();
                let expr = self.parse_expr(25)?;
                Ok(Expr::Unary {
                    op: UnaryOp::Deref,
                    expr: Box::new(expr),
                })
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr(0)?;
                self.consume(&TokenKind::RParen, "expected ')' after grouped expression")?;
                Ok(expr)
            }
            _ => Err(CompileError::new(
                "parser",
                format!("unexpected token {:?} in expression", tok.kind),
                tok.span,
            )),
        }
    }

    fn infix_binding_power(&self) -> Option<(BinOp, u8, u8)> {
        let op = match self.peek().kind {
            TokenKind::PipePipe => (BinOp::LogicalOr, 3, 4),
            TokenKind::AmpAmp => (BinOp::LogicalAnd, 5, 6),
            TokenKind::EqEq => (BinOp::Eq, 9, 10),
            TokenKind::BangEq => (BinOp::Ne, 9, 10),
            TokenKind::Lt => (BinOp::Lt, 11, 12),
            TokenKind::LtEq => (BinOp::Le, 11, 12),
            TokenKind::Gt => (BinOp::Gt, 11, 12),
            TokenKind::GtEq => (BinOp::Ge, 11, 12),
            TokenKind::Plus => (BinOp::Add, 15, 16),
            TokenKind::Minus => (BinOp::Sub, 15, 16),
            TokenKind::Star => (BinOp::Mul, 17, 18),
            TokenKind::Slash => (BinOp::Div, 17, 18),
            TokenKind::Percent => (BinOp::Mod, 17, 18),
            _ => return None,
        };
        Some(op)
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_next_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos + 1).map(|t| &t.kind)
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.pos += 1;
        }
        &self.tokens[self.pos - 1]
    }

    fn check(&self, kind: &TokenKind) -> bool {
        &self.peek().kind == kind
    }

    fn match_kind(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, kind: &TokenKind, msg: &str) -> Result<()> {
        if self.check(kind) {
            self.advance();
            Ok(())
        } else {
            Err(CompileError::new("parser", msg, self.peek().span))
        }
    }

    fn consume_ident(&mut self, msg: &str) -> Result<String> {
        let span: Span = self.peek().span;
        if let TokenKind::Ident(name) = &self.peek().kind {
            let cloned = name.clone();
            self.advance();
            Ok(cloned)
        } else {
            Err(CompileError::new("parser", msg, span))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn test_pratt_precedence_and_control_flow_ast() {
        let src = r#"
            int compute(int a, int b) {
                int x = a + b * 2;
                if (x > 10 && a != 0) {
                    return x - 1;
                }
                return 0;
            }
        "#;
        let tokens = Lexer::new(src).tokenize().unwrap();
        let prog = Parser::new(tokens).parse_program().unwrap();
        assert_eq!(prog.functions.len(), 1);
        let f = &prog.functions[0];
        assert_eq!(f.name, "compute");
        assert_eq!(f.params.len(), 2);

        // Verify `a + b * 2` parsed as `a + (b * 2)`
        match &f.body[0] {
            Stmt::VarDecl {
                init: Some(Expr::Binary { op, rhs, .. }),
                ..
            } => {
                assert_eq!(*op, BinOp::Add);
                assert!(matches!(**rhs, Expr::Binary { op: BinOp::Mul, .. }));
            }
            other => panic!("unexpected stmt: {:?}", other),
        }
    }
}
