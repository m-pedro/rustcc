pub mod ast;
pub mod codegen;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod opt;
pub mod parser;
pub mod sema;

use codegen::{emit_assembly, TargetArch};
use error::Result;
use ir::IrLowerer;
use lexer::Lexer;
use opt::optimize_program;
use parser::Parser;
use sema::TypeChecker;

/// Compiles C source code through Lexer -> Parser -> Sema -> TAC IR -> Optimizer -> Assembly.
pub fn compile_to_asm(source: &str, arch: TargetArch, optimize: bool) -> Result<String> {
    let tokens = Lexer::new(source).tokenize()?;
    let ast = Parser::new(tokens).parse_program()?;
    TypeChecker::new().check_program(&ast)?;
    let ir = IrLowerer::new().lower_program(&ast);
    let final_ir = if optimize {
        optimize_program(&ir)
    } else {
        ir
    };
    Ok(emit_assembly(&final_ir, arch))
}
