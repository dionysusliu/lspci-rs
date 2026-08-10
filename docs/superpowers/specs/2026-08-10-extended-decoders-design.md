# Extended Capability Decoders Design

日期：2026-08-10
状态：已与用户确认（brainstorming 定稿）

## 目标

为扩展配置空间（0x100+）的 capability 增加协议级解码（B 切片首轮），
覆盖 sg-232e-224 网卡 3d:00.0 上存在的五个扩展 capability：

| 扩展 Cap ID | 名称 | 复杂度 |
| ---: | --- | --- |
| 0x01 | AER（Advanced Error Reporting） | 高（含逐位名称展开） |
| 0x03 | Device Serial Number (DSN) | 极低 |
| 0x0A | Access Control Services (ACS) | 低 |
| 0x0B | Alternative Routing-ID Interpretation (ARI) | 低 |
| 0x0D | Single Root I/O Virtualization (SR-IOV) | 中 |

## 明确不做（留下一切片）

- TPH、Secondary PCIe、DPC、PTM、VC、Lane Margining、RCEC 等其余扩展 cap
- VPD 内容事务读取、配置空间写入
- 单元测试（沿用用户决定：只做真机验证）
- 不引入新的 Rust 依赖

## 验证环境

- **sg-232e-224 物理机**（曙光跳板机接入）：Alibaba Cloud Linux 3、
  glibc 2.32、libpci.so.3、lspci 3.8.0——与容器二进制完全兼容；
  免密 sudo；扩展空间 4096 字节完整可读；
  网卡 3d:00.0 同时具备 AER/DSN/ARI/SR-IOV/ACS（+TPH/Secondary PCIe）。
- 二进制分发链：容器 → myece 宿主 → 本机 → sg-232e-224（均用 sftp；
  scp 在本会话权限环境会被 kill）。

## 架构（完全复用既有框架）

- decoder 纯函数：`fn decode_x(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<XCapability>`，
  新文件放在 `crates/pci/src/decoders/`（aer.rs / dsn.rs / acs.rs / ari.rs / sriov.rs）
- `PciCapabilityContent` 枚举新增 5 个变体；名称表已覆盖这些扩展 ID
- **session 接线扩展**：`inspect()` 目前只对 standard 链解码，
  本切片对 extended 链同样做"预取 + decode_content"；
  每个 Valid 扩展节点预取 `[offset, offset+0x40)`（覆盖 SR-IOV/AER 全结构）
- `decode_content` 分发增加扩展 ID 分支（按 `kind == Extended` 区分命名空间，
  标准/扩展 ID 数值重叠——如 0x01 既是 PM 又是 AER）

## 各 decoder 字段集（lspci -vv/-vvv 基准）

### DSN (0x03)

cap+4 起 8 字节序列号。Text：`serial=c6:2e:ff:ff:79:73:34:ad`
（对齐 lspci `ad-c6-2e-ff-ff-79-73-34` 的字节序展示，实现时以 lspci 输出为准校准）。

### ARI (0x0B)

cap+4 word capability（next function number bits 8-15 等）、cap+6 word control。
字段：next_function、mfvc、acs、ato 等 capability 位与 control 使能位
（对齐 lspci `ARICap: ... ARICtl: ...`，以真机输出校准）。

### ACS (0x0A)

cap+4 word ACS capability、cap+6 word ACS control、
cap+8 起 egress control vector（长度由 capability 位决定，可为 0）。
字段：能力位集合、使能位集合、egress vector 长度与原始字节。
对齐 lspci `ACSCap: ... ACSCtl: ...`。

### SR-IOV (0x0D)

寄存器（相对 cap base）：+0x04 dword capabilities、+0x08 word control、
+0x0A word status、+0x0C initial VFs、+0x0E total VFs、+0x10 num VFs、
+0x12 function dependency link、+0x14 VF device ID、
+0x16 reserved、+0x18/0x1C Supported/System Page Size、+0x20..0x38 VF BAR0–5（6 个 dword）、+0x38/0x3C VF Migration State Array Offset/Size。
字段与 lspci `SR-IOV: ... Initial VFs ..., Total VFs ..., NumVFs ..., VF Device ...` 对齐。

### AER (0x01) —— 全量展开（用户要求一步到位）

寄存器（相对 cap base）：

| 偏移 | 寄存器 |
| ---: | --- |
| +0x04 | Uncorrectable Error Status |
| +0x08 | Uncorrectable Error Mask |
| +0x0C | Uncorrectable Error Severity |
| +0x10 | Correctable Error Status |
| +0x14 | Correctable Error Mask |
| +0x18 | AER Capabilities & Control（version bits 3:0、first error pointer bits 12:8） |
| +0x1C | Header Log（16 字节，4×dword） |
| +0x2C/0x30/0x34 | Root Error Command / Status / Error Source ID（仅桥设备） |
| +0x38 | TLP Prefix Log（16 字节，4×dword） |

version 从扩展 cap 头（offset+0 dword 的 bits 19:16）读取。

**逐位名称展开**：UE/CE 的 status/mask/severity 均输出 `Name+`/`Name-`
序列（与 lspci 的 `UESta: DLP- SDES- TLP- FCP- ...` 形态一致）。
位名称表按 PCIe 规范：

- UE：DLP(4) SDES(5) TLP(8) FCP(9) CmpltTO(10) CmpltAbrt(11) UnxCmplt(12)
  RxOF(13) MalfTLP(14) ECRC(15) UnsupReq(16) ACSViol(17) UncorrIntErr(18)
  BlockedTLP(19) AtomicOpBlocked(20) TLPPrefixBlocked(21)
- CE：RxErr(0) BadTLP(6) BadDLLP(7) Rollover(8) Timeout(9)
  AdvNonFatalErr(13) CorrIntErr(14) HeaderOF(15)

（括号内为位号；实现时与 sg-232e-224 的 lspci 输出逐项校准。）

桥设备判定：读配置空间 0x0E 字节 header type（& 0x7f == 1）决定是否
解码 root-only 寄存器；endpoint 跳过。

## 渲染

- text：extended 节点下增加 content 行；AER 的位展开按 lspci 风格多行
  （UESta/UEMsk/UESvrt/CESta/CEMsk + header log hex）
- JSON：`content.type` 取值 `aer` / `dsn` / `acs` / `ari` / `sr_iov`；
  位展开在 JSON 中用字符串数组（如 `ue_status_bits: ["RxOF"]`）+
  原始 hex 并存

## 验证

1. **sg-232e-224 真机对照**（3d:00.0，sudo）：五个 decoder 与
   `sudo lspci -s 3d:00.0 -vvv` 逐字段对照（AER 含位展开、SR-IOV 含
   VF 数/BAR、DSN 序列号、ARI/ACS 位）。
2. **dev48/myece 不回归**：两环境扩展空间不可读，输出必须保持
   `extended: chain=unavailable: ReadError` 不变。
3. handoff 进度文档记录对照结果与校准过的位号差异。

## 全局约束

- decoder 模块零 FFI；解码失败（字节不可读）不得使 `inspect()` 失败。
- 不改变 `list` 行为；不引入新依赖。
- 分支策略：从 main 切 `sdd/extended-decoders`，完成后走
  finishing-a-development-branch。
