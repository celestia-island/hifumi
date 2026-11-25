//! 三条「不支持就报错」的拒绝，钉的是**诊断形态**，不是「没编译过」。
//!
//! `#[version]` 对三类字段属性/字段形态选择显式拒绝而不是静默丢弃：
//! 字段上的 `#[cfg]`、`#[serde(flatten)]`、同名重复字段。它们都是编译期
//! 行为——只用 `trybuild` 式的「这段代码编译失败了吗」钉不住它们：
//! 把拒绝删掉，`a: i32, a: i32` 仍会因为生成物里的 E0124 编译失败，
//! `#[cfg]` 也可能因为别的理由失败，测试照样绿。
//!
//! 所以这里要求三件事同时成立：
//! 1. 编译**失败**；
//! 2. 失败里带着宏自己的那句诊断（删掉拒绝 → 换成 rustc 的报错 → 红）；
//! 3. `-->` 指在**字段自己**的行列上，而不是 `#[version]` 或 panic。
//!
//! 锚点是写死的绝对行列（见 [`Case::anchor`]）：夹具挪行会让测试红，
//! 逼你确认 span 是否仍然落在被拒的那个字段上。
//!
//! 驱动形式说明：本仓没有 `trybuild`（共享 registry 里没有该 crate，容器
//! 车道是 `--network none`，取不回来），故用 `std::process::Command` 自己
//! 驱动 `cargo build`。这正是 `trybuild` 内部做的同一件事，只是断言更硬
//! （钉到 span，而不只是「某个 stderr 子串」）。

use std::path::PathBuf;
use std::process::Command;

/// 一个拒绝用例：源码、期望的诊断片段、被拒绝那行字段的标识符。
struct Case {
    /// 出现在测试名 / crate 名里的标签。
    label: &'static str,
    /// 完整源码（`main` 由夹具补齐，这里只给结构体）。
    body: &'static str,
    /// 宏自己写的那句诊断开头。
    expected: &'static str,
    /// 诊断必须**不**含的片段（用来排除「用别的方式碰巧编译失败了」）。
    forbidden: &'static str,
    /// 诊断必须指在的绝对行列（夹具 `src/lib.rs` 内，1 起）。
    ///
    /// 写死而不是从源码现算：这是**行为**的一部分（用户在哪儿看到红），
    /// 现算会把夹具改动顺手变成断言改动。夹具行数变了这里会红，那正是
    /// 提醒你确认 span 是否仍然落在字段上。
    anchor: &'static str,
}

/// 宏的拒绝消息与 `migration.rs` / `current_version_struct.rs` 逐字一致。
const CASES: &[Case] = &[
    Case {
        label: "cfg",
        body: r#"#[version("0.2")]
#[derive(Debug, Clone, PartialEq)]
struct Conf {
    a: i32,
    #[cfg(unix)]
    b: i32,
}
"#,
        expected: "`#[cfg]` on a field of a #[version] struct is not supported",
        forbidden: "panicked",
        anchor: "src/lib.rs:8:5",
    },
    Case {
        label: "flatten",
        body: r#"#[version("0.2")]
#[derive(Debug, Clone, PartialEq)]
struct Conf {
    a: i32,
    #[serde(flatten)]
    extra: std::collections::BTreeMap<String, i32>,
}
"#,
        expected: "`#[serde(flatten)]` is not supported on a #[version] struct",
        forbidden: "panicked",
        anchor: "src/lib.rs:8:5",
    },
    Case {
        label: "duplicate",
        body: r#"#[version("0.2")]
#[derive(Debug, Clone, PartialEq)]
struct Conf {
    a: i32,
    a: i32,
}
"#,
        expected: "duplicate field `a` on a #[version] struct",
        // 拒绝被删掉后，rustc 自己报的 E0124 **也**落在同一个 `8:5` 上
        // （实测），所以单靠「失败 + 锚点对上」区分不开两件事；真正区分的是
        // 这条反向排除。
        forbidden: "E0124",
        anchor: "src/lib.rs:8:5",
    },
];

fn crate_name(label: &str) -> String {
    format!("hifumi-rejection-{label}-{}", std::process::id())
}

/// 跑一个用例，返回合并后的 stderr。
fn compile(case: &Case) -> (bool, String) {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("rejection-{}", case.label));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).expect("create fixture crate");

    let name = crate_name(case.label);
    // 依赖用绝对路径指回本仓：夹具 crate 不参与 workspace，故不会被
    // `cargo test --workspace` 当成成员；`offline` 保证不触网。
    let types_dir = env!("CARGO_MANIFEST_DIR");
    let manifest = format!(
        r#"[package]
name = "{name}"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
hifumi = {{ path = "{types_dir}" }}
"#
    );
    std::fs::write(dir.join("Cargo.toml"), manifest).expect("write fixture manifest");
    // `$version` 属性只允许在库/程序里出现；夹具本体写成 lib。
    std::fs::write(
        dir.join("src/lib.rs"),
        format!(
            "#![allow(dead_code)]\nuse hifumi::version;\n\n{}\n",
            case.body
        ),
    )
    .expect("write fixture source");

    let output = Command::new(env!("CARGO"))
        .args(["build", "--offline", "--quiet"])
        .current_dir(&dir)
        .env("CARGO_TARGET_DIR", dir.join("target"))
        .output()
        .expect("spawn cargo");

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output.status.success(), stderr)
}

/// 三条拒绝的共同断言：编译失败、诊断是宏自己的、并落在字段 span 上。
fn assert_rejected(case: &Case) {
    let (succeeded, stderr) = compile(case);

    assert!(
        !succeeded,
        "[{}] expected this to fail to compile, but it succeeded:\n{}",
        case.label, stderr
    );
    assert!(
        stderr.contains(case.expected),
        "[{}] expected the macro's own diagnostic {:?}, got:\n{}",
        case.label,
        case.expected,
        stderr
    );
    // 反面对照：不许靠「别的东西碰巧失败了」蒙混过关。
    assert!(
        !stderr.contains(case.forbidden),
        "[{}] diagnostic must not be {:?} (that means the rejection did not fire):\n{}",
        case.label,
        case.forbidden,
        stderr
    );

    // 诊断必须指在字段自己的行列上——不是 `#[version]` 那一行，也不是
    // `custom attribute panicked` 指回的属性。rustc 把带属性的字段的
    // span 落在第一行属性上，故三条用例的锚点都是属性行。
    let anchor = format!("--> {}", case.anchor);
    assert!(
        stderr.contains(&anchor),
        "[{}] expected the diagnostic at {:?}, got:\n{}",
        case.label,
        anchor,
        stderr
    );
}

/// 三条拒绝必须**串行**跑。
///
/// 每个用例都起一次嵌套的 `cargo build`（rustc + linker + 若干辅助线程）。
/// 分成三个 `#[test]` 时它们会被测试框架并发拉起，在容器车道的
/// `--pids-limit 512` 下把进程/线程配额耗尽——实测过
/// `collect2: fatal error: posix_spawnp: Resource temporarily unavailable`
/// 与 `failed to spawn helper thread ... WouldBlock`，三次里红一次。
/// 测试内部控制不了 `--test-threads`，所以串行放在这里：一个测试、一个循环。
#[test]
fn unsupported_field_forms_are_rejected_on_the_field_span() {
    for case in CASES {
        assert_rejected(case);
    }
}
