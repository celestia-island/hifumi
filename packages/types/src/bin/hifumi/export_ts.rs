//! `hifumi export-ts` —— 把 `#[version]` 类型链导出为 TS 前向迁移 runner。
//!
//! 与 `compact` 同一套源级扫描纪律：文本定位（属性块 / 结构体归属），
//! `syn` 只解析**结果**。差异在于本命令要深入 `#[migration]` 的变更块，
//! 逐字段形态分类——分类器是宏侧 `MigrationField` 文法的镜像子集：
//! 可在 TS 侧复演的结构步放行，其余**拒绝并列名**（静默跳过 = 数据丢失）。
//!
//! 可导出（前向语义与 Rust `From` 生成物一致）：
//! - `+ a: ty`（无转换器）：默认值按 Rust `Default` 投影为 JSON 字面量；
//! - `- a: ty`（含 `{ back: … }`：反向表达式不影响前向）；
//! - `+ a: ty => b: ty` / `+ a => b: ty`（单源**同型**复制，源保留）；
//! - `a: A => b: A` / `a => b: A`（单源**同型**纯改名，源消失）。
//!
//! 一律拒绝：带前向转换器的任何 op、多源 `(…) => …`、跨型 `.into()`、
//! `Default` 无法投影到 JSON 的类型（自定义结构体等）。

use anyhow::{bail, Context, Result};
use hifumi::ts::{generate_runner, Json, TsOp, TsStep, TsTypeSpec};
use proc_macro2::TokenStream;
use quote::ToTokens;
use std::collections::HashSet;
use std::path::Path;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    token, Ident, Token, Type,
};

use crate::compact::{
    decl_start, ignored_spans, in_ignored, is_migration_block, is_version_block, match_brace,
    scan_attr_blocks, string_literals, AttrBlock,
};

/// 导出入口：源码 → runner 源码。
pub fn export_source(source: &str) -> Result<String> {
    let specs = collect_specs(source)?;
    generate_runner(&specs)
}

/// CLI 入口：读文件 → 生成 runner → 打印或落盘。
pub fn export_file(file: &Path, out: Option<&Path>) -> Result<()> {
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    let runner = export_source(&source)?;
    match out {
        Some(path) => {
            std::fs::write(path, &runner)
                .with_context(|| format!("failed to write {}", path.display()))?;
            eprintln!("wrote TS runner to {}", path.display());
        }
        None => print!("{runner}"),
    }
    Ok(())
}

/// 收集文件里所有 `#[version]` 结构体的导出规格（按源码出现次序）。
fn collect_specs(source: &str) -> Result<Vec<TsTypeSpec>> {
    let mut specs = vec![];
    let mut seen_names: HashSet<String> = HashSet::new();
    for (name, start) in find_structs(source) {
        let blocks = match scan_attr_blocks(source, decl_start(source, start)) {
            Ok(blocks) => blocks,
            Err(_) => continue, // 无主体 / 形态异常的命中不可能是 DSL 结构体
        };
        let spec = match spec_for_struct(&name, start, &blocks)? {
            Some(spec) => spec,
            None => continue, // 无 #[version]，非 DSL 类型
        };
        if !seen_names.insert(name.clone()) {
            bail!("duplicate `#[version]` struct `{name}` in the same file");
        }
        specs.push(spec);
    }
    if specs.is_empty() {
        bail!("no `#[version]` structs found in the source; the TS runner is exported from hifumi DSL declarations");
    }
    Ok(specs)
}

/// 文本扫描所有 `struct NAME` 声明（与 compact 的单名定位同一判据：
/// 前一字符非标识符字符，排除 `mystruct Foo` 误命中；注释 / 字符串里
/// 提到 `struct X` 的幽灵命中一并排除）。
fn find_structs(source: &str) -> Vec<(String, usize)> {
    let ignored = ignored_spans(source);
    let mut hits = vec![];
    let mut search_from = 0usize;
    while let Some(rel) = source[search_from..].find("struct ") {
        let at = search_from + rel;
        search_from = at + "struct ".len();
        let prev_ok = at == 0
            || !source[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
        if !prev_ok || in_ignored(&ignored, at) {
            continue;
        }
        let rest = &source[at + "struct ".len()..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        hits.push((name, at));
    }
    hits
}

/// 由单个结构体的归属属性构建导出规格；无 `#[version]` 返回 `None`。
fn spec_for_struct(name: &str, _start: usize, blocks: &[AttrBlock]) -> Result<Option<TsTypeSpec>> {
    let version_blocks: Vec<&AttrBlock> = blocks
        .iter()
        .filter(|b| is_version_block(&b.text))
        .collect();
    if version_blocks.is_empty() {
        return Ok(None);
    }
    if version_blocks.len() > 1 {
        bail!("multiple #[version] attributes on `{name}`");
    }
    let version_block = version_blocks[0];
    let (final_version, baseline) = version_and_baseline(&version_block.text)
        .with_context(|| format!("`#[version]` on `{name}`"))?;

    let mut migrations = vec![];
    for b in blocks.iter().filter(|b| is_migration_block(&b.text)) {
        // 与 compact 同一前置纪律：#[version] 必须先于所有 #[migration]。
        if b.start < version_block.start {
            bail!(
                "#[migration] appears before #[version] on `{name}`; hifumi requires \
                 the #[version] attribute first — fix the source order before exporting"
            );
        }
        let (from, to) = migration_header(&b.text)?;
        let body = change_block_body(&b.text)?;
        let ops = parse_ops(&body).map_err(|e| {
            anyhow::anyhow!("step {from:?} => {to:?} of `{name}` is not exportable: {e}")
        })?;
        migrations.push((from, to, ops));
    }

    // 沿 to 从最终版本回溯（与 compact 同口径），按 baseline 截断灭绝步。
    let mut retained: Vec<(String, String, Vec<TsOp>)> = vec![];
    let mut retained_froms: Vec<String> = vec![];
    let mut current = final_version.clone();
    let mut seen: HashSet<String> = [final_version.clone()].into();
    let mut hit_baseline = false;
    while let Some(step) = migrations.iter().find(|(_, to, _)| *to == current) {
        let (from, to, ops) = step;
        if hit_baseline || baseline.as_deref() == Some(to.as_str()) {
            hit_baseline = true;
        } else {
            retained.push((from.clone(), to.clone(), ops.clone()));
            retained_froms.push(from.clone());
        }
        if !seen.insert(from.clone()) {
            bail!("migration chain of `{name}` has a cycle at version {from:?}");
        }
        current = from.clone();
    }

    let baseline_in_chain = baseline.as_deref() == Some(final_version.as_str())
        || baseline
            .as_deref()
            .is_some_and(|b| retained_froms.iter().any(|v| v == b));
    if let Some(b) = &baseline {
        if !baseline_in_chain {
            bail!(
                "baseline {b:?} of `{name}` is not a version of this chain; expected one \
                 of the migration step versions or the current version {final_version:?}"
            );
        }
    }

    // 版本链旧 → 新；步序同步反转。
    let mut versions: Vec<String> = retained_froms.into_iter().rev().collect();
    versions.push(final_version.clone());
    retained.reverse();

    Ok(Some(TsTypeSpec {
        name: name.to_string(),
        versions,
        baseline,
        steps: retained
            .into_iter()
            .map(|(from, to, ops)| TsStep { from, to, ops })
            .collect(),
    }))
}

/// `#[version("x"[, baseline = "y"])]` → 版本号 + 已声明的压缩底线。
fn version_and_baseline(text: &str) -> Result<(String, Option<String>)> {
    let inner_open = text.find('(').context("#[version has no parens")?;
    let lits = string_literals(&text[inner_open..]);
    let version = lits
        .first()
        .map(|s| s.to_string())
        .context("#[version] has no explicit version literal (CARGO_PKG_VERSION fallback is compile-time only); add one before exporting")?;
    let baseline = text
        .find("baseline")
        .and_then(|at| string_literals(&text[at..]).first().map(|s| s.to_string()));
    Ok((version, baseline))
}

/// `#[migration("a" => "b" …)]` 头部版本对（无变更块 = 空步）。
fn migration_header(text: &str) -> Result<(String, String)> {
    let head = match text.find('{') {
        Some(at) => &text[..at],
        None => text,
    };
    let lits = string_literals(head);
    if lits.len() < 2 {
        bail!("migration header missing `\"from\" => \"to\"` pair: {head}");
    }
    Ok((lits[0].to_string(), lits[1].to_string()))
}

/// `#[migration(… { ops })]` 的变更块内文（配平提取，转换器里的嵌套
/// 括号 / 字符串不破坏边界）；无块 = 空。
fn change_block_body(text: &str) -> Result<String> {
    let open = match text.find('{') {
        Some(at) => at,
        None => return Ok(String::new()),
    };
    let close = match_brace(text, open)?;
    Ok(text[open + 1..close - 1].to_string())
}

// ---------------------------------------------------------------------------
// 字段形态分类器：宏文法的镜像子集，产出 TsOp 或拒绝（带原因）。
// ---------------------------------------------------------------------------

struct Ops(Vec<TsOp>);

impl Parse for Ops {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut ops = vec![];
        while !input.is_empty() {
            ops.push(input.parse::<FieldOp>()?.0);
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Ops(ops))
    }
}

fn parse_ops(body: &str) -> Result<Vec<TsOp>> {
    if body.trim().is_empty() {
        return Ok(vec![]);
    }
    let Ops(ops) = syn::parse_str(body).map_err(|e: syn::Error| anyhow::anyhow!("{e}"))?;
    Ok(ops)
}

struct FieldOp(TsOp);

impl Parse for FieldOp {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        field_op(input).map(FieldOp)
    }
}

/// 字段形态分类主体（`MigrationField::parse` 的镜像子集）。
fn field_op(input: ParseStream) -> syn::Result<TsOp> {
    if input.peek(Token![+]) {
        input.parse::<Token![+]>()?;
        if input.peek(token::Paren) {
            return Err(input.error(
                "multi-source `+ (a: A, b: B) => c: C` requires a Rust converter, \
                 which cannot cross the language boundary",
            ));
        }
        let key = input.parse::<Ident>()?;
        if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            let source_ty = input.parse::<Type>()?;
            if input.peek(Token![=>]) {
                // `+ a: A => b: B`：单源复制（源保留）
                input.parse::<Token![=>]>()?;
                let target = input.parse::<Ident>()?;
                input.parse::<Token![:]>()?;
                let target_ty = input.parse::<Type>()?;
                if input.peek(token::Brace) {
                    return Err(input
                        .error("copy with a forward converter is Rust-only and not exportable"));
                }
                if ty_text(&source_ty) != ty_text(&target_ty) {
                    return Err(input.error(format!(
                        "copy `{}: {}` => `{}: {}` crosses types via `.into()` and is not exportable",
                        key, ty_text(&source_ty), target, ty_text(&target_ty)
                    )));
                }
                Ok(TsOp::Copy {
                    from: key.to_string(),
                    to: target.to_string(),
                })
            } else if input.peek(token::Brace) {
                Err(input.error("add with a converter expression is Rust-only and not exportable"))
            } else {
                // `+ a: ty`：默认值补字段
                let default = rust_default(&source_ty).map_err(|e| input.error(e.to_string()))?;
                Ok(TsOp::Add {
                    field: key.to_string(),
                    default,
                })
            }
        } else {
            // `+ a => b: ty`：文法保证两侧同型
            input.parse::<Token![=>]>()?;
            let target = input.parse::<Ident>()?;
            input.parse::<Token![:]>()?;
            let _ty = input.parse::<Type>()?;
            if input.peek(token::Brace) {
                return Err(
                    input.error("copy with a forward converter is Rust-only and not exportable")
                );
            }
            Ok(TsOp::Copy {
                from: key.to_string(),
                to: target.to_string(),
            })
        }
    } else if input.peek(Token![-]) {
        // `- a: ty [{ back: … }]`：前向永远纯结构，反向表达式不导出但合法
        input.parse::<Token![-]>()?;
        let key = input.parse::<Ident>()?;
        input.parse::<Token![:]>()?;
        let _ty = input.parse::<Type>()?;
        if input.peek(token::Brace) {
            let content;
            braced!(content in input);
            parse_back_body(&content)?;
        }
        Ok(TsOp::Remove {
            field: key.to_string(),
        })
    } else if input.peek(token::Paren) {
        Err(input.error(
            "multi-source `(a: A, b: B) => c: C` requires a Rust converter, \
             which cannot cross the language boundary",
        ))
    } else {
        rename_family(input)
    }
}

/// Rename 族：`a: A => b: B` / `a: A => B`（保名换型）/ `a => b: A`，
/// 后随可选 `{ fwd } { back: … }`。可导出 = 无前向转换器 ∧ 单源同型。
fn rename_family(input: ParseStream) -> syn::Result<TsOp> {
    let source = input.parse::<Ident>()?;
    let (source_ty, target, target_ty) = if input.peek(Token![:]) {
        input.parse::<Token![:]>()?;
        let source_ty = input.parse::<Type>()?;
        input.parse::<Token![=>]>()?;
        if input.peek2(Token![:]) {
            // `a: A => b: B`
            let target = input.parse::<Ident>()?;
            input.parse::<Token![:]>()?;
            (source_ty, target, input.parse::<Type>()?)
        } else {
            // `a: A => B`：保名换型
            let target_ty = input.parse::<Type>()?;
            (source_ty, source.clone(), target_ty)
        }
    } else {
        // `a => b: A`：文法保证两侧同型
        input.parse::<Token![=>]>()?;
        let target = input.parse::<Ident>()?;
        input.parse::<Token![:]>()?;
        let ty = input.parse::<Type>()?;
        (ty.clone(), target, ty)
    };

    // 后随括号：`{ fwd }`（拒绝）或 `{ back: … }`（前向无关）。合法形态
    // 不会出现第二括号跟随 back（宏侧报错），本解析器在逗号切分处自然
    // 报「expected `,`」，口径一致。
    if input.peek(token::Brace) {
        let content;
        braced!(content in input);
        if peek_back_label(&content) {
            parse_back_body(&content)?;
        } else {
            let _ = content.parse::<TokenStream>()?;
            return Err(
                input.error("rename with a forward converter is Rust-only and not exportable")
            );
        }
    }

    if ty_text(&source_ty) != ty_text(&target_ty) {
        return Err(input.error(format!(
            "rename `{}: {}` => `{}: {}` crosses types via `.into()` and is not exportable",
            source,
            ty_text(&source_ty),
            target,
            ty_text(&target_ty)
        )));
    }
    // 自改名（含 `a: A => A` 保名换同型）：Rust 侧是恒等，TS 模板的
    // set+delete 却会删字段——语义发散，按名拒收。
    if source == target {
        return Err(input.error(format!(
            "`{source}` => `{target}` renames a field to itself; drop the op instead \
             (the TS runner would silently drop the field)"
        )));
    }
    Ok(TsOp::Rename {
        from: source.to_string(),
        to: target.to_string(),
    })
}

/// peek 判定「下一个是 `标签:` 而非路径开头」（镜像宏侧 jointness 判据）。
fn peek_back_label(content: ParseStream) -> bool {
    content.peek(Ident) && content.peek2(Token![:]) && !content.peek2(Token![::])
}

/// `{ back: <expr> }`：合法反向括号则消费，否则报错（镜像 parse_back_body）。
fn parse_back_body(content: ParseStream) -> syn::Result<()> {
    if !peek_back_label(content) {
        return Err(content.error("expected a backward converter brace `{ back: <expr> }`"));
    }
    let label = content.parse::<Ident>()?;
    if label != "back" {
        return Err(content.error("expected a backward converter brace `{ back: <expr> }`"));
    }
    content.parse::<Token![:]>()?;
    let _ = content.parse::<TokenStream>()?;
    if !content.is_empty() {
        return Err(content.error("unexpected tokens after the backward converter"));
    }
    Ok(())
}

/// Rust `Default` → JSON 投影；投影不了的类型拒绝（自定义默认值语义
/// 属 Rust 侧，猜一个 TS 值等于换语义）。
///
/// 按**名字最后一段**分类（`Vec<i32>`、`std::collections::HashMap<K, V>`、
/// `Option<String>` 一律归结到 `Vec`/`HashMap`/`Option`）。这是**结构性投影**，
/// 不是 `Default` 的等值复制：只对名字最后一段恰好是这几个结构类型的路径成立，
/// 同名的自定义类型会被当成结构类型处理（宁可接受也不猜）。
///
/// 与宏侧 `<#ty as Default>::default()` 的关系**不是「同一类型集」**，两侧各有
/// 对方没有的成员：这里拒绝 `[u8; 4]` 这类非路径类型与 `PathBuf` 这类没有结构
/// 默认的路径（宏侧对它们照常调 `Default::default()`，只要求该类型真的实现了
/// `Default`）；反过来，这里的「按名字最后一段猜」比宏侧的 trait 解析宽松。
///
/// 裸 `str` 归到 `String`：`syn` 把 `str` 解析成 `Type::Path`（ident 为 `str`），
/// 所以它确实会走到这里。注意 `str` 本身不是 `Default`（它是 unsized，只有
/// `&str`/`&mut str` 实现 `Default`）；这里的映射是 TS 导出器对「字符串形态」
/// 的投影决定，不是在复述 Rust 的 `Default` 实现。
fn rust_default(ty: &Type) -> Result<Json> {
    let Type::Path(path) = ty else {
        bail!(
            "cannot express Rust `Default` for type `{}` in the TS runner; use a \
             structural default (String / bool / number / Vec / Option / map) or keep \
             this migration in Rust",
            ty_text(ty)
        );
    };
    let ident = path
        .path
        .segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default();
    match ident.as_str() {
        "String" | "str" => Ok(Json::Str(String::new())),
        "bool" => Ok(Json::Bool(false)),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128"
        | "isize" | "f32" | "f64" => Ok(Json::Int(0)),
        "Vec" => Ok(Json::Arr),
        "Option" => Ok(Json::Null),
        "HashMap" | "BTreeMap" => Ok(Json::Obj),
        _ => bail!(
            "cannot express Rust `Default` for type `{}` in the TS runner; use a \
             structural default (String / bool / number / Vec / Option / map) or keep \
             this migration in Rust",
            ty_text(ty)
        ),
    }
}

fn ty_text(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_op(body: &str) -> Result<TsOp> {
        syn::parse_str::<FieldOp>(body)
            .map(|FieldOp(op)| op)
            .map_err(|e: syn::Error| anyhow::anyhow!("{e}"))
    }

    #[test]
    fn classifies_structural_ops() {
        assert_eq!(
            one_op("+ note: String").unwrap(),
            TsOp::Add {
                field: "note".into(),
                default: Json::Str(String::new())
            }
        );
        assert_eq!(
            one_op("+ count: u32").unwrap(),
            TsOp::Add {
                field: "count".into(),
                default: Json::Int(0)
            }
        );
        assert_eq!(
            one_op("+ tags: Vec<String>").unwrap(),
            TsOp::Add {
                field: "tags".into(),
                default: Json::Arr
            }
        );
        assert_eq!(
            one_op("+ extra: Option<i64>").unwrap(),
            TsOp::Add {
                field: "extra".into(),
                default: Json::Null
            }
        );
        assert_eq!(
            one_op("- extra: String").unwrap(),
            TsOp::Remove {
                field: "extra".into()
            }
        );
        // 反向表达式不影响前向结构
        assert_eq!(
            one_op("- extra: String { back: \"gone\".to_string() }").unwrap(),
            TsOp::Remove {
                field: "extra".into()
            }
        );
        assert_eq!(
            one_op("+ old: String => note: String").unwrap(),
            TsOp::Copy {
                from: "old".into(),
                to: "note".into()
            }
        );
        assert_eq!(
            one_op("+ old => note: String").unwrap(),
            TsOp::Copy {
                from: "old".into(),
                to: "note".into()
            }
        );
        assert_eq!(
            one_op("title: String => name: String").unwrap(),
            TsOp::Rename {
                from: "title".into(),
                to: "name".into()
            }
        );
        // 仅 back 的改名仍是前向纯改名
        assert_eq!(
            one_op("title: String => name: String { back: __new.name.clone() }").unwrap(),
            TsOp::Rename {
                from: "title".into(),
                to: "name".into()
            }
        );
        assert_eq!(
            one_op("title => name: String").unwrap(),
            TsOp::Rename {
                from: "title".into(),
                to: "name".into()
            }
        );
    }

    /// 泛型 / 限定路径类型：默认值投影按**名字最后一段**分类，故
    /// `Vec<i32>` 与 `std::vec::Vec<i32>` 等价。只钉分类与投影这一层：
    /// 这里的解析器本来就吃 `Vec<i32>`（`TypePath` 子集自始支持），
    /// 旧版报 `comparison operators cannot be chained` 的是**宏侧**把
    /// 迁移类型 `#ty::default()` 拼进 `+` 表达式，与本函数无关。
    #[test]
    fn generic_and_qualified_types_classify() {
        assert_eq!(
            one_op("+ tags: Vec<i32>").unwrap(),
            TsOp::Add {
                field: "tags".into(),
                default: Json::Arr
            }
        );
        assert_eq!(
            one_op("+ extra: Option<String>").unwrap(),
            TsOp::Add {
                field: "extra".into(),
                default: Json::Null
            }
        );
        assert_eq!(
            one_op("+ counts: std::collections::HashMap<String, u32>").unwrap(),
            TsOp::Add {
                field: "counts".into(),
                default: Json::Obj
            }
        );
        assert_eq!(
            one_op("+ m: BTreeMap<K, V>").unwrap(),
            TsOp::Add {
                field: "m".into(),
                default: Json::Obj
            }
        );
        // 复制/改名两侧都是泛型时也按 token 文本比对同型
        assert_eq!(
            one_op("+ a: Vec<i32> => b: Vec<i32>").unwrap(),
            TsOp::Copy {
                from: "a".into(),
                to: "b".into()
            }
        );
        // 非路径类型（数组）没有可投影的默认值 → 拒绝，而不是猜。
        // 这是本层自己的拒绝：`one_op` 直接走 `syn::parse_str`，不经宏，
        // 故钉的就是 `rust_default` 的 `Type::Path` 前置条件。
        let err = one_op("+ grid: [u8; 4]").unwrap_err();
        assert!(err.to_string().contains("cannot express"), "{err}");
    }

    /// 裸 `str` 归到空串：`syn` 把 `str` 解析成 `Type::Path`（ident `str`），
    /// 所以它确实会走到 `rust_default`。（这是导出器的投影决定——`str` 本身
    /// 不是 `Default`，unsized。）
    #[test]
    fn bare_str_projects_to_the_empty_string() {
        assert_eq!(
            one_op("+ note: str").unwrap(),
            TsOp::Add {
                field: "note".into(),
                default: Json::Str(String::new())
            }
        );
    }

    #[test]
    fn refuses_non_exportable_ops() {
        let cases = [
            ("+ note: String { String::from(\"x\") }", "converter"),
            ("+ (a: u32, b: u32) => c: u32 { a + b }", "multi-source"),
            ("(a: u32, b: u32) => c: u32 { a + b }", "multi-source"),
            ("+ n: u32 => m: i64", "crosses types"),
            ("n: u32 => m: i64", "crosses types"),
            ("+ old: String => note: String { old.clone() }", "converter"),
            (
                "title: String => name: String { __old.title.clone() }",
                "converter",
            ),
            ("+ extra: PathBuf", "cannot express"),
            (
                "- extra: String { \"not back\".to_string() }",
                "backward converter",
            ),
        ];
        for (body, needle) in cases {
            let err = one_op(body).unwrap_err();
            assert!(
                err.to_string().contains(needle),
                "op {body:?} refused with {err:?}, expected reason containing {needle:?}"
            );
        }
    }
}
