// Lang-Zone 编译器 — ir/codegen/hide_ty.rs
// 隐藏类型标注开关：控制 Rust 后端生成的 .rs 文件中局部 let 绑定是否省略类型标注。
//
// 规则：
//   - 默认关闭：产物与既有 golden/快照逐字节不变
//   - CLI --hide-type-annotations > LZ_HIDE_TYPE_ANNOTATIONS=1 > 缺省关闭
//   - 仅省略「类型语义上可安全 rustc 推断」的标注；空容器 Vec<_>, Option<i64>,
//     Result ok 解包, ref 绑定, Arc<dyn Fn> 等有类型语义作用的标注不受影响

use std::sync::atomic::{AtomicBool, Ordering};

static HIDE_TYPE_ANNOTATIONS: AtomicBool = AtomicBool::new(false);
static RELEASE_MODE: AtomicBool = AtomicBool::new(false);
static HIDE_MODULE_META: AtomicBool = AtomicBool::new(false);
static HIDE_RETURN_KEYWORD: AtomicBool = AtomicBool::new(false);

/// 供 CLI `--hide-type-annotations` 调用：开启隐藏类型标注。
pub fn set_hide_type_annotations(on: bool) {
    HIDE_TYPE_ANNOTATIONS.store(on, Ordering::Relaxed);
}

/// 隐藏类型标注是否开启：`--hide-type-annotations` > `LZ_HIDE_TYPE_ANNOTATIONS=1` > 缺省关闭。
pub fn hide_type_annotations_str() -> bool {
    if HIDE_TYPE_ANNOTATIONS.load(Ordering::Relaxed) {
        return true;
    }
    matches!(
        std::env::var("LZ_HIDE_TYPE_ANNOTATIONS").as_deref(),
        Ok("1") | Ok("on") | Ok("true")
    )
}

/// 供 CLI `--hide-module-meta` 调用：隐藏模块元数据常量（__name__ 等）。
pub fn set_hide_module_meta(on: bool) {
    HIDE_MODULE_META.store(on, Ordering::Relaxed);
}

/// 隐藏模块元数据是否开启：`--hide-module-meta` > `LZ_HIDE_MODULE_META=1` > 缺省关闭。
pub fn hide_module_meta() -> bool {
    if HIDE_MODULE_META.load(Ordering::Relaxed) {
        return true;
    }
    matches!(
        std::env::var("LZ_HIDE_MODULE_META").as_deref(),
        Ok("1") | Ok("on") | Ok("true")
    )
}

/// 供 CLI `--hide-return-keyword` 调用：隐藏显式 return 关键字（尾表达式裸写）。
pub fn set_hide_return_keyword(on: bool) {
    HIDE_RETURN_KEYWORD.store(on, Ordering::Relaxed);
}

/// 隐藏显式 return 是否开启：`--hide-return-keyword` > `LZ_HIDE_RETURN_KEYWORD=1` > 缺省关闭。
pub fn hide_return_keyword() -> bool {
    if HIDE_RETURN_KEYWORD.load(Ordering::Relaxed) {
        return true;
    }
    matches!(
        std::env::var("LZ_HIDE_RETURN_KEYWORD").as_deref(),
        Ok("1") | Ok("on") | Ok("true")
    )
}

/// 供 CLI `--release` 调用：在 rustc 调用中追加优化参数。
pub fn set_release_mode(on: bool) {
    RELEASE_MODE.store(on, Ordering::Relaxed);
}

/// release 模式是否开启：`--release` > `LZ_RELEASE=1` > 缺省关闭。
pub fn release_mode() -> bool {
    if RELEASE_MODE.load(Ordering::Relaxed) {
        return true;
    }
    matches!(
        std::env::var("LZ_RELEASE").as_deref(),
        Ok("1") | Ok("on") | Ok("true")
    )
}
