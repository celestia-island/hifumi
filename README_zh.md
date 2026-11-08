<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/hifumi/master/docs/logo.webp" alt="Hifumi" width="240" /></p>

![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/celestia-island/hifumi/test.yml)

## 介绍

一个用于在不同版本之间迁移数据的序列化库。

名称 `hifumi` 来自游戏 [Blue Archive](https://bluearchive.jp/) 中的角色 [Hifumi](https://bluearchive.wiki/wiki/hifumi)。

> 仍在开发中，API 未来可能会发生变化。

## 快速开始

```rust
use hifumi::version;

#[version("0.2")]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.1" => "0.2" {
    + (c: i32, d: i32) => e: String { (c + d).to_string() },
    - f: f32,
})]
struct Test {
    a: i32,
    b: i32,
    c: i32,
    d: i32,
    e: String,
}
```

## 特性

### 自动版本检测

你可以在 `#[version]` 中不带参数，自动使用 `CARGO_PKG_VERSION`：

```rust
use hifumi::version;

#[version]  // 自动使用 CARGO_PKG_VERSION
#[derive(Debug, Clone, PartialEq)]
struct Config {
    // ...
}
```

### 导出 TypeScript 类型（specta）

Hifumi 支持 [specta](https://github.com/specta-rs/specta) 用于生成 TypeScript 类型：

```rust
use hifumi::version;
use specta::Type;

#[version("0.1")]
#[derive(Debug, Clone, PartialEq, Type)]  // 添加 Type derive
struct User {
    id: i32,
    name: String,
}

// 导出为 TypeScript
let ts = specta::ts::export::<User>(&Default::default())?;
```

### 应用版本一次性动作

除了迁移数据结构，hifumi 还为"应用自身升级跨入某版本时执行**一次**"的动作提供脚手架 —— 修复、偏好改写，或任何每个档案只应发生一次的事情：

```rust
use hifumi::app_migrations;

#[app_migrations]  // 当前应用版本；缺省取 CARGO_PKG_VERSION
mod app_migrations_registry {
    use anyhow::Result;
    use hifumi::app::AppMigrationContext;

    /// 升级跨入 "0.5.2" 时执行一次，来源版本不限。
    #[once("0.5.2")]
    fn rewrite_config(_ctx: &AppMigrationContext) -> Result<()> {
        // ...
        Ok(())
    }

    /// 仅当从 "0.5.0" 及以后升级跨入 "0.6.0" 时执行。
    #[once("0.5.0" => "0.6.0")]
    fn repair_settings(_ctx: &AppMigrationContext) -> Result<()> {
        // ...
        Ok(())
    }

    /// 不在本地执行：保持待办状态，直到外部执行者（例如 WebView 前端）
    /// 回报完成。函数体应留空。
    #[once("0.5.2", delegate)]
    fn webview_side_action() {}
}
```

宏会在模块内追加一个隐藏的 `registry()`，返回 `AppMigrationSet`。准入判据是"升级跨入了该动作的版本"：`上次版本 < since <= 当前版本`，`上次版本` 来自 JSON 文件账本（`AppMigrationStore`）。首次安装（没有上次版本）不执行任何动作；`from` 版本为来源加下限。

执行成功的动作会被记账且永不重跑；执行失败的动作不记账、下次启动重试 —— 动作应保持幂等。`delegate` 动作由 `set.delegated_pending(&store)` 列出，交由外部执行者完成后通过 `set.mark_completed(&mut store, id)` 回报。

```rust
let set = app_migrations_registry::registry();
let mut store = AppMigrationStore::load(config_dir.join("app-migrations.json"))?;
store.seed_last_run_version(previous_hint); // 仅在账本刚创建时补种上次版本
let run = set.run_immediate(&mut store); // 执行并记录本地动作
let pending = set.delegated_pending(&store); // 交给外部执行者
```

### 降级写回

迁移可以声明反向方向（`back:` 转换器）；`to_version_value` 随即能把任意旧版本的结构体写成 JSON（含 `$version` 标签）。Add、copy、纯 rename 天然可逆（默认值 / 改回原名）；remove 与带转换器的 rename 需要显式 `back` 表达式：

```rust
use hifumi::version;

#[version("0.3")]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.2" => "0.3" {
    - legacy: bool { back: level < 2 },
})]
struct Config {
    name: String,
    level: i32,
}

let older: serde_json::Value = current.to_version_value("0.2")?;
```

### 前向兼容

`#[version("0.2", forward)]` 让类型走两段式反序列化：**更新**版本写出的数据（未知 `$version`、未知字段丢弃）按宽容方式解析——已知字段保留——而不是直接报错。不便改定义的类型可以按值包一层 [`Forward<T>`](packages/types/src/forward.rs) 旁路。注册进 `policy: forward` / `policy: full` 注册表的类型必须有该旗标。

### 带编译期策略的注册表

把版本化类型收进注册表，让编译器证明每个类型都满足其注册的策略——不可逆类型出现在 `policy: backward` 里是编译错误，而不是运行期意外。`policy: full` 要求每个注册类型既可逆又带 forward 旗标：

```rust
mod registry {
    hifumi::registry! {
        policy: full {
            FullDoc,
            LenientDoc,
        }
    }
}

let spec = registry::spec(); // 类型名、版本链、可逆性、forward 旗标
```

### 基线与链压缩

`#[version("0.3", baseline = "0.2")]` 把低于基线的版本标记为灭绝：不再生成它们的结构体、`From` 实现与降级步骤，其 JSON 反序列化为未知变体。`hifumi compact` 原地重写源文件、删除灭绝迁移并保留其余字节：

```bash
hifumi compact -f src/models.rs -s Config --baseline "0.2" --write
```

### 二进制 wire 通道

`hifumi::wire` 把值封帧为 `b"HIFU"` + `type@version` 的 u32 哈希 + postcard 载荷（**最终**结构体明文，wire 上没有按版本区分的枚举）。宏在每个版本化结构体旁生成 `WireCodec` 实现：

```rust
let frame: Vec<u8> = hifumi::wire::to_wire(&doc, &Doc::__HIFUMI_META)?;
let doc: Doc = hifumi::wire::from_wire(&frame, &Doc::__HIFUMI_META)?;
```

## 待办事项

- [x] 支持 `specta` 导出 TypeScript 类型。
- [x] 从 Rust `#[version]` 链导出仅前向的 TypeScript 迁移执行器（`hifumi export-ts`）。
- [x] 支持 `yuuka`（通过基于 serde 的互操作层）。
- [x] 版本字段可以自动使用 crate 版本。
- [x] 从 git 历史自动生成迁移代码。
- [x] 应用版本一次性动作脚手架（`#[app_migrations]` + `#[once]`）。

## 与 Yuuka 的互操作性

Hifumi 提供了与 [yuuka](https://github.com/celestia-island/yuuka) 的基于 serde 的互操作层。由于 yuuka 使用过程宏（`derive_struct!`），而 hifumi 使用属性宏（`#[version]`），深度集成较为困难。因此我们提供了在两者格式之间转换的工具函数：

```rust
use yuuka::derive_struct;
use hifumi::version;
use hifumi_e2e::yuuka_interop::{yuuka_to_hifumi_with_version, hifumi_to_yuuka};

// 定义一个 yuuka 配置结构体
derive_struct!(
    #[derive(serde::Serialize, serde::Deserialize)]
    pub YuukaConfig {
        name: String,
        value: i32,
    }
);

// 定义一个相同字段的版本化 hifumi 结构体
#[version("0.1")]
#[derive(Debug, Clone, PartialEq)]
struct HifumiConfig {
    name: String,
    value: i32,
}

// yuuka -> hifumi
let yuuka_cfg = YuukaConfig { name: "test".into(), value: 42 };
let hifumi_cfg: HifumiConfig = yuuka_to_hifumi_with_version(&yuuka_cfg, "0.1")?;

// hifumi -> yuuka
let back: YuukaConfig = hifumi_to_yuuka(&hifumi_cfg)?;
```

## CLI 工具

安装 CLI：

```bash
cargo install hifumi
```

### 分析结构体变更

```bash
hifumi analyze -f src/models.rs -s MyStruct --from HEAD~1 --to HEAD
```

### 生成迁移代码

```bash
hifumi generate -f src/models.rs -s MyStruct \
  --from-version "0.1" --to-version "0.2" \
  --from-commit HEAD~1 --to-commit HEAD
```

这将输出形如下面的迁移代码：

```rust
#[migration("0.1" => "0.2" {
    + new_field: String,
    - old_field: i32,
    renamed_from => renamed_to: bool,
})]
```

### 导出 TypeScript 迁移执行器

`hifumi export-ts` 读取文件里的所有 `#[version]` 结构体，为每个结构体生成一个自包含、零依赖、仅前向的 TypeScript 迁移执行器。`migrate(doc, from, to)` 沿链升级普通 JSON 对象；降级会抛错。带 Rust 转换器的操作、多来源 add、跨类型 rename、`PathBuf` 类字段会被响亮拒绝，而不是导出一个说谎的执行器：

```bash
hifumi export-ts -f src/models.rs --out migrations.ts
```

### 压缩版本链

从源文件中删除基线以下的灭绝迁移（见[基线与链压缩](#基线与链压缩)）：

```bash
hifumi compact -f src/models.rs -s Config --baseline "0.2" --write
```
