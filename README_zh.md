<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/celestia-island.github.io/main/res/logos/hifumi.webp" alt="Hifumi" width="240" /></p>

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

### 版本身份

`#[version]` 的身份是**一个字符串字面量，逐字节比对**。它不解析、不规范
化、也不排序：

- `#[version("0.2")]` 与 `#[version("0.2.0")]` 是**两个不同的身份**。以
  `{"$version":"0.2.0", ...}` 写出的数据，遇到声明为 `"0.2"` 的类型不会匹配
  ——它会以「未知变体」失败（类型带 `forward` 时则按宽容读者处理）。
- 裸 `#[version]` 取 `CARGO_PKG_VERSION` 的原样值，即**完整的**
  `major.minor.patch`。没有任何截断到 `major.minor` 的步骤，因此裸 `#[version]`
  的类型与手写 `#[version("0.1")]` 的类型即使在同一发布版本上也是不同身份。
- 迁移步是 `"from" => "to"` 字符串对，从当前版本起按**精确相等**回溯。**链本身
  不假设任何顺序**：它是位置性的，不是语义性的，且只认 `to == current` 往回收，
  因此各步的版本必须真的接得上。接不上的步不是「顺序不对」，而是根本不可达
  ——`"0.2"` 却写了 `"0.1" => "9.9"` 时链上没有任何 `"9.9"` 步，`"0.1"` 压根不在
  链里（`to_version_value("0.1")` 报 `UnknownTarget`）。
- **`hifumi::app` 的动作定序是另一回事，而且容不下文字版本。** `compare_versions`
  （`#[once(...)]` 的 `from => since` 下界用它）把每个点分段按数字解析，解析不出
  数字的段一律当 `0`。于是 `"a"` 与 `"b"` 都是 `0`、比较结果为 `Equal`，写成
  `"alpha" => "beta"` 的动作永远不会到期。纯数字版本定序正常（`"0.1"` 对 `"9.9"`
  是 `Less`）；**要用版本串给 `#[once]` 划界，就请用数字身份。**
- wire 通道用 FNV-1a 哈希 `type_name@version`，因此改身份就是改 wire 兼容性。

**建议**：新链一律使用完整 semver 字符串（`"0.1.0"`、`"1.2.3"`），让身份保持
版本号本身所暗示的兼容性形状，并与 `CARGO_PKG_VERSION` 可比。既有链没有义务改
——它们的身份已经写进了落盘数据里，改写是数据迁移而非改名。

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

### 字段属性

`#[version]` 自己写出 `Serialize`/`Deserialize` 实现，因此你不能在结构体上再
`#[derive(Serialize)]`（E0119）。字段级 `#[serde(...)]` 属性留在当前版本的结构体上，
并由宏按名字搬运到隐藏的按版本结构体，`#[serde(default)]` 因此按预期生效：

```rust
use hifumi::version;

#[version("0.2")]
#[derive(Debug, Clone, PartialEq)]
struct Config {
    name: String,
    #[serde(default)]
    retries: u32,
}

// 缺少 retries 的载荷现在可以解析。
let config: Config = serde_json::from_str(r#"{"$version":"0.2","name":"a"}"#)?;
```

搬运过去的属性作用在按版本的**明文**表示上（即真正携带字段的那份），
`to_version_value` 写出的与 wire 通道编码的也是同一份。三条规则值得知道：

- 只有旧版本里**与今天同名且同型**的字段才会继承属性。跨步骤换了类型的字段
  （`n: i32 => n: String`）是另一个字段，不继承——否则 `#[serde(default)]` 会
  悄悄把旧版本一个必需字段变成可选。比较对象是**当前**类型（对每一份旧版本
  结构体都一样），不是逐跳比较；而且比的是类型写法的字面文本：一个连着几版
  都没变的字段，只要在之后任一跳改了类型就不再继承，`std::vec::Vec<T>` 与
  `Vec<T>` 也算不同类型。这两处近似都偏安全（旧字段只会保持必需）。
- 字段上的 `#[serde(flatten)]` 与 `#[cfg]` 会**以编译错误拒绝**：`$version` 是
  内部标签枚举，其载荷不能 flatten；而宏无法在每一份按版本结构体上重现同一
  组 cfg 条件。
- 属性描述的是**当前** schema。历史版本的 wire 键无法从今天的源码恢复，因此
  事后补上的 `#[serde(rename)]` 不改变已经写出去的数据。

另注意 `#[version(.., forward)]` 改的是**哪些载荷能解析**，不是「载荷坏不坏」。
机制上，生成的 `Deserialize` 对**任何**版本化解码失败（`Err(_)`）都会回退，再按
当前版本重读载荷：未知字段被丢弃、版本标签被忽略。对**只加字段**的链，这在可观察
行为上等同于「只有版本定不下来才回退」——载荷声明了一个你**认识**的版本、却不符
那个版本自己的形状时，落到当前版本的形状上，而当前版本要求的只会更多，于是以
同样的方式失败。一旦某步让当前版本**要求更少**，就不再等同：`Remove`（或丢掉旧
字段名的 `Rename`）之后，一个缺了被删字段的旧版本载荷在 `forward` 下会被**接受**
（按当前版本重读，它不再要这个字段），不带 `forward` 则被拒绝。这是 `forward`
唯一可观察的宽化——若你的链删过字段，一个损坏或截断的旧载荷会被读成当前版本而
不是报错。两种情况都不容忍的是：已知字段类型错了，哪怕版本未知也报
`invalid type: string "not-an-int", expected i32`；当前版本要求的字段缺了仍然报
`missing field`。见 [前向兼容](#前向兼容)。

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
