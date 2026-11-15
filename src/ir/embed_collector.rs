// Lang-Zone 编译器 — ir/embed_collector.rs
// #[embed(lang)] 原样块收集器：按语言分组 → 同名文件聚合
//
// foo.lz 内多个 #[embed(tnr)] 块 → 收集到 foo.tnr（按出现顺序拼接）
// → tnr lib 转译为 foo_tnr.rs（Rust 模块）→ LZ codegen 生成 use foo_tnr::*;

use std::collections::HashMap;

use super::node::Item;
use super::IrModule;

pub struct EmbedCollector {
    blocks: HashMap<String, Vec<String>>,
}

impl EmbedCollector {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
        }
    }

    pub fn collect(&mut self, module: &IrModule) {
        for item in &module.items {
            if let Item::EmbedBlock { lang, src, .. } = item {
                self.blocks
                    .entry(lang.clone())
                    .or_insert_with(Vec::new)
                    .push(src.clone());
            }
        }
    }

    pub fn files(&self) -> HashMap<String, String> {
        self.blocks
            .iter()
            .map(|(lang, blocks)| (lang.clone(), blocks.join("\n\n")))
            .collect()
    }

    pub fn module_name(source_name: &str, lang: &str) -> String {
        let base = source_name
            .rsplit('/')
            .next()
            .unwrap_or(source_name)
            .trim_end_matches(".lz");
        format!("{}_{}", base, lang)
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// 返回所有 embed 生成的模块名（如 `["foo_tnr", "foo_rust"]`）。
    pub fn module_names(&self, source_name: &str) -> Vec<String> {
        self.blocks
            .keys()
            .map(|lang| Self::module_name(source_name, lang))
            .collect()
    }
}

impl Default for EmbedCollector {
    fn default() -> Self {
        Self::new()
    }
}
