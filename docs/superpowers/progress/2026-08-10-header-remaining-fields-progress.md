# Header Remaining Fields Progress

更新时间：2026-08-10

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/header-remaining-fields`（基于 `main`）
- 设计规范：`docs/superpowers/specs/2026-08-10-header-remaining-fields-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-10-header-remaining-fields.md`

## 已提交实现

| 阶段 | ECS commit | 内容 |
| --- | --- | --- |
| Task 1 | `ff355e3` | header.rs 剩余字段解码函数（含 PciBridgeHeader） |
| Task 2 | `44fe05d` + `8ec6085` | PciDeviceDetails 字段 + inspect() 接线 |
| Task 3 | `acae126` | text/JSON 渲染（含桥窗口与 Bridge Control） |
| 真机校准 | `3a5df21` | 桥窗口 enabled 判定修正为 base <= limit |

## 真机验证结果（2026-08-10）

### dev48（QEMU PCI-PCI 桥 0000:00:1f.0）

- bus: primary=00 secondary=01 subordinate=04 ✅ 与 lspci 一致
- IO 窗口 0xc000-0xdfff [8K] ✅、Memory 0xc2000000-0xc24fffff [5M] ✅
- Prefetchable 0xc2600000-0xc27fffff [2M]：我们标注 64-bit（原始 base
  bit0=1，PCI 规范判定），dev48 定制版 lspci 标 32-bit——保留规范判定

### sg-232e-224（物理机）

- 桥 0000:00:08.0：bus 号一致；IO/Memory/Prefetchable 窗口均
  `disabled`（原始 base>limit）✅ 与 lspci `[disabled]` 一致
- X710 endpoint 3d:00.0：cache line 32 bytes、multifunction=true、
  INTA 等字段正常

## 校准记录

1. **桥窗口 enabled 判定**：初版用"raw 非零"启发式，真机关闭态窗口
   （base=0xf000, limit=0xfff）触发减法溢出 panic；修正为
   `base <= limit`，与 lspci 判定一致。

## 已知差异（非错误）

- interrupt line 输出原始寄存器值（0xff=未分配），lspci 显示路由后的
  IRQ 号（来源不同，均为事实）。
- dev48 lspci 为厂商定制版，个别标注与规范冲突时保留规范判定。

## 回归检查

- myece：fmt/check/diff check 通过；9/9 设备；config dump 输出不变
- dev48/sg：既有 capability 解码不变

## 下一切片候选

- LnkCap2 / latency/timeout ns/µs 文本化
- AER root 寄存器组、SR-IOV VF BAR
- 配置空间写入
