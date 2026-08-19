// Lang-Zong 编译器 — bridge/ledger.rs
// 调用台账（Ledger）：bridge 调用轨迹的追加式 TSV 记录与审计
//
// 格式（bridge-ledger.tsv，TSV 追加式）：
//   ts \t event \t lang \t detail
//   event ∈ { REGISTER, CALL, EXPORT, ERROR }
//
// 设计对齐方案.md G5：调用台账缺失 → bridge-ledger.tsv 追加式 + 审计
// （对齐 Tnr 审计风格：只追加、不覆盖、可审计）。

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 单条台账记录
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerRecord {
    pub ts: String,      // 时间戳（unix 毫秒）
    pub event: String,   // REGISTER / CALL / EXPORT / ERROR
    pub lang: String,    // rust / python / cy / ...
    pub detail: String,  // 符号名/参数摘要/错误信息
}

impl LedgerRecord {
    pub fn new(event: impl Into<String>, lang: impl Into<String>, detail: impl Into<String>) -> Self {
        LedgerRecord {
            ts: now_millis(),
            event: event.into(),
            lang: lang.into(),
            detail: detail.into(),
        }
    }

    /// 序列化为 TSV 行（不含换行）
    pub fn to_tsv(&self) -> String {
        format!("{}\t{}\t{}\t{}", self.ts, self.event, self.lang, self.detail)
    }

    /// 从 TSV 行解析
    pub fn from_tsv(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.trim().split('\t').collect();
        if parts.len() < 4 { return None; }
        Some(LedgerRecord {
            ts: parts[0].to_string(),
            event: parts[1].to_string(),
            lang: parts[2].to_string(),
            detail: parts[3..].join("\t"),
        })
    }
}

/// 调用台账：内存缓冲 + 追加式 TSV 落盘
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    /// 落盘路径（None = 仅内存，不写文件）
    path: Option<PathBuf>,
    /// 内存缓冲（按序追加）
    records: Vec<LedgerRecord>,
}

impl Ledger {
    pub fn new() -> Self {
        Ledger { path: None, records: Vec::new() }
    }

    /// 设置台账落盘路径；文件不存在时创建（含父目录），已存在时**追加**。
    pub fn set_path(&mut self, path: impl Into<PathBuf>) -> std::io::Result<()> {
        let p = path.into();
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        if !p.exists() {
            // 写入表头
            let mut f = OpenOptions::new().create(true).append(true).open(&p)?;
            writeln!(f, "ts\tevent\tlang\tdetail")?;
        }
        self.path = Some(p);
        Ok(())
    }

    /// 追加一条记录（写内存 + 追加落盘）
    pub fn append(&mut self, event: impl Into<String>, lang: impl Into<String>, detail: impl Into<String>) {
        let rec = LedgerRecord::new(event, lang, detail);
        self.records.push(rec.clone());
        if let Some(p) = &self.path {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
                let _ = writeln!(f, "{}", rec.to_tsv());
            }
        }
    }

    /// 追加导出事件（codegen 登记 @export 时调用）
    pub fn record_export(&mut self, lang: impl Into<String>, detail: impl Into<String>) {
        self.append("EXPORT", lang, detail);
    }

    /// 追加错误事件
    pub fn record_error(&mut self, lang: impl Into<String>, detail: impl Into<String>) {
        self.append("ERROR", lang, detail);
    }

    /// 当前内存缓冲记录数
    pub fn len(&self) -> usize { self.records.len() }

    pub fn is_empty(&self) -> bool { self.records.is_empty() }

    /// 所有记录（内存缓冲）
    pub fn records(&self) -> &[LedgerRecord] { &self.records }

    /// 从已落盘的 TSV 文件读取全部记录（审计用）
    pub fn read_from_disk(&self) -> Vec<LedgerRecord> {
        match &self.path {
            Some(p) => Self::read_path(p),
            None => self.records.clone(),
        }
    }

    /// 从指定路径读取全部记录
    pub fn read_path(path: &Path) -> Vec<LedgerRecord> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        content.lines()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with("ts\t"))
            .filter_map(LedgerRecord::from_tsv)
            .collect()
    }

    /// 审计汇总：按 event × lang 计数
    pub fn report(&self) -> LedgerReport {
        Self::report_from(&self.read_from_disk())
    }

    /// 从指定路径读取全部记录并审计汇总（CLI `emit-bridge-report` 入口）
    pub fn report_path(path: &Path) -> LedgerReport {
        Self::report_from(&Self::read_path(path))
    }

    fn report_from(recs: &[LedgerRecord]) -> LedgerReport {
        let mut events: HashMap<(String, String), usize> = HashMap::new();
        let mut total = 0usize;
        for rec in recs {
            *events.entry((rec.event.clone(), rec.lang.clone())).or_insert(0) += 1;
            total += 1;
        }
        LedgerReport { total, events }
    }
}

/// 审计汇总结果
#[derive(Debug, Clone, Default)]
pub struct LedgerReport {
    pub total: usize,
    /// (event, lang) → count
    pub events: HashMap<(String, String), usize>,
}

impl LedgerReport {
    /// 格式化为可读文本
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("bridge ledger report: {} records\n", self.total));
        let mut keys: Vec<&(String, String)> = self.events.keys().collect();
        keys.sort();
        for k in keys {
            out.push_str(&format!("  {} / {}: {}\n", k.0, k.1, self.events[k]));
        }
        out
    }
}

fn now_millis() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ledger_record_tsv_roundtrip() {
        let rec = LedgerRecord::new("CALL", "python", "numpy_array n=3");
        let tsv = rec.to_tsv();
        let parsed = LedgerRecord::from_tsv(&tsv).unwrap();
        assert_eq!(parsed.event, "CALL");
        assert_eq!(parsed.lang, "python");
        assert_eq!(parsed.detail, "numpy_array n=3");
        assert_eq!(parsed.ts, rec.ts);
    }

    #[test]
    fn test_ledger_append_memory() {
        let mut l = Ledger::new();
        assert!(l.is_empty());
        l.append("REGISTER", "python", "symbol=numpy_array");
        l.append("CALL", "python", "numpy_array n=3");
        assert_eq!(l.len(), 2);
        assert_eq!(l.records()[0].event, "REGISTER");
        assert_eq!(l.records()[1].event, "CALL");
    }

    #[test]
    fn test_ledger_disk_append_and_report() {
        let dir = crate::util::TempDir::new("lz-ledger").unwrap();
        let path = dir.path().join("bridge-ledger.tsv");
        let mut l = Ledger::new();
        l.set_path(&path).unwrap();
        l.append("REGISTER", "python", "symbol=numpy_array");
        l.append("CALL", "python", "numpy_array n=3");
        l.append("EXPORT", "rust", "fn hello");

        // 重新打开（模拟第二次运行）验证追加式
        let mut l2 = Ledger::new();
        l2.set_path(&path).unwrap();
        l2.append("CALL", "cy", "omega_gate");

        let report = l2.report();
        assert_eq!(report.total, 4);
        assert_eq!(report.events.get(&("CALL".to_string(), "python".to_string())), Some(&1));
        assert_eq!(report.events.get(&("CALL".to_string(), "cy".to_string())), Some(&1));
        assert_eq!(report.events.get(&("REGISTER".to_string(), "python".to_string())), Some(&1));
        assert_eq!(report.events.get(&("EXPORT".to_string(), "rust".to_string())), Some(&1));
    }
}
