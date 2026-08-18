---
AIGC:
    Label: "1"
    ContentProducer: 001191440300708461136T1XGW3
    ProduceID: 9f2a11add43fbf12a546606fb2b962ab_56583cea996d11f19467525400287e28
    ReservedCode1: xm7iIl2Y9wbQZ4cbnOMQ2iZ5Eaa6Mc22v8hPSFqivNjtOOT6kVX9xYwT9p1JceZzqUBoy5k/Tj7F2e9sqzO7hKQl2j0ap+Bi3dQRiJc7YYP/skZs4pYPQvRllIf/eSl0/L7kGtSLyXIcVh/qQu+0RgQe1FGBEkckB7iO9lSWfkC/qDFTVvuqQ04qr+8=
    ContentPropagator: 001191440300708461136T1XGW3
    PropagateID: 9f2a11add43fbf12a546606fb2b962ab_56583cea996d11f19467525400287e28
    ReservedCode2: xm7iIl2Y9wbQZ4cbnOMQ2iZ5Eaa6Mc22v8hPSFqivNjtOOT6kVX9xYwT9p1JceZzqUBoy5k/Tj7F2e9sqzO7hKQl2j0ap+Bi3dQRiJc7YYP/skZs4pYPQvRllIf/eSl0/L7kGtSLyXIcVh/qQu+0RgQe1FGBEkckB7iO9lSWfkC/qDFTVvuqQ04qr+8=
---

# PROJECT-SPEC/05 — Omega 验证前提

> 本文件说明 FIST 的 Omega（Ω）验证系统：为什么必须先行、Ω-spec JSON 格式、Ω-gate 跑批、准确率定义。
> **项目级任务验收前必须过 Omega 验证。**

---

## 1. 为什么必须验证

| 理由 | 说明 |
|------|------|
| **验证是验收的先决条件** | 没有 Ω-gate，AI 生成的项目产出无法客观验证"对/错"，无从验收 |
| **spec 先行决定实现** | 实现必须服从 spec；没有 Ω-spec 就没有实现依据 |
| **防语义漂移** | 先有验证器，才有"共识"锚点；后补验证器 = 让实现"定义"语义，危险 |
| **提供准确率** | 验证通过率 = 项目产出的客观质量指标，供终审决策 |

## 2. Ω-spec JSON 格式

每个验证对象（操作/回传格式/结构）对应一份 spec JSON，放在 `corpus/`：

```json
{
  "op": "tensor_add",
  "version": "1.0",
  "definition": {
    "signature": "add(a: Tensor, b: Tensor) -> Tensor",
    "note": "逐元素加法，同 dtype 同 shape"
  },
  "preconditions": ["a.shape == b.shape", "dtype_id(a) == dtype_id(b)"],
  "laws": ["L1", "L15"],
  "tests": [
    {"input": {"a": [1, 2], "b": [3, 4]}, "expected": [4, 6]},
    {"input": {"a": [1, 2], "b": [1, 2, 3]}, "error": "ShapeError"}
  ],
  "fingerprint": "fnv1a64:xxxxxxxx"
}
```

字段说明：

| 字段 | 说明 |
|------|------|
| `op` | 验证对象名（唯一） |
| `version` | spec 版本 |
| `definition` | 定义（signature + note） |
| `preconditions` | 前置条件（字符串列表） |
| `laws` | 关联定律/规则编号 |
| `tests` | 测试对：`input → expected`（含 `error` 错误路径） |
| `fingerprint` | 内容指纹（fnv1a64），防篡改 |

## 3. Ω-gate 跑批验证

- `fist.py omega-verify <project_dir>`：对项目执行验证；
- 验证维度：
  1. **结构验证**：`project_dir` 是否符合 PROJECT-SPEC/01 标准结构；
  2. **回传格式验证**：任务回传是否符合"结论/证据/分析/缺口与风险/建议入档位置"五段式；
  3. **spec 测试对验证**：`corpus/` 下每个 spec JSON 的测试对跑批（input → expected，含错误路径）。
- 每个用例：通过 / 失败（含原因）。

## 4. 准确率定义

```
项目准确率 = 通过用例数 / 总用例数 × 100%
```

- 结构验证、回传格式、spec 测试对三类用例统一计入；
- 准确率 < 100% 时，任务不得标记完成（铁律 1）；
- 验证结果落盘 `project_dir/reports/YYYY-MM-DD/omega-<时间>.json`，并写入 FIST 的 `omega_runs` 表。

## 5. 分阶段方案（Ω 随 FIST 演进）

| 阶段 | 内容 | 版本 |
|------|------|------|
| **Ω-min（先做）** | Ω-spec JSON 解析/校验 + Ω-gate 跑批 + 结构验证 | v0.2（本次） |
| **Ω-core** | 三端验证（主实现 vs 参考实现 vs 金标准）、定律证明 | v0.3+ |
| **Ω-loop** | 自演化驱动（验证→反馈→版本推进） | v0.4+ |
*（内容由AI生成，仅供参考）*
