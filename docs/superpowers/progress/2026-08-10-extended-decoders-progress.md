# Extended Capability Decoders Progress

更新时间：2026-08-10

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/extended-decoders`（基于 `main`）
- 设计规范：`docs/superpowers/specs/2026-08-10-extended-decoders-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-10-extended-decoders.md`

## 已提交实现

| 阶段 | ECS commit | 内容 |
| --- | --- | --- |
| Task 1 | `a638519` | dispatch 按 (kind, id) 匹配；session 对 extended 链预取 + 解码 |
| Task 2 | `b296908` | DSN / ARI / ACS decoder |
| Task 3 | `13d039e` | SR-IOV / AER decoder（含 UE/CE 位名称表） |
| Task 4 | `4e40743`（+`9c99853` 导出修复） | text/JSON 渲染，AER 多行位展开 |
| 真机校准 | `2e1a2f0` | ID 表与寄存器布局按物理机证据修正 |

## 真机校准记录（sg-232e-224，Intel X710 网卡 0000:3d:00.0）

计划中的三处假设被物理机证据推翻并修正：

1. **扩展 cap ID**：ARI = 0x0e、SR-IOV = 0x10、ACS = 0x0d
   （计划/spec 原假设 0x0b/0x0d/0x0a 均错误）；同时修正 names.rs
   扩展 ID 表（MR-IOV 0x11、PRI 0x13、TPH 0x17、LTR 0x18、DPC 0x1d 等）。
2. **AER UE 位号**：DLP=4 SDES=5 TLP=12 FCP=13 CmpltTO=14 CmpltAbrt=15
   UnxCmplt=16 RxOF=17 MalfTLP=18 ECRC=19 UnsupReq=20 ACSViol=21
   UncorrIntErr=22 BlockedTLP=23 AtomicOpBlocked=24 TLPPrefixBlocked=25。
   校准证据：UESta=0x00100000（bit 20）与 lspci `UnsupReq+` 吻合。
3. **SR-IOV 寄存器布局**：VF Offset/Stride 在 +0x14/+0x16（word），
   VF Device ID +0x1a，Supported/System Page Size +0x1c/+0x20，
   VF BAR0–5 从 +0x24 起，Migration Offset +0x40。
4. **AER 结构止于 +0x38**：标准 AER 无 TLP Prefix Log 寄存器，
   原实现读取 +0x38..+0x48 越界到下一个 capability，已删除该字段。
5. **DSN 字节序**：lspci 以高字节优先显示（ad-c6-...），渲染改为倒序。

## sg-232e-224 真机验证结果（2026-08-10，sudo 对照 lspci -vvv）

| capability | 结果 |
| --- | --- |
| AER (0x100) | ✅ UESta/UEMsk/UESvrt/CESta/CEMsk 全部位标志一致；HeaderLog 一致 |
| DSN (0x140) | ✅ serial ad:c6:2e:ff:ff:79:73:34 ↔ lspci ad-c6-2e-ff-ff-79-73-34 |
| ARI (0x150) | ✅ capability=0x0100 → Next Function 1；control=0 |
| SR-IOV (0x160) | ✅ Initial/Total VFs=64、NumVFs=0、offset 16、stride 1、VF Device ID 0x154c |
| ACS (0x1b0) | ✅ capability/control 全 0，与 lspci 全 `-` 一致 |
| TPH (0x1a0) / Secondary PCIe (0x1d0) | 本切片不做 decoder（留下一切片），链发现与命名正常 |

JSON 输出确认：`content.type` = aer/dsn/ari/acs/sr_iov，位展开为字符串数组
（如 `ue_status_bits: ["UnsupReq"]`），与 hex 并存。

## 回归检查

- myece：9/9 设备；`extended: chain=unavailable: ReadError` 不变
- dev48：`extended: chain=unavailable: ReadError` 不变；slot-id/hot-plug 解码保持正常
- `cargo fmt --check` / workspace check：通过

## 下一切片候选

- 其余扩展 decoder：TPH、Secondary PCIe、DPC、PTM、VC、Lane Margining、
  LTR、PASID 等（sg-232e-224 全机均有样本可验证）
- header 字段语义解读（Command/Status、BAR 类型）
- 配置空间写入
