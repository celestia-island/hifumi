//! `hifumi compact` —— 源级版本链压缩。
//!
//! 对源文件里某个 `#[version]` 类型声明压缩底线：删除比 baseline 更老的
//! `#[migration]` 属性块，并在 `#[version(...)]` 里写入 `baseline = "Y"`。
//! 宏侧（编译期）随后不再生成灭绝版本的旧结构与 From 步。
//!
//! 文本手术而非 syn 全文件重打印：保住用户文件的既有格式。定位靠自
//! 扫描器（`#[` 起步、括号配平定块边界），`syn` 只用来解析**结果**做
//! 校验——输出必须是合法 Rust 且目标结构体确实带上了 baseline。

use anyhow::{bail, Context, Result};
use std::path::Path;

/// 扫描出的属性块：字节范围 + 文本。
pub(crate) struct AttrBlock {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) text: String,
}

/// DSL 属性判定：`#[version(…)]` / 裸 `#[version]`（CARGO_PKG_VERSION
/// 回退）；不误吞 `#[versioned_…]` 之类同前缀属性。
pub(crate) fn is_version_block(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("#[version(") || t == "#[version]"
}

pub(crate) fn is_migration_block(text: &str) -> bool {
    text.trim_start().starts_with("#[migration(")
}

/// 在 `#[version(...)]` 属性里追加 `baseline = "Y"`，并删除 `to`
/// 早于（或等于）baseline 的 `#[migration]` 属性块。
///
/// 返回重写后的完整文件内容。`--write` 语义由调用方落盘。
pub fn compact_source(source: &str, struct_name: &str, baseline: &str) -> Result<String> {
    let struct_span = locate_struct(source, struct_name)?;
    // 归属扫描从声明起点（含 `pub` / `pub(…)` 前缀）回走，否则可见性
    // 关键字会挡在属性与光标之间，`pub struct` 拿不到任何属性。
    let blocks = scan_attr_blocks(source, decl_start(source, struct_span.0))?;

    let version_block = blocks
        .iter()
        .find(|b| is_version_block(&b.text))
        .with_context(|| format!("no #[version] attribute on `{struct_name}`"))?;
    if blocks.iter().filter(|b| is_version_block(&b.text)).count() > 1 {
        bail!("multiple #[version] attributes on `{struct_name}`");
    }

    let migrations = blocks
        .iter()
        .filter(|b| is_migration_block(&b.text))
        .map(|b| {
            let (from, to) = migration_versions(&b.text)?;
            Ok((from, to, b))
        })
        .collect::<Result<Vec<_>>>()?;

    // `#[version]` 必须先于所有 `#[migration]`（宏展开次序要求，反序
    // 源码本就编译不过）。提前拒绝这类输入：文本手术只会产出同样编译
    // 不过的结果，还可能把注释里的伪 migration 块误当真。
    if let Some(first) = migrations.first() {
        if first.2.start < version_block.start {
            bail!(
                "#[migration] appears before #[version] on `{struct_name}`; hifumi requires \
                 the #[version] attribute first — fix the source order before compacting"
            );
        }
    }

    // 沿 `to` 从最终版本回溯到链底：
    // - 终点 == baseline 的那一步起（含）全部灭绝——覆盖 baseline == final
    //   的整链灭绝情形（此时回走不能在第一步就停，须删掉全部步）；
    // - 其余步保留，保留步的 from 构成 baseline 的合法取值集。
    let final_version = version_literal(&version_block.text)?;
    let mut retained_tos: Vec<String> = vec![];
    let mut retained_froms: Vec<String> = vec![];
    let mut extinct_tos: Vec<String> = vec![];
    let mut current = final_version.clone();
    let mut seen: std::collections::HashSet<String> = [final_version.clone()].into();
    let mut hit_baseline = false;
    while let Some((from, to, _)) = migrations.iter().find(|(_, to, _)| *to == current) {
        if hit_baseline || to == baseline {
            hit_baseline = true;
            extinct_tos.push(current.clone());
        } else {
            retained_tos.push(current.clone());
            retained_froms.push(from.clone());
        }
        if !seen.insert(from.clone()) {
            bail!("migration chain has a cycle at version {from:?}");
        }
        current = from.clone();
    }

    // 合法 baseline = 保留步的 from ∪ {final}（与宏侧截断后的版本集一致）
    let baseline_in_chain =
        baseline == final_version || retained_froms.iter().any(|v| v == baseline);
    if !baseline_in_chain {
        bail!(
            "baseline {baseline:?} is not a version of this chain; expected one of the \
             migration step versions or the current version {final_version:?}"
        );
    }

    // 字节级重写：删除灭绝块（连同尾随换行），再改 #[version] 文本。
    // #[version] 块的位置按其之前删除的总字节数（含换行吸收）平移。
    let mut cut_ranges: Vec<(usize, usize)> = migrations
        .iter()
        .filter(|(_, to, _)| extinct_tos.contains(to))
        .map(|(_, _, b)| (b.start, b.end))
        .collect();
    cut_ranges.sort();
    let mut out = String::with_capacity(source.len());
    let mut cursor = 0usize;
    let mut version_start = version_block.start;
    let mut version_end = version_block.end;
    for (start, end) in cut_ranges.iter().copied() {
        out.push_str(&source[cursor..start]);
        cursor = end;
        let mut removed = end - start;
        // 吃掉紧随其后的换行，避免留空行
        if source[cursor..].starts_with('\n') {
            cursor += 1;
            removed += 1;
        } else if source[cursor..].starts_with("\r\n") {
            cursor += 2;
            removed += 2;
        }
        if start < version_start {
            version_start -= removed;
            version_end -= removed;
        }
    }
    out.push_str(&source[cursor..]);

    let attr_text = &out[version_start..version_end];
    let rewritten = rewrite_version_attr(attr_text, baseline)?;
    out.replace_range(version_start..version_end, &rewritten);

    // 校验：输出必须是合法 Rust，且目标结构体的 #[version] 确实带上了
    // baseline；migration 属性数等于保留步数。
    validate(&out, struct_name, baseline, retained_tos.len())?;

    Ok(out)
}

/// 定位结构体声明主体的字节范围（`struct NAME` 到配平的 `}`）。
fn locate_struct(source: &str, name: &str) -> Result<(usize, usize)> {
    let needle = format!("struct {name}");
    let ignored = ignored_spans(source);
    let mut search_from = 0usize;
    loop {
        let rel = source[search_from..]
            .find(&needle)
            .with_context(|| format!("struct `{name}` not found in the file"))?;
        let at = search_from + rel;
        // 粗判：前一字符不是标识符字符（排除 `mystruct Foo` 之类误命中）
        let prev_ok = at == 0
            || !source[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
        // 注释 / 字符串里提到 `struct NAME` 的幽灵命中跳过
        if prev_ok && !in_ignored(&ignored, at) {
            // 结构体结束 = 从声明行向后找第一个配平的 `}`
            let body_start = source[at..]
                .find('{')
                .with_context(|| "struct declaration has no body")?;
            let end = match_brace(source, at + body_start)?;
            return Ok((at, end));
        }
        search_from = at + needle.len();
    }
}

/// 声明起点回扩：`struct` 前的 `pub` / `pub(crate)` 等可见性前缀也算
/// 声明的一部分。属性归属扫描必须从它开始回走。
pub(crate) fn decl_start(source: &str, at: usize) -> usize {
    let bytes = source.as_bytes();
    let mut start = at;
    loop {
        // 1) 跳过紧邻的空白
        let mut t = start;
        while t > 0 && bytes[t - 1].is_ascii_whitespace() {
            t -= 1;
        }
        if t == start {
            break; // 前面不是空白（或顶格）：没有可剥的前缀
        }
        // 2) `pub(…)`：先回配平一对括号
        if bytes.get(t.wrapping_sub(1)) == Some(&b')') {
            let mut depth = 0usize;
            let mut i = t - 1;
            let mut open = None;
            while i > 0 {
                match bytes[i] {
                    b')' => depth += 1,
                    b'(' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            open = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
                i -= 1;
            }
            match open {
                Some(o) => t = o,
                None => break,
            }
        }
        // 3) 前一个词必须是 `pub`，否则到此为止（属性块、注释、其它项）
        let mut w = t;
        while w > 0 {
            let b = bytes[w - 1];
            if b.is_ascii_alphanumeric() || b == b'_' {
                w -= 1;
            } else {
                break;
            }
        }
        if &source[w..t] == "pub" {
            start = w;
        } else {
            break;
        }
    }
    start
}

/// 从 `open`（指向 `{`）起配平，返回闭括号后的偏移。
pub(crate) fn match_brace(source: &str, open: usize) -> Result<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut in_char = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut i = open;
    while i < bytes.len() {
        let b = bytes[i];
        match (in_line_comment, in_block_comment, in_str, in_char) {
            (true, _, _, _) => {
                if b == b'\n' {
                    in_line_comment = false;
                }
            }
            (_, true, _, _) => {
                if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    in_block_comment = false;
                    i += 1;
                }
            }
            (_, _, true, _) => {
                if b == b'\\' {
                    i += 1;
                } else if b == b'"' {
                    in_str = false;
                }
            }
            (_, _, _, true) => {
                if b == b'\\' {
                    i += 1;
                } else if b == b'\'' {
                    in_char = false;
                }
            }
            _ => {
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    in_line_comment = true;
                    i += 1;
                } else if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                    in_block_comment = true;
                    i += 1;
                } else if b == b'"' {
                    in_str = true;
                } else if b == b'\'' {
                    // 生命周期 vs 字符字面量的区分：只在意块级配平，
                    // 把 `'` 误判为 char 起点也不会吞掉 `{`/`}`。
                    in_char = true;
                } else if b == b'{' {
                    depth += 1;
                } else if b == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(i + 1);
                    }
                }
            }
        }
        i += 1;
    }
    bail!("unbalanced braces starting at offset {open}")
}

/// 扫描紧邻结构体声明的连续属性块（`#[...]` 到配平收尾）。
///
/// 只认 `#[` 块之间夹空白、`//` 行注释或 `/* */` 块注释的连续段——
/// `#[version]` / `#[migration]` 是 DSL 属性，中间夹其它项说明找错了
/// 结构体。
pub(crate) fn scan_attr_blocks(source: &str, struct_start: usize) -> Result<Vec<AttrBlock>> {
    let head = &source[..struct_start];
    // 候选块：整个前文中所有 `#[...]` 配平块；注释与字符串字面量里的
    // `#[` 不算（合法源码里 `// TODO: 修 #[broken]` 之类并不罕见，不排除
    // 会把真实结构体的候选扫描炸掉或产出垃圾候选）。
    let ignored = ignored_spans(head);
    let mut candidates: Vec<AttrBlock> = vec![];
    let mut i = 0usize;
    while let Some(rel) = head[i..].find("#[") {
        let start = i + rel;
        if ignored.iter().any(|&(a, b)| start >= a && start < b) {
            i = start + 2;
            continue;
        }
        let end = attr_end(head, start)?;
        candidates.push(AttrBlock {
            start,
            end,
            text: head[start..end].to_string(),
        });
        i = end;
    }
    // 从结构体声明处向前收：跳过空白与 `//` 行注释，命中 `end == cursor`
    // 的块即归属本结构体。
    let mut owned: Vec<AttrBlock> = vec![];
    let mut cursor = struct_start;
    loop {
        // 1) 跳过空白
        let mut trimmed = cursor;
        while trimmed > 0
            && source
                .as_bytes()
                .get(trimmed - 1)
                .is_some_and(|b| b.is_ascii_whitespace())
        {
            trimmed -= 1;
        }
        // 2) 若光标停在行内且该行以 `//` 开头，跳过整行注释
        if trimmed > 0 {
            let line_start = source[..trimmed].rfind('\n').map(|p| p + 1).unwrap_or(0);
            let line = source[line_start..trimmed].trim_start();
            if line.starts_with("//") {
                cursor = line_start;
                continue;
            }
        }
        // 2.5) 块注释：光标前以 `*/` 收尾时，跳到 ignored_spans 算出的
        // 该注释区间起点。不用 rfind("/*")——嵌套注释、注释内字符串里的
        // 内层 `/*` 会让它跳错位置，静默丢掉真结构体（Rust 块注释可嵌套）。
        if trimmed >= 2
            && source.as_bytes()[trimmed - 2] == b'*'
            && source.as_bytes()[trimmed - 1] == b'/'
        {
            if let Some(&(open, _)) = ignored.iter().find(|&&(a, b)| trimmed > a && trimmed <= b) {
                cursor = open;
                continue;
            }
        }
        // 3) 找 end == trimmed 的候选块
        match candidates.iter().rfind(|b| b.end == trimmed) {
            Some(b) => {
                cursor = b.start;
                owned.insert(
                    0,
                    AttrBlock {
                        start: b.start,
                        end: b.end,
                        text: b.text.clone(),
                    },
                );
            }
            None => break,
        }
    }
    Ok(owned)
}

/// 预扫文本中「不参与语法」的区间：行注释、块注释、字符串字面量
/// （朴素配对纪律与 [`string_literals`] 一致）。候选 `#[` 落在其中
/// 即不是属性。误扫 raw string 时最坏只会少认候选，不会误认。
pub(crate) fn ignored_spans(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut spans = vec![];
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                let end = text[i..].find('\n').map(|p| i + p).unwrap_or(bytes.len());
                spans.push((i, end));
                i = end;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                // 块注释按 Rust 语义嵌套：内部的 `/*`（包括看似字符串字面量
                // 里的）同样加深一层；未配平则吞到 EOF，与真实词法一致。
                let mut depth = 1usize;
                let mut j = i + 2;
                while j < bytes.len() && depth > 0 {
                    if bytes[j] == b'*' && bytes.get(j + 1) == Some(&b'/') {
                        depth -= 1;
                        j += 2;
                    } else if bytes[j] == b'/' && bytes.get(j + 1) == Some(&b'*') {
                        depth += 1;
                        j += 2;
                    } else {
                        j += 1;
                    }
                }
                spans.push((i, j));
                i = j;
            }
            b'"' => {
                // raw string：向前看 `r` + N 个 `#`（`r` 前可为 `b`/`c`，
                // 即 `br#"` / `cr#"` 字节 / C 字符串 raw literal；整个前缀
                // 之前不能还是标识符字符）。可跨行，闭界 = `"` + 同数
                // `#`；未闭合吞到 EOF。
                let mut hashes = 0usize;
                let mut k = i;
                while k > 0 && bytes[k - 1] == b'#' {
                    hashes += 1;
                    k -= 1;
                }
                let ident_tail = |pos: usize| {
                    pos > 0 && {
                        let prev = text[..pos].chars().next_back().unwrap();
                        prev.is_alphanumeric() || prev == '_'
                    }
                };
                let opener = if k > 0 && bytes[k - 1] == b'r' {
                    let r_pos = k - 1;
                    if !ident_tail(r_pos) {
                        Some(r_pos)
                    } else if r_pos > 0
                        && matches!(bytes[r_pos - 1], b'b' | b'c')
                        && !ident_tail(r_pos - 1)
                    {
                        Some(r_pos - 1)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(opener) = opener {
                    let closer = format!("\"{}", "#".repeat(hashes));
                    let end = text[i + 1..]
                        .find(&closer)
                        .map(|p| i + 1 + p + closer.len())
                        .unwrap_or(bytes.len());
                    spans.push((opener, end));
                    i = end;
                } else {
                    // 普通字符串不跨行；闭引号扫描跳过 `\"` 转义。行内
                    // 没有闭引号 = 孤立引号，只前进 1 字节，绝不把余下
                    // 全文吞进 span（会误杀后续真实属性/命中）。
                    let line_end = text[i..].find('\n').map(|p| i + p).unwrap_or(bytes.len());
                    let mut j = i + 1;
                    let mut closed = None;
                    while j < line_end {
                        if bytes[j] == b'\\' {
                            j += 2;
                            continue;
                        }
                        if bytes[j] == b'"' {
                            closed = Some(j + 1);
                            break;
                        }
                        j += 1;
                    }
                    match closed {
                        Some(end) => {
                            spans.push((i, end));
                            i = end;
                        }
                        None => i += 1,
                    }
                }
            }
            _ => i += 1,
        }
    }
    spans
}

/// `at` 处的命中是否落在注释（行 / 块）或字符串字面量内。文档注释里
/// 提到 `struct X` 很常见——命中点必须跳过，否则幽灵命中会认领真
/// 结构体的属性（重复名 bail 或导出幻影类型）。
pub(crate) fn in_ignored(spans: &[(usize, usize)], at: usize) -> bool {
    spans.iter().any(|&(a, b)| at >= a && at < b)
}

/// 属性块结束（`]` 之后），跳过字符串/注释/嵌套括号。
fn attr_end(source: &str, start: usize) -> Result<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut i = start;
    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'(' | b'{' | b'[' => depth += 1,
            b')' | b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    bail!("unterminated attribute starting at offset {start}")
}

/// 提取文本中所有 `"…"` 字符串字面量内容（配对消费开/闭引号）。
///
/// 版本号字面量不含转义与嵌套引号；扫到开引号后直接找下一个 `"` 作为
/// 闭引号并**跳过它**——朴素地按引号位置逐个取对，会把闭引号位置也当
/// 成下一对的开头，把 ` => ` 这类夹层当成字面量。
pub(crate) fn string_literals(text: &str) -> Vec<&str> {
    let mut lits = vec![];
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if let Some(rel) = text[i + 1..].find('"') {
                lits.push(&text[i + 1..i + 1 + rel]);
                i = i + 1 + rel + 1; // 越过闭引号
                continue;
            }
            // 未闭合的引号：终止，余下文本不属字面量
            break;
        }
        i += 1;
    }
    lits
}

/// 从 `#[migration("a" => "b" { ... })]` 文本中取 `"a"` 与 `"b"`。
fn migration_versions(text: &str) -> Result<(String, String)> {
    let body_start = text.find('{').context("migration has no change block")?;
    let head = &text[..body_start];
    let lits = string_literals(head);
    if lits.len() < 2 {
        bail!("migration header missing `\"from\" => \"to\"` pair: {head}");
    }
    Ok((lits[0].to_string(), lits[1].to_string()))
}

/// `#[version("x", ...)]` 里的版本字面量（缺省 = CARGO_PKG_VERSION，CLI
/// 无法在编译期得知——此时报错要求显式版本）。
fn version_literal(text: &str) -> Result<String> {
    let inner_open = text.find('(').context("#[version has no parens")?;
    let inner = &text[inner_open..];
    let lits = string_literals(inner);
    // baseline 字面量尚未写入；第一个字面量即版本号。
    lits.first().map(|s| s.to_string()).with_context(|| {
        "#[version] has no explicit version literal (CARGO_PKG_VERSION fallback is \
             compile-time only); add one before compacting"
    })
}

/// 把 `baseline = "Y"` 注入 `#[version(...)]` 属性文本。
fn rewrite_version_attr(attr_text: &str, baseline: &str) -> Result<String> {
    if attr_text.contains("baseline") {
        bail!("the #[version] attribute already declares a baseline");
    }
    let inner_end = attr_text
        .rfind(')')
        .context("#[version(...)] has no closing paren")?;
    let mut head = attr_text[..inner_end].to_string();
    if head.trim_end().ends_with('(') {
        head.push_str(&format!("baseline = \"{baseline}\""));
    } else {
        // 宏侧解析器接受尾逗号（`#[version("0.3",)]` 是合法 DSL），
        // 剥掉它再拼接，否则产出 `,,` 的非法属性文本。
        let trimmed = head.trim_end();
        if let Some(stripped) = trimmed.strip_suffix(',') {
            head.truncate(stripped.len());
        }
        head.push_str(&format!(", baseline = \"{baseline}\""));
    }
    Ok(format!("{}{}", head, &attr_text[inner_end..]))
}

/// 输出校验：合法 Rust + 目标结构体带 baseline + migration 数正确。
fn validate(out: &str, struct_name: &str, baseline: &str, retained: usize) -> Result<()> {
    let file = syn::parse_file(out).context("compacted source failed to re-parse")?;
    let item = file
        .items
        .iter()
        .find_map(|i| match i {
            syn::Item::Struct(s) if s.ident == struct_name => Some(s),
            _ => None,
        })
        .with_context(|| format!("struct `{struct_name}` missing after compaction"))?;
    let mut has_baseline = false;
    let mut migration_count = 0usize;
    for attr in &item.attrs {
        let ident = attr.path().require_ident()?.to_string();
        if ident == "migration" {
            migration_count += 1;
        }
        if ident == "version" {
            has_baseline = attr
                .meta
                .require_list()?
                .tokens
                .to_string()
                .contains("baseline");
        }
    }
    if !has_baseline {
        bail!("baseline did not land on #[version] after rewrite");
    }
    if migration_count != retained {
        bail!(
            "migration count {migration_count} != retained steps {retained}; \
             the compaction removed the wrong blocks"
        );
    }
    let _ = baseline;
    Ok(())
}

/// CLI 入口：读文件 → 压缩 → 打印或落盘。
pub fn compact_file(file: &Path, struct_name: &str, baseline: &str, write: bool) -> Result<()> {
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    let out = compact_source(&source, struct_name, baseline)?;
    if write {
        std::fs::write(file, &out)
            .with_context(|| format!("failed to write {}", file.display()))?;
        println!(
            "compacted `{struct_name}` to baseline {baseline:?} in {}",
            file.display()
        );
    } else {
        print!("{out}");
    }
    Ok(())
}
