// Lang-Zone 编译器 — comptime/frame.rs
// （由 comptime/mod.rs move-only 拆出，逻辑零改动）

use super::*;

impl<'a> ComptimeContext<'a> {
    pub fn new(module: &'a Module) -> Self {
        ComptimeContext {
            module,
            symtab: HashMap::new(),
            depth: 0,
            source: None,
        }
    }

    pub fn with_source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }

    pub fn enter(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > MAX_COMPTIME_DEPTH {
            return Err(format!(
                "comptime 嵌套深度超限（{} > MAX={}），疑似死循环",
                self.depth - 1,
                MAX_COMPTIME_DEPTH
            ));
        }
        Ok(())
    }

    pub fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }
}
