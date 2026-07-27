// Lang-Zong 编译器 — bridge_core.rs
// 统一桥接基础核心：所有桥接范式（源码映射/FFI/CLI/共享内存）的公共抽象
//
// 设计原则：
//   1. Bridge trait 是所有桥接范式的统一接口
//   2. BridgeMessage 是跨桥接的统一消息信封
//   3. BridgeError 是跨桥接的统一错误模型
//   4. BridgeRegistry 管理多桥接实例的注册与发现

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

// ──────────────── 统一错误模型 ────────────────

/// 桥接错误码：按故障域分层
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // 通用
    Unknown = 0,
    NotSupported = 1,
    Timeout = 2,
    Cancelled = 3,

    // 连接层 (10-19)
    ConnectionFailed = 10,
    ConnectionLost = 11,
    ConnectionRefused = 12,
    AlreadyConnected = 13,

    // 数据层 (20-29)
    SerializationError = 20,
    DeserializationError = 21,
    InvalidMessage = 22,
    PayloadTooLarge = 23,

    // 类型层 (30-39)
    TypeMismatch = 30,
    UnsupportedType = 31,
    MarshalingError = 32,

    // 安全层 (40-49)
    PermissionDenied = 40,
    SandboxViolation = 41,
    CapabilityMissing = 42,

    // 资源层 (50-59)
    OutOfMemory = 50,
    QuotaExceeded = 51,
    FileDescriptorLimit = 52,

    // 版本层 (60-69)
    VersionMismatch = 60,
    IncompatibleABI = 61,
    DeprecatedAPI = 62,
}

impl ErrorCode {
    pub fn is_retryable(&self) -> bool {
        matches!(self,
            ErrorCode::Timeout | ErrorCode::ConnectionLost | ErrorCode::ConnectionRefused
        )
    }

    pub fn domain(&self) -> &'static str {
        match self {
            ErrorCode::Unknown | ErrorCode::NotSupported | ErrorCode::Timeout | ErrorCode::Cancelled => "core",
            ErrorCode::ConnectionFailed | ErrorCode::ConnectionLost | ErrorCode::ConnectionRefused | ErrorCode::AlreadyConnected => "connection",
            ErrorCode::SerializationError | ErrorCode::DeserializationError | ErrorCode::InvalidMessage | ErrorCode::PayloadTooLarge => "data",
            ErrorCode::TypeMismatch | ErrorCode::UnsupportedType | ErrorCode::MarshalingError => "type",
            ErrorCode::PermissionDenied | ErrorCode::SandboxViolation | ErrorCode::CapabilityMissing => "security",
            ErrorCode::OutOfMemory | ErrorCode::QuotaExceeded | ErrorCode::FileDescriptorLimit => "resource",
            ErrorCode::VersionMismatch | ErrorCode::IncompatibleABI | ErrorCode::DeprecatedAPI => "version",
        }
    }
}

/// 统一桥接错误
#[derive(Debug, Clone)]
pub struct BridgeError {
    pub code: ErrorCode,
    pub message: String,
    pub details: HashMap<String, String>,
    pub source_bridge: String,
}

impl BridgeError {
    pub fn new(code: ErrorCode, message: impl Into<String>, source: impl Into<String>) -> Self {
        BridgeError {
            code,
            message: message.into(),
            details: HashMap::new(),
            source_bridge: source.into(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    pub fn is_retryable(&self) -> bool {
        self.code.is_retryable()
    }
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} (code={:?}, bridge={})",
            self.code.domain(), self.message, self.code, self.source_bridge)
    }
}

// ──────────────── 统一消息信封 ────────────────

/// 消息类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageType {
    Request,
    Response,
    Event,
    Error,
    Ping,
    Pong,
}

/// 消息头部
#[derive(Debug, Clone)]
pub struct MessageHeader {
    pub id: String,                // 请求-响应匹配 id
    pub msg_type: MessageType,
    pub timestamp: u64,            // Unix 毫秒时间戳
    pub version: String,           // 协议版本
    pub trace_id: Option<String>,  // 分布式追踪 id
    pub span_id: Option<String>,   // 当前 span id
}

/// 统一消息信封
#[derive(Debug, Clone)]
pub struct BridgeMessage {
    pub header: MessageHeader,
    pub method: String,            // 调用的方法/函数名
    pub params: HashMap<String, String>,  // 参数键值对
    pub body: Option<String>,      // 二进制数据（Base64 编码）
}

impl BridgeMessage {
    pub fn new_request(method: impl Into<String>) -> Self {
        BridgeMessage {
            header: MessageHeader {
                id: uuid_v4(),
                msg_type: MessageType::Request,
                timestamp: now_millis(),
                version: "1.0".to_string(),
                trace_id: None,
                span_id: None,
            },
            method: method.into(),
            params: HashMap::new(),
            body: None,
        }
    }

    pub fn new_response(request_id: impl Into<String>) -> Self {
        BridgeMessage {
            header: MessageHeader {
                id: request_id.into(),
                msg_type: MessageType::Response,
                timestamp: now_millis(),
                version: "1.0".to_string(),
                trace_id: None,
                span_id: None,
            },
            method: String::new(),
            params: HashMap::new(),
            body: None,
        }
    }

    pub fn new_ping() -> Self {
        BridgeMessage {
            header: MessageHeader {
                id: uuid_v4(),
                msg_type: MessageType::Ping,
                timestamp: now_millis(),
                version: "1.0".to_string(),
                trace_id: None,
                span_id: None,
            },
            method: String::new(),
            params: HashMap::new(),
            body: None,
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn with_trace(mut self, trace_id: impl Into<String>, span_id: impl Into<String>) -> Self {
        self.header.trace_id = Some(trace_id.into());
        self.header.span_id = Some(span_id.into());
        self
    }
}

// ──────────────── 统一 Bridge trait ────────────────

/// Bridge 分级：影响 codegen 策略和性能预期
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BridgeLevel {
    /// Level 0: 编译期消解，零运行时开销（源码映射）
    CompileTime = 0,
    /// Level 1: 链接时绑定，极低开销（静态链接）
    LinkTime = 1,
    /// Level 2: 运行时动态链接，低开销（DLL/.so）
    Runtime = 2,
    /// Level 3: 跨进程序列化，中等开销（CLI 管道）
    InterProcess = 3,
    /// Level 4: 跨进程共享内存，可调开销（共享内存）
    SharedMemory = 4,
}

/// 桥接能力标志（位掩码）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeCapability(u32);

impl BridgeCapability {
    pub const NONE: BridgeCapability = BridgeCapability(0);
    pub const IMPORT: BridgeCapability = BridgeCapability(1 << 0);
    pub const FUNCTION_CALL: BridgeCapability = BridgeCapability(1 << 1);
    pub const METHOD_CALL: BridgeCapability = BridgeCapability(1 << 2);
    pub const TYPE_REWRITE: BridgeCapability = BridgeCapability(1 << 3);
    pub const SHIM_INJECT: BridgeCapability = BridgeCapability(1 << 4);
    pub const HOT_RELOAD: BridgeCapability = BridgeCapability(1 << 5);
    pub const ASYNC: BridgeCapability = BridgeCapability(1 << 6);
    pub const STREAMING: BridgeCapability = BridgeCapability(1 << 7);
    pub const BATCH: BridgeCapability = BridgeCapability(1 << 8);

    pub fn contains(self, other: BridgeCapability) -> bool {
        self.0 & other.0 != 0
    }

    pub fn union(self, other: BridgeCapability) -> BridgeCapability {
        BridgeCapability(self.0 | other.0)
    }
}

impl Default for BridgeCapability {
    fn default() -> Self { BridgeCapability::NONE }
}

impl std::ops::BitOr for BridgeCapability {
    type Output = BridgeCapability;
    fn bitor(self, rhs: BridgeCapability) -> BridgeCapability {
        BridgeCapability(self.0 | rhs.0)
    }
}

/// 导入解析结果：结构化返回（供 CodeGen 生成 use 语句 + 类型别名 + shim 判断）
#[derive(Debug, Clone)]
pub struct ImportResolveResult {
    pub rust_path: String,
    pub type_aliases: Vec<(String, String)>,
    pub requires_shim: bool,
    pub is_tier2: bool,
    pub feature_flags: Vec<String>,
    pub extern_crates: Vec<String>,
    pub error: Option<String>,
}

impl ImportResolveResult {
    /// 空结果（身份透传 fallback）
    pub fn identity(lz_path: &[String]) -> Self {
        ImportResolveResult {
            rust_path: lz_path.join("::"),
            type_aliases: vec![],
            requires_shim: false,
            is_tier2: false,
            feature_flags: vec![],
            extern_crates: vec![],
            error: None,
        }
    }

    /// 空路径的结果
    pub fn empty() -> Self {
        ImportResolveResult {
            rust_path: String::new(),
            type_aliases: vec![],
            requires_shim: false,
            is_tier2: false,
            feature_flags: vec![],
            extern_crates: vec![],
            error: None,
        }
    }
}

// ──────────────── 结构化解析结果 ────────────────

/// 函数调用解析结果（lz 函数名 → Rust 路径 + 适配信息）
#[derive(Debug, Clone)]
pub struct CallResolveResult {
    pub rust_path: String,
    pub shim: String,
    pub module_name: String,
    pub is_macro: bool,
    pub is_template: bool,  // rust_path 含 {0}/{1} 等占位符
}

impl CallResolveResult {
    pub fn simple(rust_path: impl Into<String>) -> Self {
        CallResolveResult {
            rust_path: rust_path.into(),
            shim: String::new(),
            module_name: String::new(),
            is_macro: false,
            is_template: false,
        }
    }
}

/// 方法名解析结果（lz camelCase → Rust snake_case）
#[derive(Debug, Clone)]
pub struct MethodResolveResult {
    pub rust_method: String,
    pub rewritten: bool,
    pub shim: String,
}

impl MethodResolveResult {
    pub fn identity(method: impl Into<String>) -> Self {
        let m = method.into();
        MethodResolveResult { rust_method: m, rewritten: false, shim: String::new() }
    }

    pub fn mapped(lz: impl Into<String>, rust: impl Into<String>) -> Self {
        MethodResolveResult { rust_method: rust.into(), rewritten: true, shim: String::new() }
    }
}

// ──────────────── 桥接元数据 ────────────────

/// 桥接自身描述：版本、身份、兼容范围
#[derive(Debug, Clone)]
pub struct BridgeMeta {
    pub version: String,
    pub description: String,
    pub lz_version_min: Option<String>,  // 最低兼容 lz 版本
    pub lz_version_max: Option<String>,  // 最高兼容 lz 版本（None=无上限）
    pub provides: Vec<String>,           // 提供的功能集名称
}

impl Default for BridgeMeta {
    fn default() -> Self {
        BridgeMeta {
            version: "0.1.0".into(),
            description: String::new(),
            lz_version_min: None,
            lz_version_max: None,
            provides: vec![],
        }
    }
}

// ──────────────── 健康检查 ────────────────

/// 桥接运行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeState {
    Healthy,
    Degraded,      // 部分功能不可用
    Unhealthy,      // 核心功能不可用
    Disconnected,
}

/// 结构化健康检查结果
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub state: BridgeState,
    pub latency: Duration,               // 最近一次 ping 延迟
    pub active_since: Option<Duration>,  // 自激活以来的运行时间
    pub error_count: u64,
    pub detail: String,                  // 人类可读的状态描述
}

impl HealthStatus {
    pub fn unknown() -> Self {
        HealthStatus {
            state: BridgeState::Disconnected,
            latency: Duration::ZERO,
            active_since: None,
            error_count: 0,
            detail: "not connected".into(),
        }
    }

    pub fn healthy(latency: Duration) -> Self {
        HealthStatus {
            state: BridgeState::Healthy,
            latency,
            active_since: None,
            error_count: 0,
            detail: "ok".into(),
        }
    }
}

// ──────────────── 导出枚举（introspection）────────────────

/// 导出项类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Function,
    Method,
    Type,
    Module,
    Constant,
}

/// 一个桥接对外暴露的符号
#[derive(Debug, Clone)]
pub struct ExportEntry {
    pub name: String,
    pub kind: ExportKind,
    pub signature: String,   // 人类可读的类型签名
    pub module: String,      // 符号所属的子模块
}

/// 统一桥接接口
pub trait Bridge: fmt::Debug {
    /// 桥接名称
    fn name(&self) -> &str;

    /// 桥接分级
    fn level(&self) -> BridgeLevel;

    /// 能力标志
    fn capabilities(&self) -> BridgeCapability;

    // ─── 生命周期 ───

    /// 建立连接
    fn connect(&mut self) -> Result<(), BridgeError> { Ok(()) }

    /// 断开连接
    fn disconnect(&mut self) -> Result<(), BridgeError> { Ok(()) }

    /// 健康检查
    fn ping(&self) -> Result<Duration, BridgeError> {
        Err(BridgeError::new(ErrorCode::NotSupported, "ping not supported", self.name()))
    }

    /// 是否已连接
    fn is_connected(&self) -> bool { true }

    // ─── 消息传递（运行时桥接使用）───

    /// 发送消息并等待响应
    fn send(&self, _msg: BridgeMessage) -> Result<BridgeMessage, BridgeError> {
        Err(BridgeError::new(ErrorCode::NotSupported, "send not supported", self.name()))
    }

    // ─── 代码生成（编译期桥接使用）───

    /// 生成模块导入的 Rust 代码
    fn gen_import(&self, _module_path: &[String], _items: &[String]) -> String {
        String::new()
    }

    /// 结构化导入解析（返回完整 ImportResolveResult，供 CodeGen 使用）
    fn resolve_import_full(&self, _module_path: &[String], _items: &[String]) -> Option<ImportResolveResult> {
        None
    }

    /// 生成函数调用的 Rust 代码
    fn gen_call(&self, _func_name: &str, _args: &[String]) -> Option<String> {
        None
    }

    /// 解析方法名（lz → Rust）
    fn gen_method(&self, _method: &str, _receiver_type: &str) -> String {
        _method.to_string()
    }

    /// 解析类型（lz → Rust），返回 None 表示不处理
    fn gen_type(&self, _lz_type: &str) -> Option<String> {
        None
    }

    /// 需要的 shim 函数名列表
    fn required_shims(&self) -> Vec<String> {
        vec![]
    }

    // ─── 元数据 ───

    /// 桥接自身描述（版本、兼容范围等）
    fn meta(&self) -> BridgeMeta { BridgeMeta::default() }

    // ─── 改进的健康检查 ───

    /// 结构化健康检查（替代简单 ping）
    fn health(&self) -> HealthStatus {
        match self.ping() {
            Ok(latency) => HealthStatus::healthy(latency),
            Err(_) => HealthStatus::unknown(),
        }
    }

    // ─── 生命周期钩子 ───

    /// 桥接被激活时调用（注册到 Registry 后）
    fn on_activate(&mut self) -> Result<(), BridgeError> { Ok(()) }

    /// 桥接被停用时调用（从 Registry 移除前）
    fn on_deactivate(&mut self) -> Result<(), BridgeError> { Ok(()) }

    // ─── 依赖声明 ───

    /// 本桥接依赖的其他桥接名称列表（Registry 验证）
    fn depends_on(&self) -> &[String] { &[] }

    // ─── 结构化解析（与 resolve_import_full 对齐）───

    /// 结构化函数调用解析
    fn resolve_call_full(&self, _func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        None
    }

    /// 结构化方法名解析
    fn resolve_method_full(&self, _method: &str, _receiver_type: &str) -> Option<MethodResolveResult> {
        None
    }

    // ─── 导出枚举（introspection）───

    /// 列出指定类型的所有导出符号
    fn list_exports(&self, _kind: ExportKind) -> Vec<ExportEntry> { vec![] }

    /// 导出符号总数
    fn export_count(&self) -> usize { 0 }

    // ─── 批量操作 ───

    /// 批量解析导入（性能优化：一次查找多个模块）
    fn batch_import(&self, _requests: &[(&[String], &[String])]) -> Vec<ImportResolveResult> {
        // 默认回退：逐个解析
        _requests.iter()
            .map(|(path, items)| match self.resolve_import_full(path, items) {
                Some(r) => r,
                None => ImportResolveResult::identity(path),
            })
            .collect()
    }

    // ─── 热重载 ───

    /// 重新加载桥接配置
    fn reload(&mut self) -> Result<(), BridgeError> {
        Err(BridgeError::new(ErrorCode::NotSupported, "reload not supported", self.name()))
    }
}

// ──────────────── BridgeRegistry ────────────────

/// 桥接注册中心：统一管理多个桥接实例
pub struct BridgeRegistry {
    bridges: Vec<Box<dyn Bridge>>,
    default_bridge: Option<String>,
}

impl BridgeRegistry {
    pub fn new() -> Self {
        BridgeRegistry {
            bridges: Vec::new(),
            default_bridge: None,
        }
    }

    pub fn register(&mut self, bridge: Box<dyn Bridge>) {
        self.bridges.push(bridge);
    }

    pub fn set_default(&mut self, name: impl Into<String>) {
        self.default_bridge = Some(name.into());
    }

    pub fn find(&self, name: &str) -> Option<&dyn Bridge> {
        self.bridges.iter().find(|b| b.name() == name).map(|b| b.as_ref())
    }

    pub fn default(&self) -> Option<&dyn Bridge> {
        self.default_bridge.as_ref()
            .and_then(|name| self.find(name))
            .or_else(|| self.bridges.first().map(|b| b.as_ref()))
    }

    pub fn all(&self) -> &[Box<dyn Bridge>] {
        &self.bridges
    }

    /// 为导入路径选择最佳桥接
    pub fn resolve_import(&self, module_path: &[String], items: &[String]) -> String {
        for bridge in &self.bridges {
            if bridge.capabilities().contains(BridgeCapability::IMPORT) {
                let result = bridge.gen_import(module_path, items);
                if !result.is_empty() {
                    return result;
                }
            }
        }
        // fallback: 身份透传
        if items.is_empty() {
            module_path.join("::")
        } else {
            format!("{}::{{{}}}", module_path.join("::"), items.join(", "))
        }
    }

    /// 为函数调用选择最佳桥接
    pub fn resolve_call(&self, func_name: &str, args: &[String]) -> Option<String> {
        for bridge in &self.bridges {
            if bridge.capabilities().contains(BridgeCapability::FUNCTION_CALL) {
                if let Some(result) = bridge.gen_call(func_name, args) {
                    return Some(result);
                }
            }
        }
        None
    }

    /// 为导入解析选择最佳桥接（结构化结果）
    pub fn resolve_import_full(&self, module_path: &[String], items: &[String]) -> ImportResolveResult {
        for bridge in &self.bridges {
            if bridge.capabilities().contains(BridgeCapability::IMPORT) {
                if let Some(result) = bridge.resolve_import_full(module_path, items) {
                    return result;
                }
            }
        }
        // fallback: 身份透传
        ImportResolveResult::identity(module_path)
    }

    /// 为方法调用选择最佳桥接
    pub fn resolve_method(&self, method: &str, receiver_type: &str) -> String {
        for bridge in &self.bridges {
            if bridge.capabilities().contains(BridgeCapability::METHOD_CALL) {
                let result = bridge.gen_method(method, receiver_type);
                if result != method {
                    return result;
                }
            }
        }
        method.to_string()
    }

    /// 为类型重写选择最佳桥接
    pub fn resolve_type(&self, lz_type: &str) -> Option<String> {
        for bridge in &self.bridges {
            if bridge.capabilities().contains(BridgeCapability::TYPE_REWRITE) {
                if let Some(result) = bridge.gen_type(lz_type) {
                    return Some(result);
                }
            }
        }
        None
    }

    /// 收集所有桥接所需的 shim
    pub fn collect_shims(&self) -> Vec<String> {
        let mut all_shims = Vec::new();
        for bridge in &self.bridges {
            all_shims.extend(bridge.required_shims());
        }
        all_shims
    }

    // ─── 生命周期管理 ───

    /// 注册桥接并验证依赖
    pub fn register_with_deps(&mut self, bridge: Box<dyn Bridge>) -> Result<(), BridgeError> {
        let name = bridge.name().to_string();
        let deps = bridge.depends_on().to_vec();

        // 验证所有依赖已注册
        for dep in &deps {
            if self.find(dep).is_none() {
                return Err(BridgeError::new(
                    ErrorCode::CapabilityMissing,
                    format!("dependency '{}' required by '{}' not found", dep, name),
                    &name,
                ));
            }
        }

        self.register(bridge);
        Ok(())
    }

    /// 注销桥接并返回其所有权
    pub fn deregister(&mut self, name: &str) -> Option<Box<dyn Bridge>> {
        let pos = self.bridges.iter().position(|b| b.name() == name);
        if let Some(idx) = pos {
            // 检查是否是被依赖项
            let is_dep_of = self.bridges.iter()
                .filter(|b| b.name() != name)
                .any(|b| b.depends_on().contains(&name.to_string()));

            if is_dep_of {
                // 不阻止注销，但应被调用方感知（记录到返回项的 error_count 中不实际）
            }

            // 清理 default 引用
            if self.default_bridge.as_deref() == Some(name) {
                self.default_bridge = None;
            }
            Some(self.bridges.swap_remove(idx))
        } else {
            None
        }
    }

    /// 已注册桥接数量
    pub fn count(&self) -> usize {
        self.bridges.len()
    }

    /// 所有已注册桥接名称
    pub fn names(&self) -> Vec<String> {
        self.bridges.iter().map(|b| b.name().to_string()).collect()
    }

    // ─── 基于能力/级别的查找 ───

    /// 查找所有支持指定能力的桥接
    pub fn find_by_capability(&self, cap: BridgeCapability) -> Vec<&dyn Bridge> {
        self.bridges.iter()
            .filter(|b| b.capabilities().contains(cap))
            .map(|b| b.as_ref())
            .collect()
    }

    /// 按桥接级别排序（编译期优先），返回第一个支持指定能力的桥接
    pub fn best_for(&self, cap: BridgeCapability) -> Option<&dyn Bridge> {
        let mut candidates: Vec<&dyn Bridge> = self.bridges.iter()
            .filter(|b| b.capabilities().contains(cap))
            .map(|b| b.as_ref())
            .collect();
        // 按 level 升序排列（CompileTime=0 最先）
        candidates.sort_by_key(|b| b.level());
        candidates.into_iter().next()
    }

    // ─── 结构化解析 ───

    /// 结构化函数调用解析（返回完整 CallResolveResult）
    pub fn resolve_call_full(&self, func_name: &str, args: &[String]) -> Option<CallResolveResult> {
        for bridge in &self.bridges {
            if bridge.capabilities().contains(BridgeCapability::FUNCTION_CALL) {
                if let Some(result) = bridge.resolve_call_full(func_name, args) {
                    return Some(result);
                }
            }
        }
        None
    }

    /// 结构化方法名解析（返回完整 MethodResolveResult）
    pub fn resolve_method_full(&self, method: &str, receiver_type: &str) -> Option<MethodResolveResult> {
        for bridge in &self.bridges {
            if bridge.capabilities().contains(BridgeCapability::METHOD_CALL) {
                if let Some(result) = bridge.resolve_method_full(method, receiver_type) {
                    return Some(result);
                }
            }
        }
        None
    }

    // ─── 批量操作 ───

    /// 批量导入解析：[(path, items)] → [ImportResolveResult]
    pub fn batch_import(&self, requests: &[(&[String], &[String])]) -> Vec<ImportResolveResult> {
        for bridge in &self.bridges {
            if bridge.capabilities().contains(BridgeCapability::IMPORT) {
                let results = bridge.batch_import(requests);
                if !results.is_empty() {
                    return results;
                }
            }
        }
        // fallback: 逐项身份透传
        requests.iter()
            .map(|(path, _)| ImportResolveResult::identity(path))
            .collect()
    }

    // ─── 导出枚举（introspection）───

    /// 列出所有桥接中指定类型的导出
    pub fn list_exports(&self, kind: ExportKind) -> Vec<(String, ExportEntry)> {
        let mut all = Vec::new();
        for bridge in &self.bridges {
            for entry in bridge.list_exports(kind) {
                all.push((bridge.name().to_string(), entry));
            }
        }
        all
    }

    /// 所有桥接的导出总数
    pub fn total_exports(&self) -> usize {
        self.bridges.iter().map(|b| b.export_count()).sum()
    }

    // ─── 统计 ───

    /// 注册中心运行统计
    pub fn stats(&self) -> RegistryStats {
        RegistryStats {
            bridge_count: self.bridges.len(),
            default_name: self.default_bridge.clone(),
            bridge_names: self.names(),
        }
    }
}

/// 注册中心统计摘要
#[derive(Debug, Clone)]
pub struct RegistryStats {
    pub bridge_count: usize,
    pub default_name: Option<String>,
    pub bridge_names: Vec<String>,
}

// ──────────────── 内部辅助 ────────────────

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{:016x}", ts.as_nanos())
}

fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ──────────────── 测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestBridge {
        name: String,
        level: BridgeLevel,
        caps: BridgeCapability,
    }

    impl Bridge for TestBridge {
        fn name(&self) -> &str { &self.name }
        fn level(&self) -> BridgeLevel { self.level }
        fn capabilities(&self) -> BridgeCapability { self.caps }
    }

    #[test]
    fn test_error_domain() {
        assert_eq!(ErrorCode::Timeout.domain(), "core");
        assert_eq!(ErrorCode::ConnectionFailed.domain(), "connection");
        assert_eq!(ErrorCode::TypeMismatch.domain(), "type");
        assert_eq!(ErrorCode::PermissionDenied.domain(), "security");
        assert_eq!(ErrorCode::VersionMismatch.domain(), "version");
    }

    #[test]
    fn test_error_retryable() {
        assert!(ErrorCode::Timeout.is_retryable());
        assert!(ErrorCode::ConnectionLost.is_retryable());
        assert!(!ErrorCode::TypeMismatch.is_retryable());
        assert!(!ErrorCode::PermissionDenied.is_retryable());
    }

    #[test]
    fn test_bridge_error_display() {
        let err = BridgeError::new(ErrorCode::ConnectionFailed, "host unreachable", "test-bridge")
            .with_detail("host", "192.168.1.1")
            .with_detail("port", "8080");
        let s = format!("{}", err);
        assert!(s.contains("connection"));
        assert!(s.contains("host unreachable"));
        assert!(s.contains("test-bridge"));
    }

    #[test]
    fn test_message_envelope() {
        let msg = BridgeMessage::new_request("fs_read")
            .with_param("path", "/tmp/test.txt")
            .with_trace("trace-001", "span-002");
        assert_eq!(msg.method, "fs_read");
        assert_eq!(msg.params.get("path").unwrap(), "/tmp/test.txt");
        assert_eq!(msg.header.trace_id.as_ref().unwrap(), "trace-001");
        assert_eq!(msg.header.msg_type, MessageType::Request);
    }

    #[test]
    fn test_message_response_matches_request() {
        let req = BridgeMessage::new_request("test");
        let req_id = req.header.id.clone();
        let resp = BridgeMessage::new_response(&req_id);
        assert_eq!(resp.header.id, req_id);
        assert_eq!(resp.header.msg_type, MessageType::Response);
    }

    #[test]
    fn test_ping_message() {
        let ping = BridgeMessage::new_ping();
        assert_eq!(ping.header.msg_type, MessageType::Ping);
    }

    #[test]
    fn test_registry_empty() {
        let registry = BridgeRegistry::new();
        assert!(registry.default().is_none());
    }

    #[test]
    fn test_registry_register_and_find() {
        let mut registry = BridgeRegistry::new();
        let bridge = Box::new(TestBridge {
            name: "source".to_string(),
            level: BridgeLevel::CompileTime,
            caps: BridgeCapability::IMPORT | BridgeCapability::TYPE_REWRITE,
        });
        registry.register(bridge);
        assert!(registry.find("source").is_some());
        assert!(registry.find("nonexistent").is_none());
        assert!(registry.default().is_some());
    }

    #[test]
    fn test_registry_resolve_import_fallback() {
        let registry = BridgeRegistry::new();
        let result = registry.resolve_import(&["my".into(), "lib".into()], &[]);
        assert_eq!(result, "my::lib"); // 身份透传
    }

    #[test]
    fn test_bridge_level_ordering() {
        assert!(BridgeLevel::CompileTime < BridgeLevel::LinkTime);
        assert!(BridgeLevel::LinkTime < BridgeLevel::Runtime);
        assert!(BridgeLevel::Runtime < BridgeLevel::InterProcess);
        assert!(BridgeLevel::InterProcess < BridgeLevel::SharedMemory);
    }

    #[test]
    fn test_bridge_capabilities_flags() {
        let caps = BridgeCapability::IMPORT | BridgeCapability::METHOD_CALL;
        assert!(caps.contains(BridgeCapability::IMPORT));
        assert!(caps.contains(BridgeCapability::METHOD_CALL));
        assert!(!caps.contains(BridgeCapability::HOT_RELOAD));
    }

    // ─── 新 API 测试 ───

    #[test]
    fn test_call_resolve_result_simple() {
        let r = CallResolveResult::simple("std::io::stdout");
        assert_eq!(r.rust_path, "std::io::stdout");
        assert!(!r.is_macro);
        assert!(!r.is_template);
        assert!(r.shim.is_empty());
    }

    #[test]
    fn test_method_resolve_result_identity() {
        let r = MethodResolveResult::identity("isEmpty");
        assert_eq!(r.rust_method, "isEmpty");
        assert!(!r.rewritten);
    }

    #[test]
    fn test_method_resolve_result_mapped() {
        let r = MethodResolveResult::mapped("isEmpty", "is_empty");
        assert_eq!(r.rust_method, "is_empty");
        assert!(r.rewritten);
    }

    #[test]
    fn test_bridge_meta_default() {
        let m = BridgeMeta::default();
        assert_eq!(m.version, "0.1.0");
        assert!(m.description.is_empty());
        assert!(m.lz_version_min.is_none());
    }

    #[test]
    fn test_health_status_healthy() {
        let h = HealthStatus::healthy(Duration::from_millis(5));
        assert_eq!(h.state, BridgeState::Healthy);
        assert_eq!(h.latency, Duration::from_millis(5));
        assert_eq!(h.error_count, 0);
    }

    #[test]
    fn test_health_status_unknown() {
        let h = HealthStatus::unknown();
        assert_eq!(h.state, BridgeState::Disconnected);
        assert_eq!(h.error_count, 0);
    }

    #[test]
    fn test_export_kind_values() {
        assert_ne!(ExportKind::Function, ExportKind::Type);
        assert_ne!(ExportKind::Method, ExportKind::Module);
    }

    #[derive(Debug)]
    struct FullTestBridge {
        name: String,
        level: BridgeLevel,
        caps: BridgeCapability,
        deps: Vec<String>,
    }

    impl FullTestBridge {
        fn with_deps(name: &str, deps: Vec<&str>) -> Self {
            FullTestBridge {
                name: name.into(),
                level: BridgeLevel::CompileTime,
                caps: BridgeCapability::IMPORT,
                deps: deps.into_iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    impl Bridge for FullTestBridge {
        fn name(&self) -> &str { &self.name }
        fn level(&self) -> BridgeLevel { self.level }
        fn capabilities(&self) -> BridgeCapability { self.caps }
        fn depends_on(&self) -> &[String] { &self.deps }
        fn meta(&self) -> BridgeMeta {
            BridgeMeta {
                version: "2.0.0".into(),
                description: "test bridge".into(),
                ..Default::default()
            }
        }
    }

    #[test]
    fn test_registry_register_with_deps_success() {
        let mut reg = BridgeRegistry::new();
        // 先注册依赖项
        reg.register(Box::new(FullTestBridge::with_deps("base", vec![])));
        // 再注册依赖 base 的桥接
        let result = reg.register_with_deps(Box::new(FullTestBridge::with_deps("ext", vec!["base"])));
        assert!(result.is_ok());
        assert_eq!(reg.count(), 2);
    }

    #[test]
    fn test_registry_register_with_deps_missing() {
        let mut reg = BridgeRegistry::new();
        let result = reg.register_with_deps(Box::new(FullTestBridge::with_deps("ext", vec!["base"])));
        assert!(result.is_err());
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn test_registry_deregister() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(TestBridge {
            name: "temp".into(), level: BridgeLevel::CompileTime, caps: BridgeCapability::IMPORT,
        }));
        reg.set_default("temp");
        assert_eq!(reg.count(), 1);

        let removed = reg.deregister("temp");
        assert!(removed.is_some());
        assert_eq!(reg.count(), 0);
        assert!(reg.default().is_none());
    }

    #[test]
    fn test_registry_deregister_nonexistent() {
        let mut reg = BridgeRegistry::new();
        assert!(reg.deregister("nope").is_none());
    }

    #[test]
    fn test_registry_names() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(TestBridge {
            name: "a".into(), level: BridgeLevel::CompileTime, caps: BridgeCapability::IMPORT,
        }));
        reg.register(Box::new(TestBridge {
            name: "b".into(), level: BridgeLevel::Runtime, caps: BridgeCapability::FUNCTION_CALL,
        }));
        let names = reg.names();
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));
        assert_eq!(reg.count(), 2);
    }

    #[test]
    fn test_find_by_capability() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(TestBridge {
            name: "importer".into(), level: BridgeLevel::CompileTime,
            caps: BridgeCapability::IMPORT,
        }));
        reg.register(Box::new(TestBridge {
            name: "caller".into(), level: BridgeLevel::Runtime,
            caps: BridgeCapability::FUNCTION_CALL,
        }));
        reg.register(Box::new(TestBridge {
            name: "both".into(), level: BridgeLevel::CompileTime,
            caps: BridgeCapability::IMPORT | BridgeCapability::FUNCTION_CALL,
        }));

        let importers = reg.find_by_capability(BridgeCapability::IMPORT);
        assert_eq!(importers.len(), 2);
        assert!(importers.iter().any(|b| b.name() == "importer"));
        assert!(importers.iter().any(|b| b.name() == "both"));
    }

    #[test]
    fn test_best_for_prefers_compile_time() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(TestBridge {
            name: "slow-runtime".into(), level: BridgeLevel::Runtime,
            caps: BridgeCapability::FUNCTION_CALL,
        }));
        reg.register(Box::new(TestBridge {
            name: "fast-compile".into(), level: BridgeLevel::CompileTime,
            caps: BridgeCapability::FUNCTION_CALL,
        }));

        let best = reg.best_for(BridgeCapability::FUNCTION_CALL);
        assert!(best.is_some());
        assert_eq!(best.unwrap().name(), "fast-compile");
    }

    #[test]
    fn test_resolve_call_full_with_source_bridge() {
        // integration-style: create a SourceBridge and route through registry
        let std_dir = std::path::PathBuf::from("std");
        if let Ok(source) = crate::bridge::source::SourceBridge::new(std_dir) {
            let mut reg = BridgeRegistry::new();
            reg.register(Box::new(source));

            let result = reg.resolve_call_full("panic", &[]);
            assert!(result.is_some());
            let r = result.unwrap();
            assert!(r.rust_path.contains("panic"));
            assert!(r.is_macro); // panic → panic! is a macro
        }
        // if std directory not available, skip gracefully
    }

    #[test]
    fn test_resolve_method_full() {
        let std_dir = std::path::PathBuf::from("std");
        if let Ok(source) = crate::bridge::source::SourceBridge::new(std_dir) {
            let mut reg = BridgeRegistry::new();
            reg.register(Box::new(source));

            let result = reg.resolve_method_full("append", "Vec");
            assert!(result.is_some());
            let r = result.unwrap();
            assert_eq!(r.rust_method, "push");
            assert!(r.rewritten);
        }
    }

    #[test]
    fn test_batch_import() {
        let std_dir = std::path::PathBuf::from("std");
        if let Ok(source) = crate::bridge::source::SourceBridge::new(std_dir) {
            let mut reg = BridgeRegistry::new();
            reg.register(Box::new(source));

            let requests: &[(&[String], &[String])] = &[
                (&["std".into(), "io".into()], &[]),
                (&["std".into(), "fs".into()], &[]),
            ];
            let results = reg.batch_import(requests);
            assert_eq!(results.len(), 2);
            assert_eq!(results[0].rust_path, "std::io");
            assert_eq!(results[1].rust_path, "std::fs");
        }
    }

    #[test]
    fn test_list_exports_functions() {
        let std_dir = std::path::PathBuf::from("std");
        if let Ok(source) = crate::bridge::source::SourceBridge::new(std_dir) {
            let mut reg = BridgeRegistry::new();
            reg.register(Box::new(source));

            let exports = reg.list_exports(ExportKind::Function);
            // std bridge has read_to_string, write, etc.
            assert!(!exports.is_empty());
            assert!(exports.iter().any(|(bridge_name, _)| bridge_name == "source"));
        }
    }

    #[test]
    fn test_list_exports_types() {
        let std_dir = std::path::PathBuf::from("std");
        if let Ok(source) = crate::bridge::source::SourceBridge::new(std_dir) {
            let mut reg = BridgeRegistry::new();
            reg.register(Box::new(source));

            let exports = reg.list_exports(ExportKind::Type);
            assert!(!exports.is_empty());
            // Should have types like IOError, File, etc.
        }
    }

    #[test]
    fn test_total_exports_count() {
        let std_dir = std::path::PathBuf::from("std");
        if let Ok(source) = crate::bridge::source::SourceBridge::new(std_dir) {
            let mut reg = BridgeRegistry::new();
            reg.register(Box::new(source));
            assert!(reg.total_exports() > 0);
        }
    }

    #[test]
    fn test_registry_stats() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(TestBridge {
            name: "main".into(), level: BridgeLevel::CompileTime, caps: BridgeCapability::IMPORT,
        }));
        reg.set_default("main");

        let stats = reg.stats();
        assert_eq!(stats.bridge_count, 1);
        assert_eq!(stats.default_name, Some("main".to_string()));
        assert_eq!(stats.bridge_names.len(), 1);
    }

    #[test]
    fn test_bridge_health_method() {
        // SourceBridge doesn't support ping, so health returns unknown
        let std_dir = std::path::PathBuf::from("std");
        if let Ok(source) = crate::bridge::source::SourceBridge::new(std_dir) {
            let h = source.health();
            assert_eq!(h.state, BridgeState::Disconnected);
        }
    }
}
