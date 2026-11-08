/// 单个版本化类型的静态元数据，由 `#[version]` 宏以 `__HIFUMI_META`
/// 常量形式生成在每个类型上。
///
/// `hifumi::registry!` 读取这些常量做编译期兼容性断言并汇总成
/// `spec()`；工具链（如 TS runner 导出）以 `spec()` 为唯一事实源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeMeta {
    /// 类型名（宏展开处的标识符原文）。
    pub type_name: &'static str,
    /// 全部版本，旧 → 新（含当前版本）。
    pub versions: &'static [&'static str],
    /// 整条迁移链是否完全可逆（每步免费可逆或声明了 `back`）。
    pub reversible: bool,
    /// 是否带 `forward` 旗标（未知版本按宽容读者解析）。
    pub forward: bool,
    /// 压缩底线版本：`Some(v)` 时比 `v` 更老的版本已灭绝，`versions`
    /// 只含 `v` → 当前；`None` = 未压缩。
    pub baseline: Option<&'static str>,
}

#[doc(hidden)]
pub const fn registry_assert_reversible(meta: &TypeMeta) {
    assert!(
        meta.reversible,
        "hifumi registry: `policy: backward` requires every migration step reversible \
         (add a `back:` converter or avoid lossy steps)"
    );
}

#[doc(hidden)]
pub const fn registry_assert_forward(meta: &TypeMeta) {
    assert!(
        meta.forward,
        "hifumi registry: `policy: forward` requires the type to be declared with \
         `#[version(.., forward)]`"
    );
}

/// 声明 crate 级版本化类型注册表，并在编译期断言其兼容性策略。
///
/// 语法（建议放在专用模块内展开，`spec()` 会成为该模块的项）：
///
/// ```ignore
/// mod versions {
///     hifumi::registry! {
///         policy: backward { Doc, Config }
///     }
/// }
/// // versions::spec() -> &'static [hifumi::TypeMeta]
/// ```
///
/// 策略：
/// - `backward`：每个登记类型整链可逆（降级写不丢数据）；
/// - `forward`：每个登记类型带 `forward` 旗标（旧读者可读新数据）；
/// - `full`：两者兼备。
///
/// 断言在 `const` 上下文求值——违反策略直接编译失败，运行期零开销。
#[macro_export]
macro_rules! registry {
    (policy: backward { $($ty:path),* $(,)? }) => {
        $(const _: () = $crate::registry_assert_reversible(&<$ty>::__HIFUMI_META);)*
        $crate::__hifumi_registry_spec!([$($ty),*]);
    };
    (policy: forward { $($ty:path),* $(,)? }) => {
        $(const _: () = $crate::registry_assert_forward(&<$ty>::__HIFUMI_META);)*
        $crate::__hifumi_registry_spec!([$($ty),*]);
    };
    (policy: full { $($ty:path),* $(,)? }) => {
        $(const _: () = {
            $crate::registry_assert_reversible(&<$ty>::__HIFUMI_META);
            $crate::registry_assert_forward(&<$ty>::__HIFUMI_META);
        };)*
        $crate::__hifumi_registry_spec!([$($ty),*]);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __hifumi_registry_spec {
    ([$($ty:path),*]) => {
        /// 登记类型的元数据快照（由 `hifumi::registry!` 生成）。
        pub fn spec() -> &'static [$crate::TypeMeta] {
            &[ $(<$ty>::__HIFUMI_META,)* ]
        }
    };
}
