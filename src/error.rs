use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub phase: &'static str,
    pub message: String,
    pub span: Span,
}

impl CompileError {
    pub fn new(phase: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            phase,
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{} error at {}:{}] {}",
            self.phase, self.span.line, self.span.col, self.message
        )
    }
}

impl std::error::Error for CompileError {}

pub type Result<T> = std::result::Result<T, CompileError>;
