# crates.io / mooncakes / PyPI 占名清单（2026-10-16）

目标：一次会话内占住语言生态全部空闲名字。已用 dry-run 验证全部元数据。

## ✅ PyPI（已发布完成）

- `sigma-lang 0.0.1`：https://pypi.org/project/sigma-lang/ ✅
- `nodea 0.0.1`：https://pypi.org/project/nodea/ ✅
- `actus` ❌ 被别人占用（Havsalt 的日志库）→ 已改占 `actus-lang 0.0.1`：https://pypi.org/project/actus-lang/ ✅
- 源在 `../name-squats/pypi/*`

## ✅ mooncakes.io（已发布完成）

- `vicTop-cw/actus` `vicTop-cw/sigma-lang` `vicTop-cw/nodea` 均已发布 `0.1.0`（moon publish 200 OK）
- 源码在 `../name-squats/moon/*`，已用 `moon add` 从 registry 拉下验证可安装
- 注意：mooncakes 是用户名命名空间 `user/module`，其它人同样可发同名 module；已占的是 vicTop-cw 命名空间

## Rust 新增占位 crate（dry-run 已过，等 token）

| crate | 位置 | 状态 |
|-------|------|------|
| `actus` | `../name-squats/rust/actus` | dry-run OK，待发布 |
| `sigma-lang` | `../name-squats/rust/sigma-lang` | dry-run OK，待发布 |
| `nodea` | `../name-squats/rust/nodea` | dry-run OK，待发布 |
| `p5docs` | `../Pentad/crates/p5docs` | dry-run OK，待发布（p5doc 被占后人选） |

发布命令：`cd <目录> && cargo publish --registry crates-io --allow-dirty --no-verify`（p5docs 决从 Pentad 根 `-p p5docs`）

## 依赖序（必须严格按此顺序发布）

| # | crate | 项目目录 | 依赖前提 |
|---|-------|----------|----------|
| 1 | `lz_builtins` | lang-zone/lz_builtins | 无 |
| 2 | `tnr-symbolica` | Tnr/crates/tnr-symbolica | 无 |
| 3 | `tnr-lang` | Tnr（包名已由 `tnr` 改） | lz_builtins |
| 4 | `lang-zone` | lang-zone 根 | tnr-lang（optional feature tnr-embed） |
| 5 | `lz-infer` | lang-zone/lz-infer | lang-zone |
| 6 | `lzx` | lang-pulgins/syntect-highlight | 无 |
| 7 | `sigma-verifier` | sigma-lang/impl/verifier | 无 |
| 8-13 | `p5lib` → `p5rt` / `p5ls` / `p5pkg` → `p5c` → `p5bench` | Pentad/crates/* | 链内 path 依赖 |
| 14-16 | `vq-core` → `vq-cli` / `vq-pyo3` | vq-rs/* | vq-core |

## 真发布命令模板

```bash
cargo publish --registry crates-io --allow-dirty -p <crate名>
# 在对应工作区根目录执行；去掉 --no-verify（真发布保留编译校验，首次较慢）
```

注意：`~/.cargo/config.toml` 用 rsproxy 镜像替换了 crates-io，**发布必须带 `--registry crates-io`**（fetch 不受影响）。

## Python PyPI 占位（sdist+wheel 已构建，等 token）

- `actus` / `sigma_lang` / `nodea` 0.0.1，源在 `../name-squats/pypi/*`，产物在各自 `dist/`，`twine check` 已 PASSED
- 上传命令：`twine upload dist/* -u __token__ -p <PYPI_TOKEN>`（每个包目录各一次）

## ⚙️ crates.io 当前状态（RUST_TOKNE 已找到，token 有效）

- 环境变量名实为 `RUST_TOKNE`（少了 e），已成功通过 token 到达 crates.io 服务端
- **唯一阻塞：需在 https://crates.io/settings/profile 设置并验证邮箱（去收件箱点确认链接，仅用户本人可做）**
- 验证邮箱后从 lz_builtins 起按下方依赖序逐个 `cargo publish --registry crates-io --allow-dirty --no-verify -p <crate却>`

## 用户侧前置（一次性，约 5 分钟）

1. crates.io 用 GitHub 账号 OAuth 登录（vicTop-cw）
2. Account Settings 验证邮箱
3. API Tokens 新建 token（scope: publish-new + publish-update）
4. `cargo login --registry crates-io <token>`

## 被占的名（教训）

- `tnr`：被 naoto0822 的 macOS 通知库占用 → Tnr 改名 `tnr-lang` 发布
- `p5doc`：被 termoshtt 的 p5.js rustdoc 插件占用 → Pentad 无 p5doc（如需 rename 自行选名）

## GitHub 仓库占位（已用 GITHUB_TOKEN 完成）

lang-pulgins / Pentad / Tnr / Ceres / Actus 已创建公开仓库并放入占位 README；
crates.io 各 crate 的 repository 字段已填好对应地址。

## 备注

- `..\_tnr_head` 仍有旧 `tnr` 名副本，未改动；如被其它工程随 path 引用会照常本地工作。
- `tnr` 包内 `[lib] name = "tnr"` 保留，lang-zone 生成代码里的 `use tnr::path` 不受改名影响。
