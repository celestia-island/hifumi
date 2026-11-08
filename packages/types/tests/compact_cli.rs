//! `hifumi compact` CLI：源级链压缩的端到端（子进程）测试。
//!
//! 契约：删除比 baseline 更老的 `#[migration]` 块、在 `#[version]`
//! 写入 `baseline = "Y"`、其余源文本逐字节保留（文本手术，不重排）。

use std::process::Command;

const FIXTURE: &str = r#"use hifumi::version;

#[version("0.3")]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.1" => "0.2" {
    + extra: i32,
})]
#[migration("0.2" => "0.3" {
    + note: String,
})]
struct Doc {
    extra: i32,
    note: String,
}

fn main() {
    println!("untouched");
}
"#;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_hifumi"))
}

fn tmp_fixture(tag: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("hifumi-compact-{}-{tag}.rs", std::process::id()));
    std::fs::write(&path, FIXTURE).unwrap();
    path
}

/// --write 原地重写：灭绝步删除、baseline 落进 #[version]、其余不动。
#[test]
fn compact_rewrites_in_place() {
    let path = tmp_fixture("write");
    let out = bin()
        .args([
            "compact",
            "--file",
            path.to_str().unwrap(),
            "--struct-name",
            "Doc",
            "--baseline",
            "0.2",
            "--write",
        ])
        .output()
        .expect("hifumi binary runs");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let rewritten = std::fs::read_to_string(&path).unwrap();
    assert!(
        rewritten.contains(r#"#[version("0.3", baseline = "0.2")]"#),
        "baseline must land in #[version]:\n{rewritten}"
    );
    assert!(
        !rewritten.contains(r#""0.1" => "0.2""#),
        "extinct step must be deleted:\n{rewritten}"
    );
    assert!(
        rewritten.contains(r#""0.2" => "0.3""#),
        "retained step must survive:\n{rewritten}"
    );
    // 格式保全：不相关行逐字节保留
    assert!(rewritten.contains("use hifumi::version;"));
    assert!(rewritten.contains(r#"println!("untouched");"#));
    assert!(rewritten.contains("+ note: String,"));
    // 删除不留空行残骸：行数差 = 灭绝步的 3 行，一个不多
    let removed_lines = FIXTURE.lines().count() - rewritten.lines().count();
    assert_eq!(removed_lines, 3, "exactly the 3-line extinct block goes");
    std::fs::remove_file(&path).ok();
}

/// 默认模式：结果打到 stdout，源文件不动。
#[test]
fn compact_prints_without_write() {
    let path = tmp_fixture("print");
    let out = bin()
        .args([
            "compact",
            "--file",
            path.to_str().unwrap(),
            "--struct-name",
            "Doc",
            "--baseline",
            "0.2",
        ])
        .output()
        .expect("hifumi binary runs");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(r#"#[version("0.3", baseline = "0.2")]"#));
    assert!(!stdout.contains(r#""0.1" => "0.2""#));

    // 源文件原样
    assert_eq!(std::fs::read_to_string(&path).unwrap(), FIXTURE);
    std::fs::remove_file(&path).ok();
}

/// baseline == 当前版本 = 整链灭绝：所有步删除，只剩 final。
#[test]
fn compact_baseline_at_final_cuts_whole_chain() {
    let path = tmp_fixture("final");
    let out = bin()
        .args([
            "compact",
            "--file",
            path.to_str().unwrap(),
            "--struct-name",
            "Doc",
            "--baseline",
            "0.3",
            "--write",
        ])
        .output()
        .expect("hifumi binary runs");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let rewritten = std::fs::read_to_string(&path).unwrap();
    assert!(
        rewritten.contains(r#"#[version("0.3", baseline = "0.3")]"#),
        "baseline == final must land:\n{rewritten}"
    );
    assert!(!rewritten.contains(r#""0.1" => "0.2""#), "{rewritten}");
    assert!(!rewritten.contains(r#""0.2" => "0.3""#), "{rewritten}");
    // 两条步（3 行/条）全删，一个不多
    let removed_lines = FIXTURE.lines().count() - rewritten.lines().count();
    assert_eq!(removed_lines, 6, "{rewritten}");
    std::fs::remove_file(&path).ok();
}

/// baseline 不在链上 → 非零退出 + 诊断（拼错版本号必须在写入前被拦下）。
#[test]
fn compact_rejects_unknown_baseline() {
    let path = tmp_fixture("bad");
    let out = bin()
        .args([
            "compact",
            "--file",
            path.to_str().unwrap(),
            "--struct-name",
            "Doc",
            "--baseline",
            "0.9",
            "--write",
        ])
        .output()
        .expect("hifumi binary runs");
    assert!(!out.status.success(), "unknown baseline must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("0.9"), "{stderr}");
    // 文件不得被写入
    assert_eq!(std::fs::read_to_string(&path).unwrap(), FIXTURE);
    std::fs::remove_file(&path).ok();
}

/// 目标结构体不存在 → 非零退出；同名子串（DocX）不算命中。
#[test]
fn compact_rejects_missing_struct() {
    let path = tmp_fixture("missing");
    let out = bin()
        .args([
            "compact",
            "--file",
            path.to_str().unwrap(),
            "--struct-name",
            "DocX",
            "--baseline",
            "0.2",
        ])
        .output()
        .expect("hifumi binary runs");
    assert!(!out.status.success());
    std::fs::remove_file(&path).ok();
}

/// 合法尾逗号（`#[version("0.3",)]`——宏侧解析器接受的形式）不得
/// 产出 `,,`：注入后必须仍是可编译的属性文本（轮 2 审查 S1 回归）。
#[test]
fn compact_handles_trailing_comma_in_version_attr() {
    let fixture = FIXTURE.replace(r#"#[version("0.3")]"#, r#"#[version("0.3",)]"#);
    let path = std::env::temp_dir().join(format!("hifumi-compact-{}-comma.rs", std::process::id()));
    std::fs::write(&path, &fixture).unwrap();
    let out = bin()
        .args([
            "compact",
            "--file",
            path.to_str().unwrap(),
            "--struct-name",
            "Doc",
            "--baseline",
            "0.2",
            "--write",
        ])
        .output()
        .expect("hifumi binary runs");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let rewritten = std::fs::read_to_string(&path).unwrap();
    assert!(
        rewritten.contains(r#"#[version("0.3", baseline = "0.2")]"#),
        "trailing comma must be consumed, not doubled:\n{rewritten}"
    );
    assert!(!rewritten.contains(",,"));
    std::fs::remove_file(&path).ok();
}

/// 双重压缩：已带 baseline 的文件再次 compact → 非零退出，不重写。
#[test]
fn compact_rejects_double_compaction() {
    let path = tmp_fixture("twice");
    let args = [
        "compact",
        "--file",
        path.to_str().unwrap(),
        "--struct-name",
        "Doc",
        "--baseline",
        "0.2",
        "--write",
    ];
    let first = bin().args(args).output().expect("hifumi binary runs");
    assert!(first.status.success());
    let compacted = std::fs::read_to_string(&path).unwrap();

    let second = bin().args(args).output().expect("hifumi binary runs");
    assert!(
        !second.status.success(),
        "compacting an already-compacted file must fail"
    );
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(stderr.contains("baseline"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        compacted,
        "failed re-run must not touch the file"
    );
    std::fs::remove_file(&path).ok();
}

/// `#[migration]` 排在 `#[version]` 之前 → 非零退出，文件不动（这类
/// 源码本就编译不过；文本手术不应产出「看似成功」的结果）。
#[test]
fn compact_rejects_migration_before_version() {
    let fixture = FIXTURE.replace(
        r#"#[version("0.3")]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.1" => "0.2" {
    + extra: i32,
})]"#,
        r#"#[derive(Debug, Clone, PartialEq)]
#[migration("0.1" => "0.2" {
    + extra: i32,
})]
#[version("0.3")]"#,
    );
    let path = std::env::temp_dir().join(format!("hifumi-compact-{}-order.rs", std::process::id()));
    std::fs::write(&path, &fixture).unwrap();
    let out = bin()
        .args([
            "compact",
            "--file",
            path.to_str().unwrap(),
            "--struct-name",
            "Doc",
            "--baseline",
            "0.2",
            "--write",
        ])
        .output()
        .expect("hifumi binary runs");
    assert!(
        !out.status.success(),
        "migration-before-version must be rejected"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("before"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        fixture,
        "rejected input must not be written"
    );
    std::fs::remove_file(&path).ok();
}
