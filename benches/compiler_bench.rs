use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rustcc::codegen::TargetArch;
use rustcc::compile_to_asm;

const BENCH_C_SOURCE: &str = r#"
    int fib(int n) {
        if (n <= 1) return n;
        return fib(n - 1) + fib(n - 2);
    }

    int collatz(int n) {
        int steps = 0;
        while (n > 1) {
            if (n % 2 == 0) {
                n = n / 2;
            } else {
                n = (3 * n + 1) + (0 * 99);
            }
            steps = steps + 1;
        }
        return steps;
    }

    int main(void) {
        int a = fib(10);
        int b = collatz(27);
        return a + b;
    }
"#;

fn bench_full_compilation_pipeline(c: &mut Criterion) {
    c.bench_function("rustcc_end_to_end_aarch64_opt", |b| {
        b.iter(|| {
            let asm = compile_to_asm(black_box(BENCH_C_SOURCE), TargetArch::Aarch64, true).unwrap();
            black_box(asm);
        })
    });
}

criterion_group!(benches, bench_full_compilation_pipeline);
criterion_main!(benches);
