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
        .expect("failed to invoke cc");
    assert!(status.success());

    Command::new(&bin_path)
        .status()
        .expect("failed to execute binary")
        .code()
        .unwrap_or(-1)
}

#[test]
fn test_pointer_swap_and_deref_mutation() {
    let src = r#"
        void swap(int *a, int *b) {
            int tmp = *a;
            *a = *b;
            *b = tmp;
            return;
        }

        int main(void) {
            int x = 15;
            int y = 80;
            swap(&x, &y);
            // x is now 80, y is now 15 -> 80 - 15 = 65
            return x - y;
        }
    "#;
    assert_eq!(compile_and_run(src), 65);
}

#[test]
fn test_double_pointer_indirection_and_aliasing() {
    let src = r#"
        int main(void) {
            int val = 10;
            int *p = &val;
            int **pp = &p;
            **pp = **pp + 27;
            return val;
        }
    "#;
    assert_eq!(compile_and_run(src), 37);
}
