//! TypeScript 迁移 runner 生成（`hifumi export-ts` 的库侧）。
//!
//! 目标：让非 Rust 消费端（前端 / 工具链）能把**旧版本 JSON** 升级到当前
//! 版本，而不必链接 hifumi 或把任意 Rust 转换器表达式带过语言边界。
//!
//! 边界（有意为之，见 0.3.0 波次设计）：
//! - **只前向**：降级（`back` 表达式）是任意 Rust 闭包，不出 Rust 边界；
//!   runner 对 `from > to` 显式抛错。
//! - **只结构步**：可导出的 op = 无转换器的 Add（默认值在生成期按 Rust
//!   类型解析成 JSON 字面量）/ Remove / 单源同型纯 Rename / Copy。带
//!   良转换器、多源、跨型 `.into()` 的步由 CLI 侧**拒绝并列名**，绝不静默
//!   跳过——静默跳过等于数据丢失。
//!
//! 生成物是零依赖、可被 node 直接 type-strip 执行的 `.ts`（只用可擦除
//! 语法：type / interface / 注解，不用 enum / namespace / 参数属性）。

use std::fmt::Write as _;

/// 单条前向迁移步中的一个结构操作。
#[derive(Clone, Debug, PartialEq)]
pub enum TsOp {
    /// 新版本新增字段；`default` = 生成期解析出的 Rust `Default` JSON 投影。
    /// 运行期语义与 Rust 侧 `From` 一致：无条件覆写（旧版本 JSON 不会带
    /// 该字段，残键也不该存活到新版本）。
    Add { field: String, default: Json },
    /// 新版本删除字段。
    Remove { field: String },
    /// 单源同型纯改名：值搬运，源字段消失。
    Rename { from: String, to: String },
    /// 单源同型复制：值搬运，源字段保留。
    Copy { from: String, to: String },
}

/// `Add` 步的 JSON 值骨架。
///
/// 只表达「Rust `Default` 能无损投影到 JSON」的形态；其它类型的默认值
/// （自定义结构体、`PathBuf` 之类）在 CLI 侧就被拒绝，走不进这里。
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr,
    Obj,
}

/// 一条前向迁移步：`from` → `to` + 结构操作序列。
#[derive(Clone, Debug, PartialEq)]
pub struct TsStep {
    pub from: String,
    pub to: String,
    pub ops: Vec<TsOp>,
}

/// 一个 `#[version]` 类型的导出规格。
#[derive(Clone, Debug, PartialEq)]
pub struct TsTypeSpec {
    /// 类型名（TS 侧的链查找键）。
    pub name: String,
    /// 版本链，旧 → 新，**已按 baseline 截断**（灭绝版本不出现在 runner）。
    pub versions: Vec<String>,
    /// 压缩底线（`#[version(..., baseline = "Y")]`）；未压缩 = `None`。
    pub baseline: Option<String>,
    /// 保留步（旧 → 新排序），与 `versions` 相邻对一一对应（可缺步：
    /// 无 op 的版本跳跃在 DSL 里本就是空 `{}` 或省略块）。
    pub steps: Vec<TsStep>,
}

impl TsOp {
    /// 生成期渲染为 runner 表里的 op 对象字面量（不带缩进）。
    fn render(&self) -> String {
        match self {
            TsOp::Add { field, default } => {
                format!(
                    "{{ kind: \"add\", field: {}, value: {} }}",
                    quote_ts(field),
                    default.render()
                )
            }
            TsOp::Remove { field } => {
                format!("{{ kind: \"remove\", field: {} }}", quote_ts(field))
            }
            TsOp::Rename { from, to } => format!(
                "{{ kind: \"rename\", from: {}, to: {} }}",
                quote_ts(from),
                quote_ts(to)
            ),
            TsOp::Copy { from, to } => format!(
                "{{ kind: \"copy\", from: {}, to: {} }}",
                quote_ts(from),
                quote_ts(to)
            ),
        }
    }
}

impl Json {
    fn render(&self) -> String {
        match self {
            Json::Null => "null".to_string(),
            Json::Bool(b) => b.to_string(),
            Json::Int(i) => i.to_string(),
            Json::Float(f) => format!("{f:?}"),
            Json::Str(s) => quote_ts(s),
            Json::Arr => "[]".to_string(),
            Json::Obj => "{}".to_string(),
        }
    }
}

/// TS 双引号字符串字面量（转义 `\`、`"`、控制字符）。
///
/// 名字 / 版本号 / 字段名常态下都是标识符与 semver，转义只是防御性
/// 兜底——规格类型是公开 API，不假设调用方只喂标识符。
fn quote_ts(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 由规格生成自包含的 TS runner 源码。
///
/// 校验内部一致性（重名类型、步引用链外版本），不一致即报错——生成物
/// 宁可不出，不可带错出。
pub fn generate_runner(specs: &[TsTypeSpec]) -> anyhow::Result<String> {
    let mut seen = std::collections::BTreeSet::new();
    for spec in specs {
        if !seen.insert(spec.name.as_str()) {
            anyhow::bail!("duplicate type name in export specs: {}", spec.name);
        }
        if spec.versions.is_empty() {
            anyhow::bail!("type {} has an empty version chain", spec.name);
        }
        for (a, b) in spec.versions.iter().zip(spec.versions.iter().skip(1)) {
            if a == b {
                anyhow::bail!("type {} repeats version {a} in its chain", spec.name);
            }
        }
        for step in &spec.steps {
            let known = |v: &str| spec.versions.iter().any(|x| x == v);
            if !known(&step.from) || !known(&step.to) {
                anyhow::bail!(
                    "type {} has a step {} => {} referencing a version outside its chain {:?}",
                    spec.name,
                    step.from,
                    step.to,
                    spec.versions
                );
            }
        }
    }

    let mut chains = String::new();
    for (idx, spec) in specs.iter().enumerate() {
        if idx > 0 {
            chains.push('\n');
        }
        let _ = writeln!(chains, "    {}: {{", quote_ts(&spec.name));
        let _ = writeln!(
            chains,
            "        versions: [{}],",
            spec.versions
                .iter()
                .map(|v| quote_ts(v))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = writeln!(
            chains,
            "        baseline: {},",
            spec.baseline
                .as_deref()
                .map(quote_ts)
                .unwrap_or_else(|| "null".to_string())
        );
        chains.push_str("        steps: [");
        if spec.steps.is_empty() {
            chains.push_str("],\n");
        } else {
            chains.push('\n');
            for (si, step) in spec.steps.iter().enumerate() {
                let _ = write!(
                    chains,
                    "            {{ from: {}, to: {}, ops: [",
                    quote_ts(&step.from),
                    quote_ts(&step.to)
                );
                if step.ops.is_empty() {
                    chains.push_str("] },");
                } else {
                    chains.push('\n');
                    for op in &step.ops {
                        let _ = writeln!(chains, "                {},", op.render());
                    }
                    chains.push_str("            ] },");
                }
                chains.push('\n');
                let _ = si;
            }
            chains.push_str("        ],\n");
        }
        chains.push_str("    },");
        chains.push('\n');
    }

    let mut out = String::with_capacity(4096 + chains.len());
    out.push_str(RUNNER_HEAD);
    out.push_str(&chains);
    out.push_str("};\n\n");
    out.push_str(RUNNER_MIGRATE);
    Ok(out)
}

const RUNNER_HEAD: &str = r#"// Generated by `hifumi export-ts`. DO NOT EDIT.
// Source of truth: the Rust `#[version]` derive — regenerate instead of patching.

export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

export type Op =
    | { kind: "add"; field: string; value: Json }
    | { kind: "remove"; field: string }
    | { kind: "rename"; from: string; to: string }
    | { kind: "copy"; from: string; to: string };

export interface Step {
    from: string;
    to: string;
    ops: Op[];
}

export interface Chain {
    /** Old -> new, already truncated at the Rust-side baseline. */
    versions: string[];
    baseline: string | null;
    steps: Step[];
}

export const chains: Record<string, Chain> = {
"#;

const RUNNER_MIGRATE: &str = r#"/**
 * Upgrade a legacy version's JSON value to the target version.
 *
 * Semantics mirror the Rust `From` chain: `add` overwrites with the
 * generation-time default, `remove` drops the field, `rename` moves the
 * value (source gone), `copy` duplicates it (source kept). A missing
 * rename/copy source throws — that input never parsed as its claimed
 * version in Rust, so it is malformed here too.
 *
 * Downgrade (`from` newer than `to`) is Rust-side only and always throws.
 */
export function migrate(
    typeName: string,
    from: string,
    to: string,
    value: Record<string, unknown>,
): Record<string, unknown> {
    const chain = chains[typeName];
    if (chain === undefined) {
        throw new Error(
            `hifumi-ts: unknown type ${JSON.stringify(typeName)}; known: ${Object.keys(chains).join(", ")}`,
        );
    }
    const fi = chain.versions.indexOf(from);
    const ti = chain.versions.indexOf(to);
    if (fi === -1 || ti === -1) {
        throw new Error(
            `hifumi-ts: unknown version for ${typeName} (from ${JSON.stringify(from)}, to ${JSON.stringify(to)}); known: ${chain.versions.join(", ")}`,
        );
    }
    if (fi > ti) {
        throw new Error(
            `hifumi-ts: downgrade ${from} -> ${to} for ${typeName} is Rust-side only; the TS runner upgrades legacy data`,
        );
    }
    let v = { ...value };
    for (let i = fi; i < ti; i++) {
        const step = chain.steps.find(
            (s) => s.from === chain.versions[i] && s.to === chain.versions[i + 1],
        );
        if (step === undefined) {
            throw new Error(
                `hifumi-ts: no migration step ${chain.versions[i]} => ${chain.versions[i + 1]} for ${typeName}`,
            );
        }
        // Rust `From` 生成物对一步内的所有 op 都读 `__old`（步前快照），
        // 写入新结构：`b = old.a` 与 `c = old.b` 在 Rust 里互不干扰。
        // TS 侧必须复刻该快照语义——否则「前一个 op 的目标是后一个 op
        // 的源」时会串读步内中间值。写回一律 defineProperty：绕开
        // `__proto__` 的原型 setter，并保证创建自有属性。
        const src = v;
        v = { ...v };
        const has = (k: string) => Object.prototype.hasOwnProperty.call(src, k);
        const set = (k: string, val: unknown) => {
            Object.defineProperty(v, k, {
                value: val,
                enumerable: true,
                writable: true,
                configurable: true,
            });
        };
        for (const op of step.ops) {
            switch (op.kind) {
                case "add":
                    set(op.field, op.value);
                    break;
                case "remove":
                    delete v[op.field];
                    break;
                case "rename":
                case "copy": {
                    if (!has(op.from)) {
                        throw new Error(
                            `hifumi-ts: ${op.kind} source ${JSON.stringify(op.from)} missing while migrating ${typeName} to ${chain.versions[i + 1]}`,
                        );
                    }
                    set(op.to, src[op.from]);
                    if (op.kind === "rename") {
                        delete v[op.from];
                    }
                    break;
                }
            }
        }
    }
    return v;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_spec() -> TsTypeSpec {
        TsTypeSpec {
            name: "Doc".to_string(),
            versions: vec!["0.1".to_string(), "0.2".to_string(), "0.3".to_string()],
            baseline: None,
            steps: vec![
                TsStep {
                    from: "0.1".to_string(),
                    to: "0.2".to_string(),
                    ops: vec![
                        TsOp::Remove {
                            field: "extra".to_string(),
                        },
                        TsOp::Rename {
                            from: "title".to_string(),
                            to: "name".to_string(),
                        },
                    ],
                },
                TsStep {
                    from: "0.2".to_string(),
                    to: "0.3".to_string(),
                    ops: vec![TsOp::Add {
                        field: "note".to_string(),
                        default: Json::Str(String::new()),
                    }],
                },
            ],
        }
    }

    #[test]
    fn renders_chain_table_and_ops() {
        let out = generate_runner(&[doc_spec()]).unwrap();
        assert!(out.starts_with("// Generated by `hifumi export-ts`. DO NOT EDIT."));
        assert!(out.contains(r#"export const chains: Record<string, Chain> = {"#));
        assert!(out.contains(r#"        versions: ["0.1", "0.2", "0.3"],"#));
        assert!(out.contains(r#"            { from: "0.1", to: "0.2", ops: ["#));
        assert!(out.contains(r#"                { kind: "remove", field: "extra" },"#));
        assert!(out.contains(r#"                { kind: "rename", from: "title", to: "name" },"#));
        assert!(out.contains(r#"{ kind: "add", field: "note", value: "" }"#));
        assert!(out.contains("        baseline: null,"));
        // runner 入口与降级护栏都在
        assert!(out.contains("export function migrate("));
        assert!(out.contains("is Rust-side only"));
    }

    #[test]
    fn renders_baseline_and_empty_steps() {
        let spec = TsTypeSpec {
            name: "Solo".to_string(),
            versions: vec!["0.4".to_string()],
            baseline: Some("0.4".to_string()),
            steps: vec![],
        };
        let out = generate_runner(&[spec]).unwrap();
        assert!(out.contains(r#"        baseline: "0.4","#));
        assert!(out.contains("        steps: [],\n"));
    }

    #[test]
    fn json_default_rendering() {
        assert_eq!(Json::Null.render(), "null");
        assert_eq!(Json::Bool(false).render(), "false");
        assert_eq!(Json::Int(0).render(), "0");
        assert_eq!(Json::Float(0.0).render(), "0.0");
        assert_eq!(Json::Str(String::new()).render(), r#""""#);
        assert_eq!(Json::Arr.render(), "[]");
        assert_eq!(Json::Obj.render(), "{}");
        // 防御性转义：规格是公开 API，不假设调用方只喂标识符
        assert_eq!(quote_ts("a\"b\\c\n"), r#""a\"b\\c\n""#);
    }

    #[test]
    fn rejects_incoherent_specs() {
        // 重名
        let err = generate_runner(&[doc_spec(), doc_spec()]).unwrap_err();
        assert!(err.to_string().contains("duplicate type name"));

        // 步引用链外版本
        let mut spec = doc_spec();
        spec.steps[0].from = "0.9".to_string();
        let err = generate_runner(&[spec]).unwrap_err();
        assert!(err.to_string().contains("outside its chain"));

        // 空链
        let spec = TsTypeSpec {
            name: "X".to_string(),
            versions: vec![],
            baseline: None,
            steps: vec![],
        };
        let err = generate_runner(&[spec]).unwrap_err();
        assert!(err.to_string().contains("empty version chain"));
    }
}
