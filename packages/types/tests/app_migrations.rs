use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
};

use anyhow::Result;
use hifumi::{
    app::{AppMigrationAction, AppMigrationStore},
    app_migrations,
};

/// 覆盖各种声明形态的样例注册表。
#[app_migrations("0.5.2")]
mod sample {
    use anyhow::Result;
    use hifumi::app::AppMigrationContext;

    #[once("0.5.2")]
    fn plain(_ctx: &AppMigrationContext) -> Result<()> {
        Ok(())
    }

    #[once("0.5.0" => "0.6.0")]
    fn floored(_ctx: &AppMigrationContext) -> Result<()> {
        Ok(())
    }

    #[once("0.5.2", delegate)]
    fn webview_side() {}

    #[allow(dead_code)]
    fn helper() {}
}

/// 来源版本下限需要当前版本已越过 `since` 才有判定意义。
#[app_migrations("0.6.0")]
mod later {
    use anyhow::Result;
    use hifumi::app::AppMigrationContext;

    #[once("0.5.0" => "0.6.0")]
    fn action(_ctx: &AppMigrationContext) -> Result<()> {
        Ok(())
    }
}

/// 版本参数缺省时取被标注 crate 的 `CARGO_PKG_VERSION`。
#[app_migrations]
mod auto {
    use anyhow::Result;
    use hifumi::app::AppMigrationContext;

    #[once("0.5.2")]
    fn action(_ctx: &AppMigrationContext) -> Result<()> {
        Ok(())
    }
}

static FLAKY_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

/// 首次执行失败、重试成功的动作，用于验证失败不记账、下次重试。
#[app_migrations("0.5.2")]
mod flaky {
    use anyhow::{anyhow, Result};
    use std::sync::atomic::Ordering;

    use hifumi::app::AppMigrationContext;

    #[once("0.5.2")]
    fn action(_ctx: &AppMigrationContext) -> Result<()> {
        if super::FLAKY_ATTEMPTS.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(anyhow!("first attempt fails"));
        }
        Ok(())
    }
}

static TEMP_FILE_SEQ: AtomicUsize = AtomicUsize::new(0);

fn temp_store_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "hifumi-app-migrations-{}-{}.json",
        std::process::id(),
        TEMP_FILE_SEQ.fetch_add(1, AtomicOrdering::SeqCst)
    ))
}

fn store_with_previous(version: &str) -> AppMigrationStore {
    let mut store = AppMigrationStore::in_memory();
    assert!(store.seed_last_run_version(version));
    store
}

#[test]
fn registry_collects_once_entries() {
    let set = sample::registry();
    assert_eq!(set.current_version, "0.5.2");
    assert_eq!(set.entries.len(), 3);

    assert_eq!(set.entries[0].id, "plain");
    assert_eq!(set.entries[0].since, "0.5.2");
    assert_eq!(set.entries[0].from, None);
    assert!(matches!(
        set.entries[0].action,
        AppMigrationAction::Immediate(_)
    ));

    assert_eq!(set.entries[1].id, "floored");
    assert_eq!(set.entries[1].since, "0.6.0");
    assert_eq!(set.entries[1].from, Some("0.5.0"));

    assert_eq!(set.entries[2].id, "webview_side");
    assert_eq!(set.entries[2].action, AppMigrationAction::Delegated);

    assert!(set.find("helper").is_none());
}

#[test]
fn registry_defaults_to_cargo_pkg_version() {
    assert_eq!(auto::registry().current_version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn due_only_when_upgrade_crosses_into_since() {
    let set = sample::registry();

    let plain = set.find("plain").unwrap();
    assert!(!set.is_due(&AppMigrationStore::in_memory(), plain));
    assert!(set.is_due(&store_with_previous("0.5.1"), plain));
    assert!(set.is_due(&store_with_previous("0.5.0"), plain));
    assert!(!set.is_due(&store_with_previous("0.5.2"), plain));
    assert!(!set.is_due(&store_with_previous("0.5.3"), plain));

    // 当前版本 0.5.2 尚未到达 floored 的 0.6.0：任何来源都不到期。
    let floored = set.find("floored").unwrap();
    assert!(!set.is_due(&store_with_previous("0.5.9"), floored));
}

#[test]
fn from_floor_limits_the_upgrade_origin() {
    let set = later::registry();
    let entry = set.find("action").unwrap();

    assert!(set.is_due(&store_with_previous("0.5.0"), entry));
    assert!(set.is_due(&store_with_previous("0.5.9"), entry));
    assert!(!set.is_due(&store_with_previous("0.4.9"), entry));
    assert!(!set.is_due(&AppMigrationStore::in_memory(), entry));
}

#[test]
fn run_immediate_marks_success_and_retries_failure() {
    let set = flaky::registry();
    let mut store = store_with_previous("0.5.1");

    let first = set.run_immediate(&mut store);
    assert!(first.executed.is_empty());
    assert_eq!(first.failed.len(), 1);
    assert_eq!(first.failed[0].0, "action");
    assert!(store.completed_version("action").is_none());

    let second = set.run_immediate(&mut store);
    assert_eq!(second.executed, vec!["action"]);
    assert!(second.failed.is_empty());
    assert_eq!(store.completed_version("action"), Some("0.5.2"));

    let third = set.run_immediate(&mut store);
    assert!(third.executed.is_empty());
    assert!(third.failed.is_empty());
}

#[test]
fn delegated_pending_and_external_completion() -> Result<()> {
    let set = sample::registry();
    let mut store = store_with_previous("0.5.1");

    assert_eq!(set.delegated_pending(&store), vec!["webview_side"]);

    assert!(set.mark_completed(&mut store, "webview_side")?);
    assert!(set.delegated_pending(&store).is_empty());
    assert_eq!(store.completed_version("webview_side"), Some("0.5.2"));

    // 未知 id 不落盘。
    assert!(!set.mark_completed(&mut store, "unknown")?);
    assert!(store.completed_version("unknown").is_none());

    Ok(())
}

#[test]
fn store_persists_across_reloads() -> Result<()> {
    let path = temp_store_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }

    let mut store = AppMigrationStore::load(&path)?;
    assert!(store.seed_last_run_version("0.5.1"));
    store.save()?;

    let mut store = AppMigrationStore::load(&path)?;
    assert_eq!(store.last_run_version(), Some("0.5.1"));

    let set = sample::registry();
    set.mark_completed(&mut store, "webview_side")?;
    store.record_last_run_version("0.5.2")?;

    let store = AppMigrationStore::load(&path)?;
    assert_eq!(store.last_run_version(), Some("0.5.2"));
    assert_eq!(store.completed_version("webview_side"), Some("0.5.2"));

    fs::remove_file(&path)?;
    Ok(())
}

#[test]
fn store_self_heals_on_corrupt_file() -> Result<()> {
    let path = temp_store_path();
    fs::write(&path, "not json")?;

    let store = AppMigrationStore::load(&path)?;
    assert!(store.last_run_version().is_none());
    assert!(store.completed_version("plain").is_none());

    fs::remove_file(&path)?;
    Ok(())
}

#[test]
fn seed_last_run_version_only_when_missing() {
    let mut store = store_with_previous("0.5.1");
    assert!(!store.seed_last_run_version("0.4.0"));
    assert_eq!(store.last_run_version(), Some("0.5.1"));

    let mut fresh = AppMigrationStore::in_memory();
    assert!(fresh.seed_last_run_version("0.4.0"));
    assert_eq!(fresh.last_run_version(), Some("0.4.0"));
}
