# Read-Only Enhancements Progress

更新时间：2026-08-11

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/readonly-enhancements`（基于 `main`）
- 设计规范：`docs/superpowers/specs/2026-08-11-readonly-enhancements-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-11-readonly-enhancements.md`

## 已提交实现

| 阶段 | ECS commit | 内容 |
| --- | --- | --- |
| Task 1 | `15d9b65` | LnkCap2 解码（速度向量 + Crosslink/Retimer/DRS） |
| Task 2 | `57a0d3a` | latency 编码 → ns/µs 文本 |
| Task 3 | `6303528` | AER root 组（RootErrCmd/RootErrSta/ErrSrc） |
| Task 4 | `86f7138` | SR-IOV VF BAR 类型化解码 + migration size |
| 校准 | `1c7088d` | LnkCap2 速度渲染改为 min-max 区间形态 |
| 校准 | `940e537` | 零值 VF BAR 跳过 + AER root 状态标签对齐规范 |

## sg-232e-224 真机校准记录

### X710 endpoint 0000:3d:00.0

- **LnkCap2**：`Supported Link Speeds: 2.5-8GT/s, Crosslink- Retimer- 2Retimers- DRS-`
  与 lspci 逐字一致（速度位 bits1–3；DRS bit10 为 -，与该设备能力一致）。
- **Latency**：`Latency L0s <512ns, L1 <64us`、`Exit Latency L0s <2us, L1 <16us`
  与 lspci 一致。
- **SR-IOV VF Region**：Region 0 `memory-64-prefetch at 0xd3fff410000`、
  Region 3 `memory-64-prefetch at 0xd3ffe000000` 与 lspci `Region 0/3` 一致。

### Root Port 0000:00:08.0

- RootErrCmd/RootErrSta/ErrSrc 与 lspci `RootCmd/RootSta/ErrorSrc` 位位置
  完全吻合（该设备全零）；标签对齐 PCIe 规范名
  （bit4=FirstUEFatal、bit5=NonFatalMsg、bit6=FatalMsg、bits27–31 IntMsg）。

## 校准中发现并修正的问题

1. **LnkCap2 速度渲染**：初版输出列表（2.5-5-8），lspci 用 min-max 区间
   （2.5-8GT/s），已改。
2. **零值 VF BAR**：初版把全零 BAR 渲染为 `memory-32 at 0x0`，lspci 跳过，
   已加跳过逻辑。
3. **AER RootSta 标签**：初版 bit5/6 标签（FatalErr/MultFatal）与规范不符，
   改为 NonFatalMsg/FatalMsg。

## 回归检查

- dev48：QEMU NVMe latency/LnkCap2 输出正常（8-16GT/s）
- myece：fmt/check/diff check 通过；9/9 设备；extended 链
  `unavailable: ReadError` 不变

## 下一切片候选

- 配置空间写入（唯一剩下的非只读项）
- 其余扩展 cap（DP C、ACS 之外的 TPH/DPC/PTM 细节扩展，若需要）
