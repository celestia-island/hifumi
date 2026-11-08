//! `hifumi export-ts` 集成测试：金样输出、拒绝口径、node 冒烟（宿主）。

use std::path::PathBuf;
use std::process::Command;

const FIXTURE: &str = "tests/golden/ts_runner.rs";
const GOLDEN: &str = "tests/golden/ts_runner.ts";

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_hifumi"))
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hifumi-ts-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_export(source: &str, tag: &str) -> (bool, String, String) {
    let dir = tmp_dir(tag);
    let src = dir.join("input.rs");
    std::fs::write(&src, source).unwrap();
    let out = bin()
        .args(["export-ts", "--file", src.to_str().unwrap()])
        .output()
        .expect("failed to run hifumi CLI");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// 金样：CLI 对夹具的输出与 golden 文件逐字节一致。
#[test]
fn golden_runner_matches() {
    let out = bin()
        .args(["export-ts", "--file", FIXTURE])
        .output()
        .expect("failed to run hifumi CLI");
    assert!(
        out.status.success(),
        "export-ts failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let golden = std::fs::read(GOLDEN).expect("golden file missing; regenerate via the CLI");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&golden),
        "generated runner diverged from the golden file"
    );
}

/// `--out` 落盘路径与 stdout 路径产出一致。
#[test]
fn out_file_matches_stdout() {
    let dir = tmp_dir("out");
    let path = dir.join("runner.ts");
    let status = bin()
        .args([
            "export-ts",
            "--file",
            FIXTURE,
            "--out",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run hifumi CLI");
    assert!(status.status.success());
    assert!(path.exists());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        std::fs::read_to_string(GOLDEN).unwrap()
    );
}

/// 拒绝口径：不可导出形态必须**列名失败**，绝不静默跳过。
#[test]
fn refuses_non_exportable_forms() {
    let head = r#"#[version("0.2")]
#[migration("0.1" => "0.2" {"#;
    let tail = "})]\npub struct S { pub a: u32 }";
    let cases = [
        (
            format!("{head}\n    + note: String {{ String::new() }},\n{tail}"),
            &["S", "0.1", "0.2", "converter"][..],
        ),
        (
            format!("{head}\n    + (a: u32, b: u32) => c: u32 {{ a + b }},\n{tail}"),
            &["S", "multi-source"],
        ),
        (
            format!("{head}\n    (a: u32, b: u32) => c: u32 {{ a + b }},\n{tail}"),
            &["S", "multi-source"],
        ),
        (
            format!("{head}\n    n: u32 => m: i64,\n{tail}"),
            &["S", "crosses types"],
        ),
        (
            format!("{head}\n    + n: u32 => m: i64,\n{tail}"),
            &["S", "crosses types"],
        ),
        (
            format!("{head}\n    + path: std::path::PathBuf,\n{tail}"),
            &["S", "cannot express"],
        ),
        (
            format!("{head}\n    title: String => name: String {{ __old.title.clone() }},\n{tail}"),
            &["S", "converter"],
        ),
        (
            format!("{head}\n    a: u32 => a: u32,\n{tail}"),
            &["S", "renames a field to itself"],
        ),
        (
            format!("{head}\n    a: u32 => u32,\n{tail}"),
            &["S", "renames a field to itself"],
        ),
    ];
    for (i, (source, needles)) in cases.iter().enumerate() {
        let (ok, _stdout, stderr) = run_export(source, &format!("refuse{i}"));
        assert!(!ok, "case {i} unexpectedly succeeded");
        for needle in *needles {
            assert!(
                stderr.contains(needle),
                "case {i} stderr {stderr:?} missing {needle:?}"
            );
        }
    }
}

/// 链一致性：灭绝步里的不可导出 op 同样拒绝（无静默跳过）；
/// baseline 合法性、循环、重复版本沿用 compact 口径。
#[test]
fn chain_guards() {
    let (ok, _, stderr) = run_export(
        r#"#[version("0.3", baseline = "0.2")]
#[migration("0.1" => "0.2" { + bad: std::path::PathBuf, })]
#[migration("0.2" => "0.3" { + ok: u32, })]
pub struct S { pub a: u32 }"#,
        "extinct-refuse",
    );
    assert!(!ok);
    assert!(stderr.contains("cannot express"), "stderr: {stderr}");

    let (ok, _, stderr) = run_export(
        r#"#[version("0.2", baseline = "9.9")]
#[migration("0.1" => "0.2" { + ok: u32, })]
pub struct S { pub a: u32 }"#,
        "baseline-out",
    );
    assert!(!ok);
    assert!(stderr.contains("not a version"), "stderr: {stderr}");

    let (ok, _, stderr) = run_export(
        r#"#[version("0.2")]
#[migration("0.2" => "0.1" { + a: u32, })]
#[migration("0.1" => "0.2" { + b: u32, })]
pub struct S { pub a: u32 }"#,
        "cycle",
    );
    assert!(!ok);
    assert!(stderr.contains("cycle"), "stderr: {stderr}");

    // 无 #[version] 结构体的文件：空导出而非空输出
    let (ok, _, stderr) = run_export("pub struct Plain { pub a: u32 }", "plain");
    assert!(!ok);
    assert!(
        stderr.contains("no `#[version]` structs"),
        "stderr: {stderr}"
    );

    // `#[versioned_…]` 同前缀属性不得被误吞成版本块
    let (ok, _, stderr) = run_export(
        "#[versioned_custom]\nstruct Almost { pub a: u32 }",
        "versioned-attr",
    );
    assert!(!ok);
    assert!(
        stderr.contains("no `#[version]` structs"),
        "prefixed attr must be ignored, stderr: {stderr}"
    );
}

/// 扫描器韧性：注释形态不得让类型静默丢失或产出幻影类型
/// （块注释夹在属性与结构体之间 / 文档注释提及 `struct X` /
/// 注释里出现未配平的 `#[`）。
#[test]
fn scanner_survives_comment_shapes() {
    // F2：`/* */` 块注释夹在属性与 struct 之间——结构体仍被认领
    let (ok, out, stderr) = run_export(
        r#"#[version("0.2")]
/* 块注释夹层 */
pub struct Blocky { pub a: u32 }"#,
        "block-comment-between",
    );
    assert!(ok, "block comment must not drop the struct: {stderr}");
    assert!(out.contains(r#""Blocky""#), "type must be exported: {out}");

    // F3：文档注释提及 `struct Doc`（同名）——不得重复 bail、只导出一个
    let (ok, out, stderr) = run_export(
        r#"#[version("0.2")]
/// struct Doc is the stable shape.
pub struct Doc { pub a: u32 }"#,
        "doc-mention-same-name",
    );
    assert!(ok, "doc-comment mention must not bail: {stderr}");
    assert_eq!(out.matches(r#""Doc""#).count(), 1, "exactly one Doc: {out}");

    // F3 变体：异名幽灵命中不得认领真链、产出幻影类型
    let (ok, out, stderr) = run_export(
        r#"#[version("0.2")]
/// legacy struct DocV1 kept for reference.
pub struct Doc { pub a: u32 }"#,
        "doc-mention-other-name",
    );
    assert!(ok, "phantom must not poison export: {stderr}");
    assert!(out.contains(r#""Doc""#));
    assert!(!out.contains("DocV1"), "no phantom type: {out}");

    // F5：注释里未配平的 `#[` 不得炸掉候选扫描（真实结构体照常导出）
    let (ok, out, stderr) = run_export(
        "// TODO: 修 #[broken 注释里的残缺属性\n#[version(\"0.2\")]\npub struct Tail { pub a: u32 }",
        "stray-hash-in-comment",
    );
    assert!(ok, "stray #[ in comment must not poison scan: {stderr}");
    assert!(out.contains(r#""Tail""#), "type must be exported: {out}");

    // F5 变体：字符串字面量里的 `#[version(` 也不算属性
    let (ok, out, stderr) = run_export(
        r##"const NOTE: &str = "#[version("0.1")] decorative";
#[version("0.2")]
pub struct Quoted { pub a: u32 }"##,
        "hash-in-string",
    );
    assert!(ok, "string-literal #[ must not count: {stderr}");
    assert!(out.contains(r#""Quoted""#));
    assert!(out.contains(r#""0.2""#));
    assert!(
        !out.contains(r#""0.1""#),
        "decorative version must not leak"
    );

    // 嵌套块注释夹层（Rust 块注释可嵌套）：rfind(`/*`) 会误中内层，
    // 静默丢结构体——区间查表必须跳到最外层起点
    let (ok, out, stderr) = run_export(
        r#"#[version("0.2")]
/* outer /* nested */ still outer */
pub struct Nesty { pub a: u32 }"#,
        "nested-block-comment",
    );
    assert!(
        ok,
        "nested block comment must not drop the struct: {stderr}"
    );
    assert!(out.contains(r#""Nesty""#), "type must be exported: {out}");

    // 跨行 raw string 里的假 DSL：不得产出幻影类型 / 泄漏版本号
    let (ok, out, stderr) = run_export(
        r##"const EMBED: &str = r#"
#[version("9.9")]
struct Sample { pub a: u32 }
"#;
#[version("0.2")]
pub struct Real { pub a: u32 }"##,
        "raw-string-embed",
    );
    assert!(ok, "raw string must not poison scan: {stderr}");
    assert!(out.contains(r#""Real""#), "real type must export: {out}");
    assert!(!out.contains("Sample"), "no phantom type: {out}");
    assert!(
        !out.contains("9.9"),
        "embedded version must not leak: {out}"
    );

    // 字节 raw string `br#"`（跨行）：同样不得产出幻影类型
    let (ok, out, stderr) = run_export(
        r##"const FIXTURE: &[u8] = br#"
#[version("9.9")]
struct Sample { pub a: u32 }
"#;
#[version("0.2")]
pub struct Real { pub a: u32 }"##,
        "byte-raw-string-embed",
    );
    assert!(ok, "br# string must not poison scan: {stderr}");
    assert!(out.contains(r#""Real""#), "real type must export: {out}");
    assert!(!out.contains("Sample"), "no phantom type: {out}");
    assert!(
        !out.contains("9.9"),
        "embedded version must not leak: {out}"
    );

    // 转义引号 + 字符串里的 `/*`：错误配对会让假块注释吞掉后续真结构体
    let (ok, out, stderr) = run_export(
        r#"const PATH: &str = "a\"/*b";
#[version("0.2")]
pub struct Esc { pub a: u32 }"#,
        "escaped-quote",
    );
    assert!(ok, "escaped quote must not fake a block comment: {stderr}");
    assert!(out.contains(r#""Esc""#), "type must be exported: {out}");

    // 孤立引号（char 字面量）：行内无闭引号，只跳 1 字节，绝不吞全文
    let (ok, out, stderr) = run_export(
        r#"const QUOTE: char = '"';
#[version("0.2")]
pub struct Orphan { pub a: u32 }"#,
        "orphan-quote",
    );
    assert!(ok, "orphan quote must not swallow the file: {stderr}");
    assert!(out.contains(r#""Orphan""#), "type must be exported: {out}");
}

/// node 冒烟：宿主 node（≥ 22.6，直接 type-strip）执行生成 runner 的
/// 前向语义 + 降级 / 未知类型 / 缺源护栏。沙箱镜像无 node，故 `#[ignore]`。
#[test]
#[ignore = "host-only: sandbox image has no node; run with --ignored on the host"]
fn node_smoke() {
    let dir = tmp_dir("smoke");
    let runner = dir.join("runner.ts");
    let status = bin()
        .args([
            "export-ts",
            "--file",
            FIXTURE,
            "--out",
            runner.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run hifumi CLI");
    assert!(status.status.success());

    let driver = dir.join("driver.ts");
    std::fs::write(
        &driver,
        r#"import { migrate } from "./runner.ts";

const eq = (a: unknown, b: unknown, msg: string) => {
    const sa = JSON.stringify(a), sb = JSON.stringify(b);
    if (sa !== sb) throw new Error(`${msg}: got ${sa}, want ${sb}`);
};

// 0.2 => 0.3：add / remove / rename(back 括号不影响前向)。
// 有效的 0.2 文档字段 = 宏逆向推断集 {content, heading, dead, alias, label}
// ——`label` 由 copy 步的逆向推断产生。
const v02 = { content: "c", heading: "h", dead: "d", alias: "a", label: "L" };
const v03 = migrate("Doc", "0.2", "0.3", v02);
eq(v03.pinned, false, "add bool default");
eq(v03.dead, undefined, "remove drops the field");
eq(v03.label, "a", "rename moves the value");
eq(v03.alias, undefined, "rename consumes the source");
// 快照语义：步内所有 op 读步前数据（Rust 的 __old）。rename 先把
// label 写成 old.alias，copy 仍必须读出步前的 label("L") 而非串读
// 步内中间值——这是 TS runner 与 Rust From 生成物的关键对齐点。
eq(v03.echo, "L", "copy reads the pre-step snapshot, not intra-step writes");
eq(v03.content, "c", "untouched fields survive");

// 灭绝版本不再出现在链里（baseline 截断）
let threw = false;
try { migrate("Doc", "0.1", "0.3", {}); } catch { threw = true; }
if (!threw) throw new Error("extinct version 0.1 must be rejected");

// 降级护栏
threw = false;
try { migrate("Doc", "0.3", "0.2", {}); } catch (e) {
    threw = true;
    if (!String(e).includes("Rust-side only")) throw new Error(`wrong downgrade error: ${e}`);
}
if (!threw) throw new Error("downgrade must throw");

// 未知类型
threw = false;
try { migrate("Nope", "0.1", "0.2", {}); } catch (e) {
    threw = true;
    if (!String(e).includes("unknown type")) throw new Error(`wrong unknown-type error: ${e}`);
}
if (!threw) throw new Error("unknown type must throw");

// 缺 rename 源：输入从未能按该版本解析
threw = false;
try { migrate("Doc", "0.2", "0.3", { content: "c", heading: "h" }); } catch (e) {
    threw = true;
    if (!String(e).includes("missing")) throw new Error(`wrong missing-source error: ${e}`);
}
if (!threw) throw new Error("missing rename source must throw");

// 零步类型：from == to 原样返回
const solo = migrate("Solo", "0.9", "0.9", { id: 7 });
eq(solo.id, 7, "no-op migration returns the value");

console.log("SMOKE_OK");
"#,
    )
    .unwrap();

    let out = Command::new("node")
        .arg(driver.to_str().unwrap())
        .output()
        .expect("failed to run node; this test is host-only");
    assert!(
        out.status.success(),
        "node driver failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("SMOKE_OK"));
}
