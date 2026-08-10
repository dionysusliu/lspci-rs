# Header Field Semantic Decoding Progress

更新时间：2026-08-10

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/header-field-decoding`（基于 `main`）
- 设计规范：`docs/superpowers/specs/2026-08-10-header-field-decoding-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-10-header-field-decoding.md`

## 已提交实现

| 阶段 | ECS commit | 内容 |
| --- | --- | --- |
| Task 1 | `15aa352` | header.rs：Command/Status/BAR 纯解码函数 |
| Task 2 | `1c855f8` | details.rs 字段扩展 + inspect() 重构（零新增读取） |
| Task 3 | `da079dc` | text/JSON 渲染（control/status/BAR type） |
| handoff | （本次提交） | 验证记录 |

## 真机验证结果（2026-08-10）

位表按 PCI 规范实现，三机对照全部吻合，无需校准。

| 环境 | 设备 | 结果 |
| --- | --- | --- |
| myece 容器 | 0000:00:05.0 | ✅ control/status/BAR type 正常输出（header 区可读） |
| dev48 | 0000:00:05.0 (NVMe) | ✅ BusMaster/DisINTx/fast devsel 一致；BAR0 memory-64-prefetch 16K ↔ lspci `(64-bit, prefetchable) [size=16K]` |
| dev48 | 0000:00:1f.0 (桥) | ✅ 66MHz+/FastB2B+ 一致；BAR0 memory-64 non-prefetch 256 ↔ lspci 一致 |
| sg-232e-224 | 0000:3d:00.0 (X710) | ✅ BAR0 8M/BAR3 32K memory-64-prefetch ↔ lspci `[size=8M]`/`[size=32K]`；BAR 编号正确跳过 64 位占用的上槽 |

说明：dev48/sg 的 lspci 为厂商定制版，header 输出形态为 `Flags:` 而非
`Control:/Status:`，语义逐项核对一致。

## 回归检查

- myece：fmt/check/diff check 通过；9/9 设备；config dump 输出不变
- dev48：slot-id/hot-plug capability 解码保持正常
- JSON：`command`/`status` 对象（bool 位 + devsel 字符串）、resources
  增加 `bar_type` 字符串

## 下一切片候选

- Header Type（设备/桥）、BIST、Expansion ROM、Interrupt Pin 解读
- 配置空间写入
- 其余扩展 cap（Lane Margining/Phys16GT/RCEC，遇到相应硬件再补）
