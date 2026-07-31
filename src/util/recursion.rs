// Lang-Zong 编译器 — util/recursion.rs
// 递归防护：防止策略解析/隐式调用中的无限递归。
//
// 四层防护：
// 1. 栈式活跃策略追踪（同策略+同类型不可重复入栈）
// 2. 深度限制（MAX_STRATEGY_DEPTH = 5）
// 3. 循环检测（策略依赖图环检测）
// 4. @no_strategy 注解支持（由调用方检查）

use std::cell::RefCell;
use std::collections::HashSet;

/// 策略递归防护上下文（每个 resolve 会话一个实例）
///
/// 使用 `RefCell` 内部可变性，使 `try_enter` 可以接受 `&self`，
/// 避免 `EnterGuard` 存活期间阻塞对 `RecursionGuard` 的其他操作。
pub struct RecursionGuard {
    /// 内部状态
    inner: RefCell<Inner>,
}

struct Inner {
    /// 当前活跃策略栈
    active: Vec<StrategyKey>,
    /// 已检测到的循环集合
    cycle_set: HashSet<StrategyKey>,
    /// 当前深度
    depth: usize,
}

/// 策略标识键
#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct StrategyKey {
    /// 策略类型名（例如 "GuardedAction", "ImplicitCopy", "Cast"）
    pub kind: String,
    /// 宿主类型名
    pub type_name: String,
}

/// 策略解析错误
#[derive(Debug, Clone)]
pub enum RecursionError {
    /// 策略已在活跃栈中（同类型重复入栈）
    AlreadyActive(StrategyKey),
    /// 超出最大深度限制
    MaxDepthExceeded(usize),
    /// 检测到策略依赖循环
    CycleDetected(StrategyKey),
}

impl std::fmt::Display for RecursionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecursionError::AlreadyActive(key) => {
                write!(f, "strategy '{}' is already active for type '{}'",
                    key.kind, key.type_name)
            }
            RecursionError::MaxDepthExceeded(d) => {
                write!(f, "strategy recursion depth exceeded (max={})", d)
            }
            RecursionError::CycleDetected(key) => {
                write!(f, "strategy cycle detected for '{}' on type '{}'",
                    key.kind, key.type_name)
            }
        }
    }
}

/// 最大策略解析嵌套深度
pub const MAX_STRATEGY_DEPTH: usize = 5;

impl RecursionGuard {
    /// 创建新的递归防护上下文
    pub fn new() -> Self {
        RecursionGuard {
            inner: RefCell::new(Inner {
                active: Vec::new(),
                cycle_set: HashSet::new(),
                depth: 0,
            }),
        }
    }

    /// 尝试进入一个策略解析。
    /// 成功时返回 `EnterGuard`（Drop 时自动出栈），
    /// 失败时返回 `RecursionError`。
    pub fn try_enter(&self, kind: &str, type_name: &str) -> Result<EnterGuard<'_>, RecursionError> {
        let key = StrategyKey {
            kind: kind.to_string(),
            type_name: type_name.to_string(),
        };

        let mut inner = self.inner.borrow_mut();

        // 1. 栈式查重：同一策略+同一类型不可重复入栈
        if inner.active.contains(&key) {
            return Err(RecursionError::AlreadyActive(key));
        }

        // 2. 深度限制
        if inner.depth >= MAX_STRATEGY_DEPTH {
            return Err(RecursionError::MaxDepthExceeded(inner.depth));
        }

        // 3. 循环检测
        if inner.cycle_set.contains(&key) {
            return Err(RecursionError::CycleDetected(key));
        }

        inner.active.push(key.clone());
        inner.cycle_set.insert(key.clone());
        inner.depth += 1;

        Ok(EnterGuard { guard: self, key })
    }

    /// 获取当前活跃策略数量
    pub fn active_count(&self) -> usize {
        self.inner.borrow().active.len()
    }

    /// 获取当前深度
    pub fn depth(&self) -> usize {
        self.inner.borrow().depth
    }

    /// 检查某策略是否在循环集合中
    pub fn is_in_cycle(&self, kind: &str, type_name: &str) -> bool {
        let key = StrategyKey {
            kind: kind.to_string(),
            type_name: type_name.to_string(),
        };
        self.inner.borrow().cycle_set.contains(&key)
    }

    /// 出栈一个策略（由 EnterGuard::drop 调用）
    fn pop(&self, key: &StrategyKey) {
        let mut inner = self.inner.borrow_mut();
        inner.active.retain(|k| k != key);
        inner.depth = inner.depth.saturating_sub(1);
    }
}

/// Drop 守卫：出栈时自动恢复，无法绕过。
///
/// 持有对 `RecursionGuard` 的不可变引用，通过 `guard.pop()` 出栈。
pub struct EnterGuard<'a> {
    guard: &'a RecursionGuard,
    key: StrategyKey,
}

impl Drop for EnterGuard<'_> {
    fn drop(&mut self) {
        self.guard.pop(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_enter_exit() {
        let guard = RecursionGuard::new();
        assert_eq!(guard.active_count(), 0);

        {
            let _g = guard.try_enter("Cast", "MyType").unwrap();
            assert_eq!(guard.active_count(), 1);
        }
        assert_eq!(guard.active_count(), 0);
    }

    #[test]
    fn test_detect_already_active() {
        let guard = RecursionGuard::new();
        let g1 = guard.try_enter("Cast", "MyType").unwrap();
        let result = guard.try_enter("Cast", "MyType");
        assert!(result.is_err());
        assert!(matches!(result, Err(RecursionError::AlreadyActive(_))));
        drop(g1);
    }

    #[test]
    fn test_different_types_ok() {
        let guard = RecursionGuard::new();
        let g1 = guard.try_enter("Cast", "MyType").unwrap();
        let result = guard.try_enter("Cast", "OtherType");
        assert!(result.is_ok());
        drop(result);
        drop(g1);
    }

    #[test]
    fn test_max_depth() {
        let guard = RecursionGuard::new();
        let mut guards = Vec::new();
        for i in 0..MAX_STRATEGY_DEPTH {
            let key = format!("S{}", i);
            let ty = format!("T{}", i);
            match guard.try_enter(&key, &ty) {
                Ok(g) => guards.push(g),
                Err(e) => panic!("failed at depth {}: {}", i, e),
            }
        }
        let result = guard.try_enter("Overflow", "Overflow");
        assert!(matches!(result, Err(RecursionError::MaxDepthExceeded(_))));
        drop(guards);
    }

    #[test]
    fn test_cycle_detection() {
        let guard = RecursionGuard::new();
        {
            let _g = guard.try_enter("Cast", "MyType").unwrap();
        }
        let result = guard.try_enter("Cast", "MyType");
        assert!(matches!(result, Err(RecursionError::CycleDetected(_))));
    }

    #[test]
    fn test_is_in_cycle() {
        let guard = RecursionGuard::new();
        {
            let _g = guard.try_enter("Test", "T").unwrap();
        }
        assert!(guard.is_in_cycle("Test", "T"));
        assert!(!guard.is_in_cycle("Test", "U"));
    }

    #[test]
    fn test_guard_drop_restores_depth() {
        let guard = RecursionGuard::new();
        assert_eq!(guard.depth(), 0);
        {
            let _g = guard.try_enter("A", "T").unwrap();
            assert_eq!(guard.depth(), 1);
        }
        assert_eq!(guard.depth(), 0);
    }

    #[test]
    fn test_active_count_parallel_strategies() {
        let guard = RecursionGuard::new();
        let g1 = guard.try_enter("Cast", "T").unwrap();
        let g2 = guard.try_enter("ImplicitCopy", "T").unwrap();
        assert_eq!(guard.active_count(), 2);
        drop(g1);
        drop(g2);
    }
}
