use std::fs;
use std::process::Command;

use rustcc::codegen::TargetArch;
use rustcc::compile_to_asm;
use tempfile::tempdir;

fn compile_and_run(source: &str) -> i32 {
    let dir = tempdir().unwrap();
    let asm_path = dir.path().join("prog.s");
    let bin_path = dir.path().join("prog.out");

    let asm = compile_to_asm(source, TargetArch::host(), true).expect("compile failed");
    fs::write(&asm_path, asm).unwrap();

    let status = Command::new("cc")
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .status()
        .expect("failed to invoke system C compiler/linker");
    assert!(status.success(), "linking failed");

    Command::new(&bin_path)
        .status()
        .expect("failed to run binary")
        .code()
        .unwrap_or(-1)
}

#[test]
fn test_abi_six_argument_function_call() {
    let src = r#"
        int weighted_sum(int a, int b, int c, int d, int e, int f) {
            return a * 1 + b * 2 + c * 3 + d * 4 + e * 5 + f * 6;
        }

        int main(void) {
            // 1 + 4 + 9 + 16 + 25 + 36 = 91
            return weighted_sum(1, 2, 3, 4, 5, 6);
        }
    "#;
    assert_eq!(compile_and_run(src), 91);
}

#[test]
fn test_recursive_fibonacci_and_euclid_gcd() {
    let src = r#"
        int fib(int n) {
            if (n <= 1) {
                return n;
            }
            return fib(n - 1) + fib(n - 2);
        }

        int gcd(int a, int b) {
            if (b == 0) {
                return a;
            }
            return gcd(b, a % b);
        }

        int main(void) {
            // fib(10) = 55, gcd(1071, 462) = 21 -> 55 + 21 = 76
            return fib(10) + gcd(1071, 462);
        }
    "#;
    assert_eq!(compile_and_run(src), 76);
}

#[test]
fn test_mutual_recursion_even_odd() {
    let src = r#"
        int is_even(int n) {
            if (n == 0) return 1;
            return is_odd(n - 1);
        }

        int is_odd(int n) {
            if (n == 0) return 0;
            return is_even(n - 1);
        }

        int main(void) {
            return is_even(14) * 10 + is_odd(14);
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}
