# PCIe Capability Full Decoding Progress

更新时间：2026-08-10

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/pcie-full-decoding`（基于 `main`）
- 设计规范：`docs/superpowers/specs/2026-08-10-pcie-full-decoding-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-10-pcie-full-decoding.md`

## 已提交实现

| 阶段 | ECS commit | 内容 |
| --- | --- | --- |
| Task 1 | `e4fc201` | pcie.rs 全量重写：16 寄存器组、嵌套结构、条件组 |
| Task 2 | `b33a17b` | text 多行 + JSON 嵌套对象渲染 |
| 真机校准 | `7e016a9` | LnkSta2 de-emphasis/retimer、(ok) 后缀、DevCap2/DevCtl2 ARI/NROPrPrP 渲染 |

## sg-232e-224 真机校准记录

### X710 网卡 0000:3d:00.0（endpoint，PCIe v2）

DevCap/DevCtl/DevSta/LnkCap/LnkCtl/LnkSta/DevCap2/DevCtl2/LnkCtl2/LnkSta2
全部与 `lspci -vv` 逐行一致（MaxPayload 2048/512、MaxReadReq 4096、
LnkCap 8GT/s x8 ASPM L1、LnkSta 8GT/s x8、Completion Timeout 0f/09、
EqualizationComplete/Phase1-3+ 等）。

校准修正：
1. **LnkSta2 de-emphasis 语义**：bit0=1 → -3.5dB、bit0=0 → -6dB
   （初版渲染方向相反；原始值 0x001e + lspci "-6dB" 证实）。
2. **LnkSta2 retimer 显示**：bits6–7 编码，0=无、1=Retimer、2=2Retimers；
   渲染映射为 lspci 的 `Retimer-` 形态。
3. **非降速时不显示 "(ok)"**：对齐 lspci（仅降速时显示 "(downgraded)"）。
4. **DevCap2/DevCtl2 渲染补全**：ARI 与 NROPrPrP 标志初版漏渲染；
   原始值（DevCap2=0x0c37）证实位定义本身正确。

### Root Port 0000:00:08.0（Slot-）

RootCtl（ErrCorrectable/Non-Fatal/Fatal+、PMEInterrupt+、CRSVisible+）
与 RootSta（PME ReqID/Status/Pending）与 lspci 一致；Slot 寄存器组正确
地未渲染（slot_implemented=false）。

### dev48 辅助

QEMU NVMe endpoint（v2，LnkCap 16GT/s x16）渲染正常。

## 已知差异（非错误）

- Latency L0s/L1 与 Exit Latency 输出原始编码值，未转 ns/µs 文本。
- Completion Timeout 输出原始编码（如 0f/09），未转范围文本。
- LnkCap2 寄存器组未解码（本切片范围外）。

## 回归检查

- myece：fmt/check/diff check 通过；9/9 设备；extended 链
  `unavailable: ReadError` 不变
- dev48：capability 解码保持正常

## 下一切片候选

- LnkCap2 / Slot Power 细节
- AER root 寄存器组、SR-IOV VF BAR 细节
- header 其余字段（Header Type、BIST、Expansion ROM、Interrupt Pin）
- 配置空间写入
