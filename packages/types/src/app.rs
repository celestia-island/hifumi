//! 应用版本迁移 —— 面向"软件升级到某版本时执行一次动作"的脚手架。
//!
//! 与 [`crate::version`] 处理的"数据结构跨版本迁移"相对，这里处理的是
//! 应用自身的升级动作：把一批 `#[once(...)]` 声明的动作交给
//! `#[app_migrations]` 生成注册表，启动时依据账本判断哪些动作到期，
//! 执行成功即记账，永不重复执行。
//!
//! 准入判据是"升级跨入了该动作的版本"：`上次运行的版本 < since <= 当前版本`。
//! 首次安装（账本里没有上次运行版本）不执行任何动作 —— 新档案直接从当前
//! 默认值开始。`from` 可以给来源版本加下限，用于只对某个时代的升级生效。
//!
//! 账本是一个 JSON 文件（[`AppMigrationStore`]），记录每个已执行动作的
//! id 与执行时的版本，以及上次运行的版本。动作执行失败时不记账，下一次
//! 启动会重试，因此动作应当保持幂等。`delegate` 标记的动作不在本地执行：
//! 它们出现在待办列表里，由外部执行者（例如桌面应用的 WebView 前端）
//! 完成后调用 [`AppMigrationSet::mark_completed`] 记账。
//!
//! 注意：不要对 `#[once]` 函数做 `#[cfg]` 门控 —— 过程宏无法求值 cfg，
//! 生成的注册表会无条件引用函数名，被门控掉的函数会让编译失败。

use std::{cmp::Ordering, collections::BTreeMap, fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// 一次动作执行时能看到的上下文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppMigrationContext<'a> {
    /// 上一次运行的应用版本（首次安装为 `None`）。
    pub previous_version: Option<&'a str>,
    /// 本次运行的应用版本。
    pub current_version: &'a str,
}

/// 本地动作的函数签名（`#[once]` 标注的函数必须匹配此签名）。
pub type AppActionFn = for<'a> fn(&'a AppMigrationContext<'a>) -> Result<()>;

/// 动作的执行方式。
#[derive(Debug, Clone, Copy)]
pub enum AppMigrationAction {
    /// 注册表持有函数指针，引擎直接调用。
    Immediate(AppActionFn),
    /// 仅登记，由外部执行者完成后回报记账（见模块文档）。
    Delegated,
}

// 函数指针的地址比较没有意义，相等性只看变体（条目级比较用 id 与版本
// 字段足够）。
impl PartialEq for AppMigrationAction {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Immediate(_), Self::Immediate(_)) | (Self::Delegated, Self::Delegated)
        )
    }
}

impl Eq for AppMigrationAction {}

/// 注册表中的一条动作声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppMigrationEntry {
    /// 稳定唯一 id（取自 `#[once]` 函数名）。发布后不得改名：改名会遗弃
    /// 旧账本记录，导致动作重复执行。
    pub id: &'static str,
    /// 引入该动作的版本；升级跨入它时到期。
    pub since: &'static str,
    /// 来源版本下限（`#[once("from" => "since")]` 的 `from`）。
    pub from: Option<&'static str>,
    pub action: AppMigrationAction,
}

/// 由 `#[app_migrations]` 生成的动作注册表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMigrationSet {
    /// 当前应用版本（`#[app_migrations]` 的版本参数，默认取
    /// `CARGO_PKG_VERSION`）。
    pub current_version: &'static str,
    pub entries: Vec<AppMigrationEntry>,
}

/// 一轮本地动作执行的结果：成功的已记账，失败的未记账等待重试。
#[derive(Debug, Default)]
pub struct AppMigrationRun {
    pub executed: Vec<&'static str>,
    pub failed: Vec<(&'static str, anyhow::Error)>,
}

impl AppMigrationSet {
    /// 按 id 查找条目。
    pub fn find(&self, id: &str) -> Option<&AppMigrationEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// 用账本里的上次运行版本构造本次动作上下文。
    pub fn context<'a>(&self, store: &'a AppMigrationStore) -> AppMigrationContext<'a> {
        AppMigrationContext {
            previous_version: store.last_run_version(),
            current_version: self.current_version,
        }
    }

    /// 准入判断："升级跨入了 `since`" 且未执行过：
    /// `上次版本 < since <= 当前版本`，来源版本满足 `from` 下限。
    pub fn is_due(&self, store: &AppMigrationStore, entry: &AppMigrationEntry) -> bool {
        if store.completed_version(entry.id).is_some() {
            return false;
        }
        let Some(previous) = store.last_run_version() else {
            return false;
        };
        if compare_versions(previous, entry.since) != Ordering::Less {
            return false;
        }
        if compare_versions(self.current_version, entry.since) == Ordering::Less {
            return false;
        }
        if let Some(from) = entry.from {
            if compare_versions(previous, from) == Ordering::Less {
                return false;
            }
        }
        true
    }

    /// 全部到期条目（本地与委托 alike），按声明顺序。
    pub fn due<'s>(&'s self, store: &AppMigrationStore) -> Vec<&'s AppMigrationEntry> {
        self.entries
            .iter()
            .filter(|entry| self.is_due(store, entry))
            .collect()
    }

    /// 待外部执行的委托动作 id 列表。
    pub fn delegated_pending(&self, store: &AppMigrationStore) -> Vec<&'static str> {
        self.entries
            .iter()
            .filter(|entry| {
                matches!(entry.action, AppMigrationAction::Delegated) && self.is_due(store, entry)
            })
            .map(|entry| entry.id)
            .collect()
    }

    /// 执行全部到期的本地动作。成功者记账，失败者跳过且不记账（下次启动
    /// 重试）；账本写盘失败同样按失败处理，保证"先记账后执行"不会发生。
    pub fn run_immediate(&self, store: &mut AppMigrationStore) -> AppMigrationRun {
        let due = self
            .entries
            .iter()
            .filter(|entry| {
                matches!(entry.action, AppMigrationAction::Immediate(_))
                    && self.is_due(store, entry)
            })
            .collect::<Vec<_>>();
        let mut run = AppMigrationRun::default();
        for entry in due {
            let AppMigrationAction::Immediate(action) = entry.action else {
                continue;
            };
            let outcome = {
                let context = self.context(store);
                action(&context)
            };
            match outcome.and_then(|()| store.mark_completed(entry.id, self.current_version)) {
                Ok(()) => run.executed.push(entry.id),
                Err(error) => run.failed.push((entry.id, error)),
            }
        }
        run
    }

    /// 为委托动作记账（外部执行者完成动作后调用）。未知 id 返回 `false`
    /// 且不落盘。
    pub fn mark_completed(&self, store: &mut AppMigrationStore, id: &str) -> Result<bool> {
        if self.find(id).is_none() {
            return Ok(false);
        }
        store.mark_completed(id, self.current_version)?;
        Ok(true)
    }
}

/// 宽松的数字段版本比较：逐段按数值比较，缺段按 0，`v` 前缀与段内的
/// 非数字后缀（如 `2-beta` 的 `-beta`）被忽略，无法解析的段按 0 处理。
/// 应用按 `x.y.z` 发布，后缀不参与定序。
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn segments(version: &str) -> Vec<u64> {
        version
            .trim()
            .trim_start_matches(['v', 'V'])
            .split('.')
            .map(|segment| {
                let digits = segment
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>();
                digits.parse().unwrap_or(0)
            })
            .collect()
    }

    let (sa, sb) = (segments(a), segments(b));
    for index in 0..sa.len().max(sb.len()) {
        let left = sa.get(index).copied().unwrap_or(0);
        let right = sb.get(index).copied().unwrap_or(0);
        match left.cmp(&right) {
            Ordering::Equal => continue,
            ordering => return ordering,
        }
    }
    Ordering::Equal
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AppMigrationData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_run_version: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    completed: BTreeMap<String, String>,
}

/// 迁移账本：一个 JSON 文件（或纯内存，供测试与临时场景使用）。
pub struct AppMigrationStore {
    path: Option<PathBuf>,
    data: AppMigrationData,
}

impl AppMigrationStore {
    /// 纯内存账本，不落盘。
    pub fn in_memory() -> Self {
        Self {
            path: None,
            data: AppMigrationData::default(),
        }
    }

    /// 从文件加载账本；文件缺失或内容损坏时回退为空账本（自愈语义，
    /// 与读取侧宽容、写入侧规范的应用约定一致）。
    pub fn load(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        Ok(Self {
            path: Some(path),
            data,
        })
    }

    /// 上次运行的应用版本；`None` 表示首次安装（或账本刚创建）。
    pub fn last_run_version(&self) -> Option<&str> {
        self.data.last_run_version.as_deref()
    }

    /// 仅在还没有上次运行版本时补种一个（例如升级后账本刚创建，由调用
    /// 方把升级前的版本告知进来）。不落盘，需随后 [`Self::save`] 或
    /// [`Self::record_last_run_version`]。返回是否发生了补种。
    pub fn seed_last_run_version(&mut self, version: &str) -> bool {
        if self.data.last_run_version.is_some() {
            return false;
        }
        self.data.last_run_version = Some(version.to_string());
        true
    }

    /// 记录本次运行的应用版本并落盘。调用时机由应用决定 —— 建议在所有
    /// 到期动作（含委托动作）都已了结后再记录，否则中途崩溃会让下次
    /// 启动误判"升级早已跨过"而跳过未完成的动作。
    pub fn record_last_run_version(&mut self, version: &str) -> Result<()> {
        self.data.last_run_version = Some(version.to_string());
        self.save()
    }

    /// 某动作的执行记录（执行时的应用版本）；未执行过为 `None`。
    pub fn completed_version(&self, id: &str) -> Option<&str> {
        self.data.completed.get(id).map(String::as_str)
    }

    /// 记账并落盘；内存账本为空操作。
    fn mark_completed(&mut self, id: &str, version: &str) -> Result<()> {
        self.data
            .completed
            .insert(id.to_string(), version.to_string());
        self.save()
    }

    /// 落盘（内存账本为空操作）。
    pub fn save(&self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let raw = serde_json::to_string_pretty(&self.data)
            .with_context(|| "Failed to serialize app migration store")?;
        fs::write(path, raw).with_context(|| "Failed to write app migration store")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_versions_orders_numeric_segments() {
        assert_eq!(compare_versions("0.5.2", "0.5.2"), Ordering::Equal);
        assert_eq!(compare_versions("0.5.10", "0.5.2"), Ordering::Greater);
        assert_eq!(compare_versions("0.5", "0.5.0"), Ordering::Equal);
        assert_eq!(compare_versions("v1.0.0", "0.9.9"), Ordering::Greater);
        assert_eq!(compare_versions("0.5.2-beta", "0.5.2"), Ordering::Equal);
        assert_eq!(compare_versions("dev", "0.0.1"), Ordering::Less);
    }
}
