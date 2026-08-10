# Remaining Extended Capability Decoders Progress

更新时间：2026-08-10

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/remaining-extended-decoders`（基于 `main`）
- 设计规范：`docs/superpowers/specs/2026-08-10-remaining-extended-decoders-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-10-remaining-extended-decoders.md`

## 已提交实现

| 阶段 | ECS commit | 内容 |
| --- | --- | --- |
| Task 1 | `4524101` | extended 预取 0x40 → 0x60 |
| Task 2 | `759bcec` | LTR / ATS / PRI / PASID / PTM decoder |
| Task 3 | `6b7463c` | DPC / TPH decoder |
| Task 4 | `611f72b` | Vendor-Ext / DVSEC dump decoder |
| Task 5 | `fe60cfd` | VC / Secondary PCIe decoder |
| Task 6 | `4417c9b` | text/JSON 渲染 11 个变体 |
| 真机校准 | （本次提交） | PTM/TPH/DPC/VC 布局与位号修正、DVSEC 命名补全 |

## sg-232e-224 真机验证结果（2026-08-10，sudo 对照 lspci -vvv）

| capability | 样本设备 | 结果 |
| --- | --- | --- |
| PTM (0x1f) | 0000:00:08.0 | ✅ requester/responder/root 能力位、granularity=2ns 与 lspci 一致 |
| LTR (0x18) | 0000:75:01.0 | ✅ snoop/no-snoop latency 一致（均为 0） |
| DPC (0x1d) | 0000:00:08.0 | ✅ Trigger/Reason/INT/RPBusy/ErrPtr=0x1f/Source 全一致 |
| TPH (0x17) | 0000:3d:00.0 | ✅ device_specific 支持、无 ST table（location=0）一致 |
| VC (0x02) | 0000:75:01.0 | ✅ LPEVC=1、2 个 VC 资源，寄存器与 lspci 逐项吻合 |
| ATS/PRI/PASID | 0000:76:00.0 | ✅ 原始寄存器一致（见下方说明 1、2） |
| Secondary PCIe (0x19) | 0000:3d:00.0 | ✅ LnkCtl3 位与 LaneErrStat 一致 |
| Vendor-Ext (0x0b) | 0000:16:01.0 | ✅ ID/Rev/Len/dump 与 lspci 一致 |
| DVSEC (0x23) | 0000:75:03.1 | ✅ Vendor=8086/ID/Rev/Len/dump 与 lspci 一致 |

## 校准记录（计划假设被推翻的项）

1. **PTM**：capability 位为 Requester=bit0 / Responder=bit1 / Root=bit2；
   granularity 在 capability bits 8–15（计划误放在 control）。
2. **TPH**：位定义按 pciutils：Interrupt Vector=bit1、Device Specific=bit2、
   Extended Requester=bit8、ST Table Location=bits9–10（0 无 / 1 在 cap / 2 在 MSI-X）、
   ST Table Size=bits16–26；"No ST mode" 位不存在，已删除该字段。
3. **DPC**：INT Msg 为 bits0–4（计划 bits0–2）；RPExt=bit5、PoisonedTLP=bit6、
   SwTrigger=bit7、RP PIO Log Size=bits8–11、DL_ActiveErr=bit12；
   status 布局：Trigger=0、Reason=1–2、INT=3、RPBusy=4、TriggerExt=5–6、
   RP PIO ErrPtr=8–13。
4. **VC**：lspci 实际寄存器顺序为 per-VC **control(+0x14) / status(+0x18) /
   capability(+0x1c)**、stride 12（与 PCIe 规范的 cap/ctrl/status 顺序相反，
   以 lspci 输出反推并逐位验证）；port 级 cr1 bits0–2 evc_count、bits4–6 LPEVC、
   bits8–9 RefClk、bits10–11 PATEntryBits。
5. **PASID 宽度差异说明**：本机 lspci 显示 `Max PASID Width: 14`，但原始
   寄存器 bits8–12 = 20（与规范及上游 pciutils 宏一致）；该机 lspci 为
   云厂商定制版（含非上游的 IOMMU group 输出），我们保留规范原值。
6. **PRI stopped 语义说明**：lspci 的 `Stopped+` 为派生状态（Enable 且无
   Stop Request），我们输出原始 status 位。

## 回归检查

- myece：9/9 设备；`extended: chain=unavailable: ReadError` 不变；fmt/diff check 通过
- dev48：`extended: chain=unavailable: ReadError` 不变；slot-id/hot-plug 解码保持正常

## 未实现（spec 明确不做）

Lane Margining、Phys Layer 16GT、RCEC、MR-IOV、Multicast、ReBAR、DPA、
PM-Mux（机器上部分无样本或价值低）。

## 下一切片候选

- header 字段语义解读（Command/Status、BAR 类型、Header Type）
- 配置空间写入
- 剩余扩展 cap（若遇到相应硬件）
