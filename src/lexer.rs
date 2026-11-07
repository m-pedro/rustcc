use crate::error::{CompileError, Result, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // Keywords
    Int,
    Void,
    Return,
    If,
    Else,
    While,
    For,
    // Identifiers & Literals
    Ident(String),
    IntLit(i64),
    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Ampersand,
    Assign,
    EqEq,
    Bang,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AmpAmp,
    PipePipe,
    // Delimiters
    LParen,
    RParen,
    LBrace,
    RBrace,
    Semicolon,
    Comma,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments()?;
            let span = Span {
                line: self.line,
                col: self.col,
            };
            let Some(ch) = self.peek() else {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    span,
                });
                break;
            };

            let kind = match ch {
                '(' => {
                    self.advance();
                    TokenKind::LParen
                }
                ')' => {
                    self.advance();
                    TokenKind::RParen
                }
                '{' => {
                    self.advance();
                    TokenKind::LBrace
                }
                '}' => {
                    self.advance();
                    TokenKind::RBrace
                }
                ';' => {
                    self.advance();
                    TokenKind::Semicolon
                }
                ',' => {
                    self.advance();
                    TokenKind::Comma
                }
                '+' => {
                    self.advance();
                    TokenKind::Plus
                }
                '-' => {
                    self.advance();
                    TokenKind::Minus
                }
                '*' => {
                    self.advance();
                    TokenKind::Star
                }
                '/' => {
                    self.advance();
                    TokenKind::Slash
                }
                '%' => {
                    self.advance();
                    TokenKind::Percent
                }
                '=' => {
                    self.advance();
                    if self.match_char('=') {
                        TokenKind::EqEq
                    } else {
                        TokenKind::Assign
                    }
                }
                '!' => {
                    self.advance();
                    if self.match_char('=') {
                        TokenKind::BangEq
                    } else {
                        TokenKind::Bang
                    }
                }
                '<' => {
                    self.advance();
                    if self.match_char('=') {
                        TokenKind::LtEq
                    } else {
                        TokenKind::Lt
                    }
                }
                '>' => {
                    self.advance();
                    if self.match_char('=') {
                        TokenKind::GtEq
                    } else {
                        TokenKind::Gt
                    }
                }
                '&' => {
                    self.advance();
                    if self.match_char('&') {
                        TokenKind::AmpAmp
                    } else {
                        TokenKind::Ampersand
                    }
                }
                '|' => {
                    self.advance();
                    if self.match_char('|') {
                        TokenKind::PipePipe
                    } else {
                        return Err(CompileError::new(
                            "lexer",
                            "unexpected single '|'; did you mean '||'?",
                            span,
                        ));
                    }
                }
                c if c.is_ascii_digit() => self.lex_number(span)?,
                c if c.is_ascii_alphabetic() || c == '_' => self.lex_ident_or_keyword(),
                other => {
                    return Err(CompileError::new(
                        "lexer",
                        format!("unexpected character '{}'", other),
                        span,
                    ))
                }
            };

            tokens.push(Token { kind, span });
        }
        Ok(tokens)
    }

    fn lex_number(&mut self, span: Span) -> Result<TokenKind> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        let val = s.parse::<i64>().map_err(|_| {
            CompileError::new("lexer", format!("invalid integer literal '{}'", s), span)
        })?;
        Ok(TokenKind::IntLit(val))
    }

    fn lex_ident_or_keyword(&mut self) -> TokenKind {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        match s.as_str() {
            "int" => TokenKind::Int,
            "void" => TokenKind::Void,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            _ => TokenKind::Ident(s),
        }
    }

    fn skip_whitespace_and_comments(&mut self) -> Result<()> {
        loop {
            while let Some(c) = self.peek() {
                if c.is_whitespace() {
                    self.advance();
                } else {
                    break;
                }
            }
            // Single-line // comment
            if self.peek() == Some('/') && self.peek_next() == Some('/') {
                while let Some(c) = self.peek() {
                    self.advance();
                    if c == '\n' {
                        break;
                    }
                }
                continue;
            }
            // Multi-line /* ... */ comment
            if self.peek() == Some('/') && self.peek_next() == Some('*') {
                let start_span = Span {
                    line: self.line,
                    col: self.col,
                };
                self.advance();
                self.advance();
                let mut closed = false;
                while let Some(c) = self.peek() {
                    if c == '*' && self.peek_next() == Some('/') {
                        self.advance();
                        self.advance();
                        closed = true;
                        break;
                    }
                    self.advance();
                }
                if !closed {
                    return Err(CompileError::new(
                        "lexer",
                        "unterminated block comment",
                        start_span,
                    ));
                }
                continue;
            }
            break;
        }
        Ok(())
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_keywords_operators_and_comments() {
        let src = r#"
            // Compute factorial
            int fact(int n) {
                if (n <= 1) return 1;
                /* recursive step */
                return n * fact(n - 1);
            }
        "#;
        let mut lex = Lexer::new(src);
        let tokens = lex.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Int);
        assert_eq!(tokens[1].kind, TokenKind::Ident("fact".into()));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::LtEq));
        assert_eq!(tokens.last().unwrap().kind, TokenKind::Eof);
    }
}
