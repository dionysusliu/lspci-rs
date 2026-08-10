# More Standard Capability Decoders Progress

更新时间：2026-08-10

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/more-standard-decoders`（基于 `main`）
- 设计规范：`docs/superpowers/specs/2026-08-10-more-standard-decoders-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-10-more-standard-decoders.md`

## 已提交实现

| 阶段 | ECS commit | 内容 |
| --- | --- | --- |
| Task 1 | `a324dc2` | VPD / Slot ID / PCI-X / Hot-Plug decoder + 枚举变体 + 分发 |
| Task 2 | `e2c5c3e` | text/JSON 渲染四个新变体 |
| 真机修正 | `d973e96` | Slot ID 位布局与 Hot-Plug 语义按 dev48 证据修正 |

## 真机验证中发现并修复的问题

dev48 `0000:00:1f.0`（QEMU PCI-PCI bridge）对照发现两处偏差：

1. **Slot ID 位布局**：原始字节 `0x20`，lspci 显示 `0 slots, First+`。
   正确布局为 bits 0–4 = 槽位数、bit 5 = First（计划最初假设 bit 7 错误）。
2. **Hot-Plug 语义**：cap+2 字节为 `0x00`，但 lspci 仍显示 `Hot-plug capable`。
   lspci 以 capability 存在本身为据，不读标志位；decoder 改为同语义。

这正是坚持真机对照的价值——两个错误都无法靠静态审查发现。

## dev48 真机验证结果（2026-08-10）

| capability | 偏移 | 结果 |
| --- | --- | --- |
| Slot Identification (0x04) | 0x048 | ✅ `slots=0 first=true chassis=0x01` ↔ lspci `Slot ID: 0 slots, First+, chassis 01` |
| Hot-Plug (0x0c) | 0x040 | ✅ `hot_plug_capable=true` ↔ lspci `Hot-plug capable` |
| VPD (0x03) | — | ⚠️ dev48 无 VPD 设备，本环境不可验证 |
| PCI-X (0x07) | — | ⚠️ dev48 无 PCI-X 设备，本环境不可验证 |

JSON 输出确认：`content.type` 为 `slot_id` / `hot_plug`，字段形态符合 spec。

## myece 不回归检查

- `cargo fmt --all -- --check`：通过
- `cargo check --workspace`：通过（无警告）
- `list`：9/9 设备
- `show --config standard`：输出与合并前一致
- `git diff --check`：通过

## 下一步

1. whole-branch review，决定是否合并回 `main`。
2. B 切片（扩展 capability decoder：AER/SR-IOV/ARI/ACS 等）：
   验证环境已确认为 sg-232e-224 物理机（Ice Lake，扩展空间 4096 字节
   完整可读，网卡 3d:00.0 具备 AER/DSN/ARI/SR-IOV/TPH/ACS，免密 sudo）。
