# rustcc

A hand-written C subset compiler written from scratch in Rust with **zero external parser dependencies**. Compiles C source code through a 6-stage pipeline: Lexer -> Pratt/Recursive-Descent AST Parser -> Scoped Semantic Type Checker -> Three-Address Code (TAC) IR -> Constant-Folding Optimizer -> Native Apple Silicon (`aarch64`) and `x86_64` Assembly Codegen.

## 15-Day Incremental Engineering Roadmap (Month 3, Days 61–75)

- [x] **Day 61:** Initialize `rustcc` compiler crate, span error diagnostics, and 15-day roadmap
- [x] **Day 62:** Implement hand-written C tokenizer with source spans, keywords, and multi-char operators (`src/lexer.rs`)
- [ ] **Day 63:** Define C abstract syntax tree nodes for types, expressions, statements, and functions (`src/ast.rs`)
- [ ] **Day 64:** Implement recursive-descent statement parser and Pratt operator-precedence expression parser (`src/parser.rs`)
- [ ] **Day 65:** Implement lexical scope resolver, symbol table, and static type checker (`src/sema.rs`)
- [ ] **Day 66:** Lower typed AST into Three-Address Code (TAC) intermediate representation (`src/ir.rs`)
- [ ] **Day 67:** Implement IR constant folding, algebraic simplification, and dead-code elimination (`src/opt.rs`)
- [ ] **Day 68:** Implement stack-frame allocator and `x86_64` assembly generator (`src/codegen.rs`)
- [ ] **Day 69:** Add native Apple Silicon `aarch64` assembly backend for macOS Mach-O execution
- [ ] **Day 70:** Compile `if`/`else` branches, `while`/`for` loops, and short-circuit boolean logic
- [ ] **Day 71:** Implement ABI-compliant function calls, recursion, and register saving
- [ ] **Day 72:** Support address-of (`&`), dereference (`*`), and local stack pointer arithmetic
- [ ] **Day 73:** Add end-to-end execution test suite compiling C programs to native Mach-O binaries
- [ ] **Day 74:** Build `rustcc` driver CLI (`--dump-tokens`, `--dump-ast`, `--dump-ir`, `-S`, `-o`)
- [ ] **Day 75:** Complete compiler architecture guide, IR/ASM pipeline diagrams, and benchmark suite
