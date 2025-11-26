# Hifumi Copilot Instructions

## 项目概述

Hifumi 是一个 Rust 序列化库，专门用于处理不同版本之间的数据迁移。它通过过程宏（proc-macro）提供了一种声明式的方式来定义数据结构的版本历史和迁移规则。

项目名称来源于游戏《蔚蓝档案》(Blue Archive) 中的角色 [Hifumi](https://bluearchive.wiki/wiki/hifumi)。

## 项目结构

```
hifumi/
├── Cargo.toml              # Workspace 配置
├── packages/
│   ├── macros/             # hifumi-macros: 过程宏实现
│   │   └── src/
│   │       ├── lib.rs      # 宏入口点
│   │       ├── template/   # 代码生成模板
│   │       ├── tools/      # 解析工具
│   │       └── utils/      # 工具函数
│   └── types/              # hifumi: 主 crate，导出宏和 trait
│       ├── src/lib.rs      # MigrateInto 和 Versioned trait
│       └── tests/          # 集成测试
```

## 核心概念

### 版本化数据结构

使用 `#[version("x.y")]` 属性宏标记当前版本：

```rust
#[version("0.2")]
#[derive(Debug, Clone, PartialEq)]
struct MyData {
    field: Type,
}
```

### 迁移规则

使用 `#[migration("from" => "to" { ... })]` 定义版本间的迁移：

```rust
#[migration("0.1" => "0.2" {
    + new_field: Type,           // 添加字段
    - old_field: Type,           // 删除字段
    old => new: Type,            // 重命名字段
    field: OldType => NewType { converter }, // 类型转换
    + (a: T1, b: T2) => c: T3 { (a, b) => ... }, // 合并字段
})]
```

### 迁移字段操作符

| 操作符 | 语法 | 说明 |
|--------|------|------|
| `+` | `+ field: Type` | 添加新字段（使用 Default） |
| `+` | `+ field: Type { expr }` | 添加新字段（使用表达式） |
| `-` | `- field: Type` | 删除字段 |
| (无) | `old => new: Type` | 重命名字段 |
| (无) | `field: OldType => NewType { converter }` | 转换字段类型 |
| `+` | `+ src => dst: Type` | 复制字段 |
| (无) | `(a: T, b: T) => c: T { converter }` | 合并多字段 |

## 代码生成

宏会生成以下内容：

1. **版本化结构体** - 每个历史版本的内部结构体（带 `#[doc(hidden)]`）
2. **外部枚举** - 用于 serde 反序列化的带 `$version` 标签的枚举
3. **From 实现** - 每个旧版本到新版本的 From trait 实现
4. **Serialize/Deserialize** - 自定义序列化实现，自动添加版本标签

## 开发指南

### 添加新功能

1. 如果涉及新的迁移操作符，修改 `packages/macros/src/tools/migration_field.rs`
2. 如果涉及代码生成，修改 `packages/macros/src/template/` 下的相应模块
3. 添加测试到 `packages/types/tests/`

### 代码规范

- 使用 `anyhow::Result` 进行错误处理
- 使用 `quote!` 宏生成 TokenStream
- 使用 `syn` 解析 Rust 语法
- 生成的代码应添加 `#[doc(hidden)]` 和 `#[allow(...)]` 以避免警告
- 使用 `BTreeMap` 保持字段顺序一致

### 版本标识符生成

使用 `sqids` 库将版本字符串转换为合法的 Rust 标识符：

```rust
// "0.1" -> "_StructName_xxxxxxxxxxxx"
generate_ident(&ident, version)
```

### 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test -p hifumi add_field
```

## 依赖

- `syn` / `proc-macro2` / `quote` - 过程宏基础设施
- `serde` - 序列化框架
- `sqids` - 标识符生成
- `anyhow` - 错误处理

## TODO 实现指南

### Support rs_ts (已完成)

通过现有的 `extra_macros` 机制，hifumi 已经支持透传 `#[derive(specta::Type)]` 属性。用户只需：
- 添加 `specta` 依赖
- 在 derive 列表中添加 `Type`

### Support yuuka (需要设计)

[yuuka](https://github.com/celestia-island/yuuka) 使用完全不同的宏系统（过程宏 `derive_struct!` 而非属性宏），直接集成比较困难。

**可能的方案**：
1. **桥接宏**：创建一个新的宏，允许在 yuuka 的 `derive_struct!` 内部使用 hifumi 的版本化功能
2. **代码生成**：提供一个工具从 hifumi 结构体生成 yuuka 兼容的代码
3. **运行时兼容**：让两者生成的结构体在运行时兼容（通过 serde）

目前建议用户：
- 使用 hifumi 进行版本化数据迁移
- 单独使用 yuuka 进行配置管理
- 两者通过 serde 序列化进行数据交换

### Version field auto-detection (已完成)

`#[version]` 现在支持无参数调用，将自动使用 `CARGO_PKG_VERSION` 环境变量：

```rust
#[version]  // 等同于 #[version(env!("CARGO_PKG_VERSION"))]
struct MyConfig { ... }
```

### Git history migration generation (已完成)

已创建 `hifumi-cli` crate 提供 CLI 工具：

```bash
# 分析两个 commit 之间的结构体变化
hifumi analyze -f src/models.rs -s MyStruct --from HEAD~1 --to HEAD

# 生成迁移代码
hifumi generate -f src/models.rs -s MyStruct \
  --from-version "0.1" --to-version "0.2" \
  --from-commit HEAD~1 --to-commit HEAD
```

**实现细节**：
- `packages/cli/src/analyze.rs` - 分析 git 历史中的结构体变化
- `packages/cli/src/generate.rs` - 生成迁移代码
- 使用 `git2` 库读取 git 历史
- 使用 `syn` 解析 Rust 源代码
- 检测字段添加、删除、重命名和类型变化
