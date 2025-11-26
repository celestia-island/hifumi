<img src="splash.png" alt="hifumi" />

![Crates.io License](https://img.shields.io/crates/l/hifumi)
[![Crates.io Version](https://img.shields.io/crates/v/hifumi)](https://docs.rs/hifumi)
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

## 待办事项

- [x] 支持 `specta` 导出 TypeScript 类型。
- [x] 支持 `yuuka`（通过基于 serde 的互操作层）。
- [x] 版本字段可以自动使用 crate 版本。
- [x] 从 git 历史自动生成迁移代码。

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
cargo install hifumi-cli
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
