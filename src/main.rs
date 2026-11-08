use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{self, Command};

use rustcc::codegen::{emit_assembly, TargetArch};
use rustcc::ir::IrLowerer;
use rustcc::lexer::Lexer;
use rustcc::opt::optimize_program;
use rustcc::parser::Parser;
use rustcc::sema::TypeChecker;

fn print_usage() {
    eprintln!(
        "Usage: rustcc <input.c> [options]\n\
         Options:\n\
         \x20 --dump-tokens          Print lexed tokens and exit\n\
         \x20 --dump-ast             Print parsed Abstract Syntax Tree and exit\n\
         \x20 --dump-ir              Print Three-Address Code (TAC) IR and exit\n\
         \x20 --no-opt               Disable IR constant folding & dead-code elimination\n\
         \x20 --arch <aarch64|x86_64> Target assembly architecture (default: host)\n\
         \x20 -S                     Emit assembly (.s) instead of linking a binary\n\
         \x20 -o <path>              Output file path (default: a.out or out.s)"
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        process::exit(if args.is_empty() { 1 } else { 0 });
    }

    let mut input_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut dump_tokens = false;
    let mut dump_ast = false;
    let mut dump_ir = false;
    let mut emit_asm_only = false;
    let mut optimize = true;
    let mut arch = TargetArch::host();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dump-tokens" => dump_tokens = true,
            "--dump-ast" => dump_ast = true,
            "--dump-ir" => dump_ir = true,
            "--no-opt" => optimize = false,
            "-S" => emit_asm_only = true,
            "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: -o requires an output path");
                    process::exit(1);
                }
                output_path = Some(PathBuf::from(&args[i]));
            }
            "--arch" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --arch requires 'aarch64' or 'x86_64'");
                    process::exit(1);
                }
                arch = match args[i].as_str() {
                    "aarch64" | "arm64" => TargetArch::Aarch64,
                    "x86_64" | "amd64" => TargetArch::X86_64,
                    other => {
                        eprintln!("error: unsupported architecture '{}'", other);
                        process::exit(1);
                    }
                };
            }
            arg if !arg.starts_with('-') => input_path = Some(PathBuf::from(arg)),
            other => {
                eprintln!("error: unknown option '{}'", other);
                print_usage();
                process::exit(1);
            }
        }
        i += 1;
    }

    let Some(input) = input_path else {
        eprintln!("error: missing input .c source file");
        process::exit(1);
    };

    let source = match fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {}", input.display(), e);
            process::exit(1);
        }
    };

    let tokens = match Lexer::new(&source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };
    if dump_tokens {
        for tok in &tokens {
            println!("{}:{} {:?}", tok.span.line, tok.span.col, tok.kind);
        }
        return;
    }

    let ast = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };
    if dump_ast {
        println!("{:#?}", ast);
        return;
    }

    if let Err(e) = TypeChecker::new().check_program(&ast) {
        eprintln!("{}", e);
        process::exit(1);
    }

    let raw_ir = IrLowerer::new().lower_program(&ast);
    let ir = if optimize {
        optimize_program(&raw_ir)
    } else {
        raw_ir
    };
    if dump_ir {
        for func in &ir.functions {
            println!("fn {}({}):", func.name, func.params.join(", "));
            for instr in &func.body {
                println!("  {:?}", instr);
            }
        }
        return;
    }

    let asm = emit_assembly(&ir, arch);
    if emit_asm_only {
        let out = output_path.unwrap_or_else(|| input.with_extension("s"));
        fs::write(&out, asm).expect("failed to write assembly output");
        println!("wrote assembly to {}", out.display());
        return;
    }

    let bin_out = output_path.unwrap_or_else(|| PathBuf::from("a.out"));
    let tmp_asm = std::env::temp_dir().join("rustcc_tmp.s");
    fs::write(&tmp_asm, asm).expect("failed to write temporary assembly file");

    let status = Command::new("cc")
        .arg(&tmp_asm)
        .arg("-o")
        .arg(&bin_out)
        .status()
        .expect("failed to invoke system C linker (cc)");
    let _ = fs::remove_file(&tmp_asm);

    if !status.success() {
        eprintln!("error: system linker failed");
        process::exit(1);
    }
}
