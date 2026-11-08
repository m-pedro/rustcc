use std::fs;
use std::process::Command;

use rustcc::codegen::TargetArch;
use rustcc::compile_to_asm;
use tempfile::tempdir;

fn run_with_opt(source: &str, opt: bool) -> i32 {
    let dir = tempdir().unwrap();
    let asm_path = dir.path().join("prog.s");
    let bin_path = dir.path().join("prog.out");

    let asm = compile_to_asm(source, TargetArch::host(), opt).expect("compile failed");
    fs::write(&asm_path, asm).unwrap();

    let status = Command::new("cc")
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .status()
        .expect("cc failed");
    assert!(status.success());

    Command::new(&bin_path)
        .status()
        .unwrap()
        .code()
        .unwrap_or(-1)
}

#[test]
fn test_e2e_collatz_conjecture_steps() {
    let src = r#"
        int collatz_steps(int n) {
            int steps = 0;
            while (n > 1) {
                if (n % 2 == 0) {
                    n = n / 2;
                } else {
                    n = 3 * n + 1;
                }
                steps = steps + 1;
            }
            return steps;
        }

        int main(void) {
            // Collatz sequence for 27 takes 111 steps to reach 1
            return collatz_steps(27);
        }
    "#;
    assert_eq!(run_with_opt(src, false), 111);
    assert_eq!(run_with_opt(src, true), 111);
}

#[test]
fn test_e2e_prime_counting_trial_division() {
    let src = r#"
        int is_prime(int n) {
            if (n < 2) return 0;
            for (int d = 2; d * d <= n; d = d + 1) {
                if (n % d == 0) return 0;
            }
            return 1;
        }

        int main(void) {
            int count = 0;
            for (int i = 2; i <= 100; i = i + 1) {
                count = count + is_prime(i);
            }
            // There are 25 primes <= 100
            return count;
        }
    "#;
    assert_eq!(run_with_opt(src, true), 25);
}

#[test]
fn test_e2e_modular_exponentiation() {
    let src = r#"
        int mod_pow(int base, int exp, int m) {
            int result = 1;
            base = base % m;
            while (exp > 0) {
                if (exp % 2 == 1) {
                    result = (result * base) % m;
                }
                exp = exp / 2;
                base = (base * base) % m;
            }
            return result;
        }

        int main(void) {
            // 3^13 mod 97 = 1594323 - (97 * 16436) = 31
            return mod_pow(3, 13, 97);
        }
    "#;
    assert_eq!(run_with_opt(src, true), 31);
}
