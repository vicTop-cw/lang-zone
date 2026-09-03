// Lang-Zong 编译器 — bridge/embed.rs
// Level 4: LZ 嵌入桥接
// 跨进程共享内存通信，通过 mmap 共享内存区实现 LZ ↔ 宿主之间的高速数据交换。
// 实现 Bridge trait，生成宿主应用的共享内存管理 Rust 代码。
//
// 使用场景：
//   1. 游戏引擎嵌入 LZ 脚本（Unity/Unreal/Godot 插件）
//   2. WebAssembly 宿主嵌入 LZ（WasmEdge/Wasmtime）
//   3. 数据库 UDF 使用 LZ 编写存储过程
//   4. CLI 工具通过 LZ 脚本扩展功能
//
// 设计原则：
//   1. 零拷贝：数据直接写入共享内存，接收方 mmap 读取
//   2. 原子化协议：请求/响应各一个 struct，通过 u64 版本号协调
//   3. 断开通告：进程退出时关闭共享内存，防止悬空指针
//   4. 与 CLI（Level 3）完全正交：CLI 适合低频/复杂输出，Embed 适合高频/小数据
//
// 共享内存布局：
//   ┌──────────────────────────────────────────┐
//   │ header: magic(4B) | version(4B) | sz(8B) │
//   ├──────────────────────────────────────────┤
//   │ request struct (固定偏移)                 │
//   ├──────────────────────────────────────────┤
//   │ response struct (固定偏移)                │
//   ├──────────────────────────────────────────┤
//   │ payload region (变长数据)                 │
//   └──────────────────────────────────────────┘
//   magic = 0x4C5A454D ("LZEM")

use crate::bridge::core::{
    Bridge, BridgeCapability, BridgeError, BridgeLevel, BridgeMeta,
    CallResolveResult, ExportEntry, ExportKind, ImportResolveResult,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

// ══════════════════════════════════════════════════════════════
// 协议常量
// ══════════════════════════════════════════════════════════════

/// 共享内存 magic 头（LZEM = Lang-Zone Embedded）
pub const LZEM_MAGIC: u32 = 0x4C5A454D;
/// 当前协议版本
pub const LZEM_VERSION: u32 = 1;

/// 共享内存区域大小（默认 1MB）
pub const DEFAULT_SHM_SIZE: usize = 1 << 20; // 1 MiB

/// 固定头大小（magic + version + size）
pub const LZEM_HEADER_SIZE: usize = 4 + 4 + 8; // 16 bytes

/// 请求结构偏移
pub const LZEM_REQUEST_OFFSET: usize = LZEM_HEADER_SIZE;

/// 请求结构大小（固定 64 bytes）
pub const LZEM_REQUEST_SIZE: usize = 64;

/// 响应结构偏移
pub const LZEM_RESPONSE_OFFSET: usize = LZEM_REQUEST_OFFSET + LZEM_REQUEST_SIZE;

/// 响应结构大小（固定 64 bytes）
pub const LZEM_RESPONSE_SIZE: usize = 64;

/// 载荷区起始偏移
pub const LZEM_PAYLOAD_OFFSET: usize = LZEM_RESPONSE_OFFSET + LZEM_RESPONSE_SIZE;

// ══════════════════════════════════════════════════════════════
// 协议结构体（用于 mmap 共享内存）
// ══════════════════════════════════════════════════════════════

/// LZEM 头（mmap 前 16 字节）
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LzemHeader {
    pub magic: u32,       // 0x4C5A454D
    pub version: u32,    // 协议版本
    pub size: u64,       // 共享内存总大小
}

impl LzemHeader {
    pub fn is_valid(&self) -> bool {
        self.magic == LZEM_MAGIC && self.version <= LZEM_VERSION
    }
}

/// LZEM 请求结构（固定 64 字节）
#[repr(C)]
#[derive(Debug)]
pub struct LzemRequest {
    /// 版本号（原子递增，接收方用版本号判断是否有新请求）
    pub version: AtomicU64,
    /// 调用编号（追踪用）
    pub call_id: u64,
    /// 函数名长度（不含 NUL）
    pub func_name_len: u32,
    /// 载荷长度（字节数）
    pub payload_len: u32,
    /// 保留字段
    pub reserved: [u8; 44],
}

impl Default for LzemRequest {
    fn default() -> Self {
        LzemRequest {
            version: AtomicU64::new(0),
            call_id: 0,
            func_name_len: 0,
            payload_len: 0,
            reserved: [0; 44],
        }
    }
}

/// LZEM 响应结构（固定 64 字节）
#[repr(C)]
#[derive(Debug)]
pub struct LzemResponse {
    /// 版本号（与请求版本号对应，接收方据此判断响应就绪）
    pub version: AtomicU64,
    /// 调用编号
    pub call_id: u64,
    /// 状态码（0 = OK, >0 = 错误码）
    pub status: u32,
    /// 响应载荷长度
    pub payload_len: u32,
    /// 错误消息长度（若 status != 0）
    pub error_len: u32,
    /// 保留字段
    pub reserved: [u8; 44],
}

impl Default for LzemResponse {
    fn default() -> Self {
        LzemResponse {
            version: AtomicU64::new(0),
            call_id: 0,
            status: 0,
            payload_len: 0,
            error_len: 0,
            reserved: [0; 44],
        }
    }
}

// ══════════════════════════════════════════════════════════════
// 嵌入端点配置
// ══════════════════════════════════════════════════════════════

/// 嵌入端点：宿主注册的可调用 LZ 函数
#[derive(Debug, Clone)]
pub struct EmbedEndpoint {
    /// 函数名（lz 中定义的函数名）
    pub name: String,
    /// 签名描述（"fn(i32, str) -> i64"）
    pub signature: String,
    /// 是否是异步函数
    pub is_async: bool,
    /// 描述文档
    pub doc: String,
}

// ══════════════════════════════════════════════════════════════
// EmbedBridge — LZ 嵌入桥接
// ══════════════════════════════════════════════════════════════

/// Level 4: 共享内存嵌入桥接
///
/// 核心职责：
///   1. 注册宿主暴露给 LZ 的函数（`import embed::<host>::<func>`）
///   2. 注册 LZ 暴露给宿主的函数（`@export` 标注的 LZ 函数）
///   3. 生成宿主侧共享内存管理代码
///   4. 生成 LZ 侧调用 shim 代码
///
/// 双向桥接：
///   - 正向（lz → 宿主）：lz import embed::host::func，codegen 生成 shm_write + shm_read
///   - 反向（宿主 → lz）：宿主 import lz_func，codegen 生成 lz export wrapper
#[derive(Debug)]
pub struct EmbedBridge {
    /// 宿主暴露给 LZ 的函数（host_func_name → EmbedEndpoint）
    host_funcs: HashMap<String, EmbedEndpoint>,
    /// LZ 暴露给宿主的函数（lz_func_name → EmbedEndpoint）
    exported_funcs: HashMap<String, EmbedEndpoint>,
    /// 共享内存配置
    shm_name: String,
    shm_size: usize,
    /// 是否作为 server（创建共享内存）还是 client（连接已有共享内存）
    is_server: bool,
    /// 共享内存路径（Linux: /dev/shm/lzem_<name>, Windows: Global\\LZEM_<name>）
    shm_path: String,
}

impl EmbedBridge {
    pub fn new(name: &str) -> Self {
        let shm_name = format!("lzem_{}", name);
        #[cfg(windows)]
        let shm_path = format!("Global\\{}", shm_name);
        #[cfg(not(windows))]
        let shm_path = format!("/dev/shm/{}", shm_name);

        EmbedBridge {
            host_funcs: HashMap::new(),
            exported_funcs: HashMap::new(),
            shm_name,
            shm_size: DEFAULT_SHM_SIZE,
            is_server: false,
            shm_path,
        }
    }

    /// 设为服务端（创建共享内存）
    pub fn as_server(mut self) -> Self {
        self.is_server = true;
        self
    }

    /// 设为客户端（连接共享内存）
    pub fn as_client(mut self) -> Self {
        self.is_server = false;
        self
    }

    /// 设置共享内存大小
    pub fn with_size(mut self, size: usize) -> Self {
        self.shm_size = size;
        self
    }

    // ─── 注册函数 ───

    /// 注册宿主暴露给 LZ 的函数
    pub fn register_host_func(&mut self, endpoint: EmbedEndpoint) {
        self.host_funcs.insert(endpoint.name.clone(), endpoint);
    }

    /// 注册 LZ 暴露给宿主的函数
    pub fn register_exported_func(&mut self, endpoint: EmbedEndpoint) {
        self.exported_funcs.insert(endpoint.name.clone(), endpoint);
    }

    // ─── 代码生成 ───

    /// 生成宿主侧共享内存管理模块
    pub fn generate_host_module(&self) -> String {
        let mut out = String::new();

        out.push_str("// ═══ Lang-Zone Embed Bridge — Host Side ═══\n");
        out.push_str("// Generated by lzc --bridge=embed\n");
        out.push_str("// Shared memory: ");
        out.push_str(&self.shm_path);
        out.push('\n');
        out.push_str("// ══════════════════════════════════════════\n\n");

        out.push_str("use std::sync::atomic::{AtomicU64, Ordering};\n");
        out.push_str("use std::ptr;\n\n");

        // 常量
        out.push_str(&format!("const LZEM_MAGIC: u32 = {:#X};\n", LZEM_MAGIC));
        out.push_str(&format!("const LZEM_VERSION: u32 = {};\n", LZEM_VERSION));
        out.push_str(&format!("const SHM_SIZE: usize = {};\n\n", self.shm_size));
        out.push_str(&format!("const HEADER_SIZE: usize = {};\n", LZEM_HEADER_SIZE));
        out.push_str(&format!("const REQUEST_OFFSET: usize = {};\n", LZEM_REQUEST_OFFSET));
        out.push_str(&format!("const RESPONSE_OFFSET: usize = {};\n", LZEM_RESPONSE_OFFSET));
        out.push_str(&format!("const PAYLOAD_OFFSET: usize = {};\n\n", LZEM_PAYLOAD_OFFSET));

        // Header 结构
        out.push_str("#[repr(C)]\n");
        out.push_str("struct ShmHeader {\n");
        out.push_str("    magic: u32,\n");
        out.push_str("    version: u32,\n");
        out.push_str("    size: u64,\n");
        out.push_str("}\n\n");

        // LzemRequest 结构（简化版）
        out.push_str("#[repr(C)]\n");
        out.push_str("struct ShmRequest {\n");
        out.push_str("    version: u64,\n");
        out.push_str("    call_id: u64,\n");
        out.push_str("    func_name_len: u32,\n");
        out.push_str("    payload_len: u32,\n");
        out.push_str("    _reserved: [u8; 44],\n");
        out.push_str("}\n\n");

        // LzemResponse 结构
        out.push_str("#[repr(C)]\n");
        out.push_str("struct ShmResponse {\n");
        out.push_str("    version: u64,\n");
        out.push_str("    call_id: u64,\n");
        out.push_str("    status: u32,\n");
        out.push_str("    payload_len: u32,\n");
        out.push_str("    error_len: u32,\n");
        out.push_str("    _reserved: [u8; 44],\n");
        out.push_str("}\n\n");

        // LzemSession 结构
        out.push_str("/// 共享内存会话\n");
        out.push_str("pub struct LzemSession {\n");
        out.push_str("    data: *mut u8,\n");
        out.push_str("    size: usize,\n");
        out.push_str("    call_counter: AtomicU64,\n");
        out.push_str("}\n\n");

        out.push_str("unsafe impl Send for LzemSession {}\n");
        out.push_str("unsafe impl Sync for LzemSession {}\n\n");

        // new 函数
        if self.is_server {
            out.push_str("impl LzemSession {\n");
            out.push_str("    /// 创建（服务端）共享内存\n");
            out.push_str(&format!("    pub fn create() -> Result<Box<Self>, String> {{\n"));
            out.push_str("        #[cfg(windows)] {\n");
            out.push_str(&format!(
                "            Err(\"Windows shared memory not yet implemented\".to_string())\n",
            ));
            out.push_str("        }\n");
            out.push_str("        #[cfg(unix)] {\n");
            out.push_str("            use std::fs;\n");
            out.push_str(&format!("            let path = \"{}\";\n", self.shm_path));
            out.push_str("            // 移除已存在的共享内存\n");
            out.push_str("            let _ = fs::remove_file(path);\n");
            out.push_str("            let fd = unsafe {\n");
            out.push_str("                libc::shm_open(path, libc::O_CREAT | libc::O_RDWR, 0o600)\n");
            out.push_str("            };\n");
            out.push_str("            if fd < 0 {\n");
            out.push_str("                return Err(format!(\"shm_open failed\"));\n");
            out.push_str("            }\n");
            out.push_str(&format!("            if unsafe {{ libc::ftruncate(fd, {} as libc::off_t) }} < 0 {{\n", self.shm_size));
            out.push_str("                return Err(format!(\"ftruncate failed\"));\n");
            out.push_str("            }\n");
            out.push_str("            let data = unsafe {\n");
            out.push_str("                libc::mmap(\n");
            out.push_str("                    ptr::null_mut(),\n");
            out.push_str(&format!("                    {},\n", self.shm_size));
            out.push_str("                    libc::PROT_READ | libc::PROT_WRITE,\n");
            out.push_str("                    libc::MAP_SHARED,\n");
            out.push_str("                    fd,\n");
            out.push_str("                    0,\n");
            out.push_str("                )\n");
            out.push_str("            };\n");
            out.push_str("            if data == libc::MAP_FAILED {\n");
            out.push_str("                return Err(format!(\"mmap failed\"));\n");
            out.push_str("            }\n");
            out.push_str("            // 初始化头\n");
            out.push_str("            let header = data as *mut ShmHeader;\n");
            out.push_str("            unsafe {\n");
            out.push_str(&format!("                (*header).magic = {:#X};\n", LZEM_MAGIC));
            out.push_str(&format!("                (*header).version = {};\n", LZEM_VERSION));
            out.push_str(&format!("                (*header).size = {};\n", self.shm_size));
            out.push_str("            }\n");
            out.push_str("            Ok(Box::new(LzemSession {\n");
            out.push_str("                data: data as *mut u8,\n");
            out.push_str(&format!("                size: {},\n", self.shm_size));
            out.push_str("                call_counter: AtomicU64::new(0),\n");
            out.push_str("            }))\n");
            out.push_str("        }\n");
            out.push_str("    }\n\n");
        } else {
            out.push_str("impl LzemSession {\n");
            out.push_str("    /// 连接（客户端）共享内存\n");
            out.push_str(&format!("    pub fn connect() -> Result<Box<Self>, String> {{\n"));
            out.push_str("        #[cfg(windows)] {\n");
            out.push_str(&format!(
                "            Err(\"Windows shared memory not yet implemented\".to_string())\n",
            ));
            out.push_str("        }\n");
            out.push_str("        #[cfg(unix)] {\n");
            out.push_str("            let fd = unsafe {\n");
            out.push_str(&format!("                libc::shm_open(\"{}\", libc::O_RDWR, 0o600)\n", self.shm_path));
            out.push_str("            };\n");
            out.push_str("            if fd < 0 {\n");
            out.push_str("                return Err(format!(\"shm_open failed: {{}}\", std::io::Error::last_os_error()));\n");
            out.push_str("            }\n");
            out.push_str("            let data = unsafe {\n");
            out.push_str("                libc::mmap(\n");
            out.push_str("                    ptr::null_mut(),\n");
            out.push_str(&format!("                    {},\n", self.shm_size));
            out.push_str("                    libc::PROT_READ | libc::PROT_WRITE,\n");
            out.push_str("                    libc::MAP_SHARED,\n");
            out.push_str("                    fd,\n");
            out.push_str("                    0,\n");
            out.push_str("                )\n");
            out.push_str("            };\n");
            out.push_str("            if data == libc::MAP_FAILED {\n");
            out.push_str("                return Err(format!(\"mmap failed\"));\n");
            out.push_str("            }\n");
            out.push_str("            // 验证 magic\n");
            out.push_str("            let header = data as *const ShmHeader;\n");
            out.push_str("            unsafe {\n");
            out.push_str(&format!(
                "                if (*header).magic != {:#X} {{\n",
                LZEM_MAGIC
            ));
            out.push_str("                    return Err(format!(\"invalid magic\"));\n");
            out.push_str("                }\n");
            out.push_str("            }\n");
            out.push_str("            Ok(Box::new(LzemSession {\n");
            out.push_str("                data: data as *mut u8,\n");
            out.push_str(&format!("                size: {},\n", self.shm_size));
            out.push_str("                call_counter: AtomicU64::new(0),\n");
            out.push_str("            }))\n");
            out.push_str("        }\n");
            out.push_str("    }\n\n");
        }

        // call 方法
        out.push_str("    /// 调用 LZ 函数（通过共享内存）\n");
        out.push_str("    pub fn call(&self, func_name: &str, payload: &[u8]) -> Result<Vec<u8>, String> {\n");
        out.push_str("        let call_id = self.call_counter.fetch_add(1, Ordering::Relaxed) + 1;\n");
        out.push_str("        let req = (self.data as *mut ShmRequest).add(0);\n");
        out.push_str("        let resp = (self.data as *mut ShmResponse).add(0);\n\n");
        out.push_str("        // 写入请求\n");
        out.push_str("        unsafe {\n");
        out.push_str("            (*req).version.store(call_id, Ordering::SeqCst);\n");
        out.push_str("            (*req).call_id = call_id;\n");
        out.push_str("            (*req).func_name_len = func_name.len() as u32;\n");
        out.push_str("            (*req).payload_len = payload.len() as u32;\n");
        out.push_str("            // 写入函数名到载荷区\n");
        out.push_str("            let name_dst = self.data.add(PAYLOAD_OFFSET);\n");
        out.push_str("            ptr::copy_nonoverlapping(func_name.as_ptr(), name_dst, func_name.len());\n");
        out.push_str("            // 写入载荷\n");
        out.push_str("            let payload_dst = self.data.add(PAYLOAD_OFFSET + func_name.len());\n");
        out.push_str("            ptr::copy_nonoverlapping(payload.as_ptr(), payload_dst, payload.len());\n");
        out.push_str("        }\n\n");
        out.push_str("        // 等待响应（轮询 version）\n");
        out.push_str("        let mut spin = 0usize;\n");
        out.push_str("        loop {\n");
        out.push_str("            let resp_version = unsafe { (*resp).version.load(Ordering::SeqCst) };\n");
        out.push_str("            if resp_version == call_id {\n");
        out.push_str("                let status = unsafe { (*resp).status };\n");
        out.push_str("                let payload_len = unsafe { (*resp).payload_len } as usize;\n");
        out.push_str("                if status != 0 {\n");
        out.push_str("                    let error_len = unsafe { (*resp).error_len } as usize;\n");
        out.push_str("                    let error_bytes = unsafe {\n");
        out.push_str("                        std::slice::from_raw_parts(\n");
        out.push_str("                            self.data.add(PAYLOAD_OFFSET),\n");
        out.push_str("                            error_len,\n");
        out.push_str("                        )\n");
        out.push_str("                    };\n");
        out.push_str("                    return Err(String::from_utf8_lossy(error_bytes).to_string());\n");
        out.push_str("                }\n");
        out.push_str("                let result = unsafe {\n");
        out.push_str("                    std::slice::from_raw_parts(\n");
        out.push_str("                        self.data.add(PAYLOAD_OFFSET),\n");
        out.push_str("                        payload_len,\n");
        out.push_str("                    )\n");
        out.push_str("                }.to_vec();\n");
        out.push_str("                return Ok(result);\n");
        out.push_str("            }\n");
        out.push_str("            spin += 1;\n");
        out.push_str("            if spin > 1_000_000 {\n");
        out.push_str("                return Err(format!(\"timeout waiting for LZ response (call_id={})\", call_id));\n");
        out.push_str("            }\n");
        out.push_str("            #[cfg(not(windows))]\n");
        out.push_str("            std::hint::spin_loop();\n");
        out.push_str("        }\n");
        out.push_str("    }\n\n");

        // drop
        out.push_str("    /// 关闭共享内存连接\n");
        out.push_str("    pub fn close(self: Box<Self>) {\n");
        out.push_str("        #[cfg(unix)] {\n");
        out.push_str("            unsafe {\n");
        out.push_str("                libc::munmap(self.data as *mut _, self.size);\n");
        out.push_str("            }\n");
        out.push_str("        }\n");
        out.push_str("    }\n");
        out.push_str("}\n\n");

        // 注册的宿主函数列表
        if !self.host_funcs.is_empty() {
            out.push_str("// ═══ Registered host functions ═══\n");
            out.push_str("// Available to LZ scripts via `import embed::host::<name>`\n\n");
            for (name, ep) in &self.host_funcs {
                if !ep.doc.is_empty() {
                    out.push_str(&format!("// {}\n", ep.doc));
                }
                out.push_str(&format!(
                    "// pub fn {}(...) -> ...  // signature: {}\n",
                    name, ep.signature
                ));
            }
            out.push('\n');
        }

        out
    }

    /// 生成 LZ 侧调用 shim（宿主函数包装为 LZ 可调用函数）
    pub fn generate_lz_shims(&self) -> String {
        let mut out = String::new();

        out.push_str("// ═══ Lang-Zone Embed Bridge — LZ Side Shims ═══\n");
        out.push_str("// Generated by lzc --bridge=embed\n\n");

        if self.host_funcs.is_empty() {
            return out;
        }

        out.push_str("// Host functions callable from LZ:\n");
        out.push_str("// Usage: `import embed::host::<func>; result = host::<func>(args)`\n\n");

        for (name, ep) in &self.host_funcs {
            out.push_str(&format!("/// embed::host::{} — {}\n", name, ep.signature));
            if ep.is_async {
                out.push_str(&format!("// async fn embed_host_{}(...) -> ... {{ /* shim to shm_call */ }}\n\n", name));
            } else {
                out.push_str(&format!(
                    "// fn embed_host_{}(...) -> ... {{ /* shim to shm_call */ }}\n\n",
                    name
                ));
            }
        }

        out
    }

    /// 生成导出函数注册代码（LZ 函数暴露给宿主的包装）
    pub fn generate_export_wrappers(&self) -> String {
        let mut out = String::new();

        out.push_str("// ═══ Lang-Zone Embed Bridge — Export Wrappers ═══\n");
        out.push_str("// LZ functions exposed to the host application\n\n");

        if self.exported_funcs.is_empty() {
            out.push_str("// (no exported functions registered)\n");
            return out;
        }

        out.push_str("// Exported LZ functions (callable from host via embed::<name>):\n");
        for (name, ep) in &self.exported_funcs {
            if !ep.doc.is_empty() {
                out.push_str(&format!("/// {} — {}\n", name, ep.doc));
            }
            out.push_str(&format!(
                "// pub fn embed_export_{}(...) -> ... // signature: {}\n",
                name, ep.signature
            ));
        }

        out
    }

    /// 宿主函数数量
    pub fn host_func_count(&self) -> usize {
        self.host_funcs.len()
    }

    /// 导出函数数量
    pub fn exported_func_count(&self) -> usize {
        self.exported_funcs.len()
    }
}

// ══════════════════════════════════════════════════════════════
// Bridge trait 实现
// ══════════════════════════════════════════════════════════════

impl Bridge for EmbedBridge {
    fn name(&self) -> &str { "embed" }

    fn level(&self) -> BridgeLevel { BridgeLevel::SharedMemory }

    fn capabilities(&self) -> BridgeCapability {
        BridgeCapability::IMPORT
            | BridgeCapability::FUNCTION_CALL
            | BridgeCapability::HOT_RELOAD
    }

    fn meta(&self) -> BridgeMeta {
        BridgeMeta {
            version: "0.1.0".into(),
            description: format!(
                "embed bridge: {} host funcs, {} exported, shm={} ({} bytes)",
                self.host_funcs.len(),
                self.exported_funcs.len(),
                self.shm_name,
                self.shm_size
            ),
            provides: vec!["embed".into(), "lz_embed".into()],
            ..Default::default()
        }
    }

    fn resolve_import(&self, module_path: &[String], _items: &[String]) -> Option<ImportResolveResult> {
        if module_path.is_empty() {
            return None;
        }
        if module_path[0] != "embed" {
            return None;
        }

        let func_name = module_path.get(2).cloned()?;
        if self.host_funcs.contains_key(&func_name) {
            Some(ImportResolveResult {
                rust_path: format!("embed::host::{}", func_name),
                type_aliases: vec![],
                requires_shim: true,
                is_tier2: false,
                feature_flags: vec!["lz_embed".into()],
                extern_crates: vec![],
                error: None,
            })
        } else {
            None
        }
    }

    fn resolve_import_full(&self, module_path: &[String], _items: &[String]) -> Option<ImportResolveResult> {
        self.resolve_import(module_path, &[])
    }

    fn resolve_call(&self, func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        let func_name = func_name.strip_prefix("embed::host::").unwrap_or(func_name);
        if self.host_funcs.contains_key(func_name) {
            Some(CallResolveResult {
                rust_path: format!("_lz_embed_host_{}", func_name),
                shim: String::new(),
                module_name: "embed".into(),
                is_macro: false,
                is_template: false,
                ret_result: false,
            })
        } else {
            None
        }
    }

    fn resolve_call_full(&self, func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        self.resolve_call(func_name, _args)
    }

    fn list_exports(&self, kind: ExportKind) -> Vec<ExportEntry> {
        match kind {
            ExportKind::Function => {
                let mut entries = Vec::new();
                for (name, ep) in &self.host_funcs {
                    entries.push(ExportEntry {
                        name: name.clone(),
                        kind: ExportKind::Function,
                        signature: ep.signature.clone(),
                        module: "embed".into(),
                    });
                }
                for (name, ep) in &self.exported_funcs {
                    entries.push(ExportEntry {
                        name: name.clone(),
                        kind: ExportKind::Function,
                        signature: ep.signature.clone(),
                        module: "lz_export".into(),
                    });
                }
                entries
            }
            ExportKind::Module => {
                vec![
                    ExportEntry {
                        name: "embed".into(),
                        kind: ExportKind::Module,
                        signature: format!("embed shm={}", self.shm_name),
                        module: String::new(),
                    },
                    ExportEntry {
                        name: "lz_export".into(),
                        kind: ExportKind::Module,
                        signature: "LZ exports to host".into(),
                        module: String::new(),
                    },
                ]
            }
            _ => vec![],
        }
    }

    fn export_count(&self) -> usize {
        self.host_funcs.len() + self.exported_funcs.len()
    }
}

// ══════════════════════════════════════════════════════════════
// 单元测试
// ══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bridge() -> EmbedBridge {
        let mut bridge = EmbedBridge::new("test");
        bridge.register_host_func(EmbedEndpoint {
            name: "get_time_ms".to_string(),
            signature: "fn() -> i64".to_string(),
            is_async: false,
            doc: "Get current time in milliseconds".to_string(),
        });
        bridge.register_host_func(EmbedEndpoint {
            name: "send_event".to_string(),
            signature: "fn(name: str, data: &[u8]) -> i32".to_string(),
            is_async: true,
            doc: "Send event to game engine".to_string(),
        });
        bridge.register_exported_func(EmbedEndpoint {
            name: "on_click".to_string(),
            signature: "fn(x: i32, y: i32) -> ()".to_string(),
            is_async: false,
            doc: "Called when user clicks".to_string(),
        });
        bridge
    }

    #[test]
    fn test_embed_bridge_new() {
        let bridge = EmbedBridge::new("game");
        assert_eq!(bridge.name(), "embed");
        assert_eq!(bridge.level(), BridgeLevel::SharedMemory);
        assert!(bridge.capabilities().contains(BridgeCapability::FUNCTION_CALL));
        assert!(bridge.capabilities().contains(BridgeCapability::HOT_RELOAD));
    }

    #[test]
    fn test_embed_bridge_server_client() {
        let server = EmbedBridge::new("test").as_server();
        let client = EmbedBridge::new("test").as_client();
        // 配置相同，路径相同
        assert_eq!(server.shm_path, client.shm_path);
    }

    #[test]
    fn test_embed_bridge_shm_path() {
        let bridge = EmbedBridge::new("myapp");
        #[cfg(windows)]
        assert_eq!(bridge.shm_path, "Global\\lzem_myapp");
        #[cfg(not(windows))]
        assert_eq!(bridge.shm_path, "/dev/shm/lzem_myapp");
    }

    #[test]
    fn test_embed_bridge_with_size() {
        let bridge = EmbedBridge::new("test").with_size(4096);
        assert_eq!(bridge.shm_size, 4096);
    }

    #[test]
    fn test_embed_constants() {
        assert_eq!(LZEM_MAGIC, 0x4C5A454D);
        assert_eq!(LZEM_VERSION, 1);
        assert_eq!(LZEM_HEADER_SIZE, 16);
        assert_eq!(LZEM_REQUEST_OFFSET, 16);
        assert_eq!(LZEM_RESPONSE_OFFSET, 80);
        assert_eq!(LZEM_PAYLOAD_OFFSET, 144);
    }

    #[test]
    fn test_header_is_valid() {
        let valid = LzemHeader {
            magic: 0x4C5A454D,
            version: 1,
            size: 1024,
        };
        assert!(valid.is_valid());

        let invalid_magic = LzemHeader {
            magic: 0xDEADBEEF,
            version: 1,
            size: 1024,
        };
        assert!(!invalid_magic.is_valid());

        let future_version = LzemHeader {
            magic: 0x4C5A454D,
            version: 999,
            size: 1024,
        };
        // version > CURRENT_VERSION 也视为无效
        assert!(!future_version.is_valid());
    }

    #[test]
    fn test_request_response_default() {
        let req = LzemRequest::default();
        assert_eq!(req.version.load(Ordering::SeqCst), 0);
        assert_eq!(req.call_id, 0);
        assert_eq!(req.func_name_len, 0);
        assert_eq!(req.payload_len, 0);

        let resp = LzemResponse::default();
        assert_eq!(resp.version.load(Ordering::SeqCst), 0);
        assert_eq!(resp.status, 0);
        assert_eq!(resp.payload_len, 0);
        assert_eq!(resp.error_len, 0);
    }

    #[test]
    fn test_resolve_import_embed_host() {
        let bridge = make_bridge();
        let path = vec!["embed".into(), "host".into(), "get_time_ms".into()];
        let r = bridge.resolve_import_full(&path, &[]).unwrap();
        assert_eq!(r.rust_path, "embed::host::get_time_ms");
        assert!(r.requires_shim);
        assert!(r.feature_flags.contains(&"lz_embed".into()));
    }

    #[test]
    fn test_resolve_import_non_embed_ignored() {
        let bridge = make_bridge();
        let path = vec!["std".into(), "io".into()];
        assert!(bridge.resolve_import_full(&path, &[]).is_none());
    }

    #[test]
    fn test_resolve_import_unknown_func() {
        let bridge = make_bridge();
        let path = vec!["embed".into(), "host".into(), "nonexistent".into()];
        assert!(bridge.resolve_import_full(&path, &[]).is_none());
    }

    #[test]
    fn test_resolve_call() {
        let bridge = make_bridge();
        let r = bridge.resolve_call("embed::host::get_time_ms", &[]).unwrap();
        assert_eq!(r.rust_path, "_lz_embed_host_get_time_ms");
    }

    #[test]
    fn test_resolve_call_unknown() {
        let bridge = make_bridge();
        assert!(bridge.resolve_call("embed::host::nonexistent", &[]).is_none());
    }

    #[test]
    fn test_generate_host_module() {
        // 默认（客户端）模式：生成 connect()
        let bridge = make_bridge();
        let code = bridge.generate_host_module();
        assert!(code.contains("struct LzemSession"));
        assert!(code.contains("pub fn connect()"));
        assert!(code.contains("pub fn call("));
        assert!(code.contains("const LZEM_MAGIC"));
        assert!(code.contains("const LZEM_VERSION"));
        assert!(!code.contains("pub fn create()"));

        // 服务端模式：生成 create()
        let server = make_bridge().as_server();
        let server_code = server.generate_host_module();
        assert!(server_code.contains("struct LzemSession"));
        assert!(server_code.contains("pub fn create()"));
        assert!(server_code.contains("pub fn call("));
        assert!(!server_code.contains("pub fn connect()"));
    }

    #[test]
    fn test_generate_host_module_contains_all_funcs() {
        let bridge = make_bridge();
        let code = bridge.generate_host_module();
        assert!(code.contains("get_time_ms"));
        assert!(code.contains("send_event"));
    }

    #[test]
    fn test_generate_lz_shims() {
        let bridge = make_bridge();
        let code = bridge.generate_lz_shims();
        assert!(code.contains("embed::host::get_time_ms"));
        assert!(code.contains("embed::host::send_event"));
    }

    #[test]
    fn test_generate_export_wrappers() {
        let bridge = make_bridge();
        let code = bridge.generate_export_wrappers();
        assert!(code.contains("on_click"));
    }

    #[test]
    fn test_list_exports_functions() {
        let bridge = make_bridge();
        let exports = bridge.list_exports(ExportKind::Function);
        assert_eq!(exports.len(), 3); // 2 host + 1 exported
        assert!(exports.iter().any(|e| e.name == "get_time_ms" && e.module == "embed"));
        assert!(exports.iter().any(|e| e.name == "send_event" && e.module == "embed"));
        assert!(exports.iter().any(|e| e.name == "on_click" && e.module == "lz_export"));
    }

    #[test]
    fn test_list_exports_modules() {
        let bridge = make_bridge();
        let exports = bridge.list_exports(ExportKind::Module);
        assert!(exports.iter().any(|e| e.name == "embed"));
        assert!(exports.iter().any(|e| e.name == "lz_export"));
    }

    #[test]
    fn test_export_count() {
        let bridge = make_bridge();
        assert_eq!(bridge.export_count(), 3);
    }

    #[test]
    fn test_host_func_count() {
        let bridge = make_bridge();
        assert_eq!(bridge.host_func_count(), 2);
        assert_eq!(bridge.exported_func_count(), 1);
    }

    #[test]
    fn test_meta_description() {
        let bridge = make_bridge();
        let meta = bridge.meta();
        assert!(meta.description.contains("2 host funcs"));
        assert!(meta.description.contains("1 exported"));
        assert!(meta.description.contains("lzem_test"));
    }

    #[test]
    fn test_capabilities_no_import() {
        // embed bridge 有 HOT_RELOAD 能力（脚本可热重载）
        let bridge = EmbedBridge::new("test");
        let caps = bridge.capabilities();
        assert!(caps.contains(BridgeCapability::FUNCTION_CALL));
        assert!(caps.contains(BridgeCapability::HOT_RELOAD));
        assert!(caps.contains(BridgeCapability::IMPORT));
        assert!(!caps.contains(BridgeCapability::METHOD_CALL));
        assert!(!caps.contains(BridgeCapability::TYPE_REWRITE));
    }

    #[test]
    fn test_empty_bridge() {
        let bridge = EmbedBridge::new("empty");
        assert_eq!(bridge.host_func_count(), 0);
        assert_eq!(bridge.exported_func_count(), 0);
        assert_eq!(bridge.export_count(), 0);

        let code = bridge.generate_host_module();
        assert!(code.contains("struct LzemSession"));

        // 空 host_funcs：shims 仅含头部、无函数条目
        let shims = bridge.generate_lz_shims();
        assert!(shims.contains("LZ Side Shims"));
        assert!(!shims.contains("embed_host_"));

        // 空 exported_funcs：导出包装给出显式提示
        let wrappers = bridge.generate_export_wrappers();
        assert!(wrappers.contains("no exported functions"));
    }

    #[test]
    fn test_resolve_call_full() {
        let bridge = make_bridge();
        let r = bridge.resolve_call_full("embed::host::send_event", &[]).unwrap();
        assert_eq!(r.rust_path, "_lz_embed_host_send_event");
        assert_eq!(r.module_name, "embed");
        assert!(!r.is_macro);
    }

    #[test]
    fn test_resolve_import_short_path() {
        let bridge = make_bridge();
        // 路径不足 3 个元素
        let path = vec!["embed".into(), "host".into()];
        assert!(bridge.resolve_import_full(&path, &[]).is_none());
    }
}
