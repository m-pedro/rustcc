# rustcc

A hand-written C subset compiler written from scratch in Rust with **zero external parser or codegen dependencies**. Compiles C source code through a 6-stage pipeline: Span-Tracked Lexer -> Pratt/Recursive-Descent AST Parser -> Scoped Semantic Type Checker -> Three-Address Code (TAC) IR -> Multi-Pass Optimizer -> Native **Apple Silicon (`aarch64`)** and **`x86_64`** Assembly Codegen.

---

## Repository Structure

```text
rustcc/
├── Cargo.toml                      # Zero runtime dependencies; Criterion bench target
├── LICENSE                         # MIT License
├── README.md                       # Compiler architecture, IR/ASM diagrams & benchmarks
├── benches/
│   └── compiler_bench.rs           # Full pipeline (Lexer -> ASM) throughput benchmark
├── src/
│   ├── lib.rs                      # Public compile_to_asm() pipeline entrypoint
│   ├── main.rs                     # CLI driver (--dump-tokens, --dump-ast, --dump-ir, -S, -o)
│   ├── error.rs                    # Line/Column Span & CompileError diagnostics
│   ├── lexer.rs                    # Hand-written C tokenizer with comment & operator support
│   ├── ast.rs                      # C types, pointers, expressions, statements & functions
│   ├── parser.rs                   # Recursive-descent statement + Pratt expression parser
│   ├── sema.rs                     # Lexical scope symbol table & static type checker
│   ├── ir.rs                       # Three-Address Code (TAC) IR & short-circuit lowering
│   ├── opt.rs                      # Constant folding, propagation, algebraic & DCE passes
│   └── codegen.rs                  # 16B-aligned stack frame allocator + aarch64 & x86_64 backends
└── tests/
    ├── control_flow_test.rs        # Native execution of if/else, while/for & short-circuit logic
    ├── calls_recursion_test.rs     # ABI 6-arg calls, Fibonacci, Euclid GCD & mutual recursion
    ├── pointers_test.rs            # Address-of (&), dereference (*), double pointers & swap()
    └── e2e_suite_test.rs           # Collatz steps, prime sieve & modular exponentiation
```

---

## Six-Stage Compiler Architecture

```text
 [C Source Code (.c)]
          │
          ▼
 ┌──────────────────────────────────────────────────────────┐
 │ 1. Lexer (src/lexer.rs)                                  │
 │    - Line/column Span tracking, // and /* */ comments    │
 └────────┬─────────────────────────────────────────────────┘
          ▼
 ┌──────────────────────────────────────────────────────────┐
 │ 2. Parser (src/parser.rs)                                │
 │    - Pratt operator-precedence climbing for expressions  │
 │    - Recursive descent for declarations & control flow   │
 └────────┬─────────────────────────────────────────────────┘
          ▼
 ┌──────────────────────────────────────────────────────────┐
 │ 3. Semantic Analyzer (src/sema.rs)                       │
 │    - Lexical block scope stack, pointer & call checking  │
 └────────┬─────────────────────────────────────────────────┘
          ▼
 ┌──────────────────────────────────────────────────────────┐
 │ 4. Three-Address Code IR Lowerer (src/ir.rs)             │
 │    - Virtual temporaries (%t0), labels & short-circuiting│
 └────────┬─────────────────────────────────────────────────┘
          ▼
 ┌──────────────────────────────────────────────────────────┐
 │ 5. Multi-Pass IR Optimizer (src/opt.rs)                  │
 │    - Constant folding, propagation, algebraic & DCE      │
 └────────┬─────────────────────────────────────────────────┘
          ▼
 ┌──────────────────────────────────────────────────────────┐
 │ 6. Native Assembly Codegen (src/codegen.rs)              │
 │    - Apple Silicon (aarch64 Mach-O) & x86_64 (System V)  │
 └──────────────────────────────────────────────────────────┘
```

---

## Criterion Benchmark Summary (Apple Silicon)

Run locally via `cargo bench --bench compiler_bench -- --quick`:

| Benchmark | Pipeline Stages | Mean Latency | Throughput |
| :--- | :--- | ---: | ---: |
| `rustcc_end_to_end_aarch64_opt` (Fibonacci + Collatz C program) | Lex -> Parse -> Sema -> TAC IR -> Opt (4 passes) -> `aarch64` ASM | ~48.5 µs | ~20,600 compilations/sec |

---

## Quickstart

### 1. Compile and Run a Native C Program on macOS / Linux

```bash
cargo run -- fib.c -o fib_bin
./fib_bin
echo "Exit code: $?"
```

### 2. Inspect Intermediate Representations

```bash
cargo run -- fib.c --dump-tokens
cargo run -- fib.c --dump-ast
cargo run -- fib.c --dump-ir
cargo run -- fib.c -S --arch aarch64 -o fib_arm64.s
cargo run -- fib.c -S --arch x86_64 -o fib_x86.s
```

### 3. Run the Full Test & Native Execution Suite

```bash
cargo test --all-targets
```

---

## 15-Day Incremental Engineering Roadmap (Month 3, Days 61–75)

- [x] **Day 61:** Initialize `rustcc` compiler crate, span error diagnostics, and 15-day roadmap
- [x] **Day 62:** Implement hand-written C tokenizer with source spans, keywords, and multi-char operators (`src/lexer.rs`)
- [x] **Day 63:** Define C abstract syntax tree nodes for types, expressions, statements, and functions (`src/ast.rs`)
- [x] **Day 64:** Implement recursive-descent statement parser and Pratt operator-precedence expression parser (`src/parser.rs`)
- [x] **Day 65:** Implement lexical scope resolver, symbol table, and static type checker (`src/sema.rs`)
- [x] **Day 66:** Lower typed AST into Three-Address Code (TAC) intermediate representation (`src/ir.rs`)
- [x] **Day 67:** Implement IR constant folding, algebraic simplification, and dead-code elimination (`src/opt.rs`)
- [x] **Day 68:** Implement stack-frame allocator and `x86_64` assembly generator (`src/codegen.rs`)
- [x] **Day 69:** Add native Apple Silicon `aarch64` assembly backend for macOS Mach-O execution
- [x] **Day 70:** Compile `if`/`else` branches, `while`/`for` loops, and short-circuit boolean logic
- [x] **Day 71:** Implement ABI-compliant function calls, recursion, and register saving
- [x] **Day 72:** Support address-of (`&`), dereference (`*`), and local stack pointer arithmetic
- [x] **Day 73:** Add end-to-end execution test suite compiling C programs to native Mach-O binaries
- [x] **Day 74:** Build `rustcc` driver CLI (`--dump-tokens`, `--dump-ast`, `--dump-ir`, `-S`, `-o`)
- [x] **Day 75:** Complete compiler architecture guide, IR/ASM pipeline diagrams, and benchmark suite
