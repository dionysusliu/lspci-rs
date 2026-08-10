# `show` 命令设计

## 状态

已确认设计，等待用户审阅后进入实现计划。

## 背景

当前项目已经可以在真实 Alibaba Cloud Linux 3 ECS 环境中通过 libpci 扫描 PCI 设备，并以 text/JSON 格式输出设备列表。

下一阶段增加单设备只读查询：

```text
lspci-rs show <PCI_ADDRESS>
lspci-rs show <PCI_ADDRESS> --format json
```

该功能用于学习 PCI 设备信息模型、libpci 的延迟字段填充、Linux PCI backend 以及 Rust FFI 生命周期，同时为后续 TUI 详情面板提供数据接口。

## 目标

- 查询一个指定 PCI function 的详细信息；
- 继续以 libpci 作为系统交互层的主要接口；
- 使用 libpci 自动选择 Linux backend，通常为 `linux-sysfs`；
- 把 C 指针和 libpci 生命周期限制在 `PciSession` 内部；
- 将字段缺失与命令级失败区分开；
- 对无法确认原因的字段保持诚实，不猜测为权限问题；
- 同时支持 text 和 JSON 输出；
- 在真实 ECS 环境中验证普通用户和 root 用户的差异。

## 非目标

第一版不实现：

- 修改 PCI 配置空间；
- reset、remove、rescan、unbind 或 bind 驱动；
- 完整配置空间十六进制 dump；
- PCI capability 的完整语义解析；
- TUI；
- 独立的 sysfs 主数据读取器；
- libpci fatal callback 的 Rust 级错误恢复。

## 命令接口

### `show`

```text
lspci-rs show <PCI_ADDRESS>
```

地址使用完整 BDF 格式：

```text
<domain>:<bus>:<slot>.<function>
```

例如：

```text
lspci-rs show 0000:00:05.0
```

第一版支持已有的输出格式选择：

```text
--format text
--format json
```

### 地址解析

地址解析在 CLI 层完成，解析失败时不创建 libpci context，并返回用户可理解的参数错误。

设备地址本身标识 PCI function，而不是一定意义上的完整物理卡。一个 slot 可以包含多个 function。

## 分层架构

```text
CLI
  ├── 解析 PCI 地址
  ├── 创建 PciSession
  ├── 调用 inspect(address)
  └── 调用 text/json renderer

pci crate
  ├── PciSession
  │     └── libpci context
  │           └── Linux 通常选择 linux-sysfs backend
  ├── PciDevice
  ├── PciDeviceDetails
  ├── PciInspection
  └── PciField<T>

pci-sys crate
  ├── bindgen 生成的 libpci bindings
  └── 当前阶段只保留 header 与 callback shim 的预留位置
```

libpci 是跨平台的 PCI 访问抽象层，而 `linux-sysfs` 是其 Linux backend。第一版不重新实现 sysfs 数据读取；只有未来需要捕获逐字段 errno 时，才增加诊断辅助路径。

参考：

- [pcilib(7)](https://www.man7.org/linux/man-pages/man7/pcilib.7.html)
- [libpci `pci.h`](https://kernel.googlesource.com/pub/scm/utils/pciutils/pciutils/+/refs/heads/master/lib/pci.h)
- [Linux sysfs backend](https://kernel.googlesource.com/pub/scm/utils/pciutils/pciutils/+/cb94f26e6933815fe4484cd2697ae7e1ffdcd9ac/lib/sysfs.c)

## 数据模型

### `PciDevice`

`PciDevice` 继续表示列表阶段已经拥有的稳定身份信息：

- `PciAddress`
- vendor ID
- device ID
- class ID
- vendor name
- device name
- class name

不把所有详细字段都塞入列表模型，以免 `list` 默认承担昂贵或可能需要权限的读取操作。

### `PciField<T>`

字段不能只使用 `Option<T>`，因为缺少值可能有不同原因：

```rust
pub enum PciField<T> {
    Available(T),
    Unavailable {
        reason: PciFieldUnavailableReason,
    },
    NotApplicable,
}
```

原因枚举至少包含：

```rust
pub enum PciFieldUnavailableReason {
    PermissionDenied,
    UnsupportedByBackend,
    UnsupportedByLibrary,
    DeviceUnavailable,
    NotBound,
    ReadError,
    Unknown,
}
```

第一版只有在底层明确提供权限错误时才使用 `PermissionDenied`。仅仅因为 `known_fields` 缺少某个位，不能推断为权限问题。

驱动没有绑定是合法状态，应显示为 `NotBound`，不能和读取失败混淆。

### `PciDeviceDetails`

```rust
pub struct PciDeviceDetails {
    pub revision: PciField<u8>,
    pub programming_interface: PciField<u8>,
    pub subsystem_vendor_id: PciField<u16>,
    pub subsystem_device_id: PciField<u16>,
    pub parent: PciField<PciAddress>,
    pub irq: PciField<u32>,
    pub driver: PciField<String>,
    pub resources: PciField<Vec<PciResource>>,
}
```

资源先保留 libpci 提供的地址、大小和 flags：

```rust
pub struct PciResource {
    pub index: u8,
    pub start: u64,
    pub size: u64,
    pub flags: u64,
}
```

第一版不急于把 flags 转成复杂 Rust 枚举，先保留原始值，后续再增加 Memory/IO、prefetchable 等语义层。

### `PciInspection`

```rust
pub struct PciInspection {
    pub device: PciDevice,
    pub details: PciDeviceDetails,
}
```

## Session API

保留已有的列表接口：

```rust
impl PciSession {
    pub fn scan(&mut self) -> Result<PciSnapshot, PciError>;
}
```

增加单设备详情接口：

```rust
impl PciSession {
    pub fn inspect(
        &mut self,
        address: PciAddress,
    ) -> Result<PciInspection, PciError>;
}
```

`inspect()` 的内部流程：

1. 通过 libpci 扫描总线；
2. 按 BDF 找到目标 `pci_dev`；
3. 调用 `pci_fill_info()` 请求第一版详细字段；
4. 根据返回的 `known_fields` 生成 `PciField<T>`；
5. 将 C 结构中的值复制成 Rust 自有数据；
6. 返回 `PciInspection`，不让 C 指针离开 session。

第一版不会让 CLI 直接持有 `*mut pci_dev`。

## libpci 字段请求

Alibaba Cloud Linux 3 当前使用 pciutils 3.8.0。第一版重点确认并使用这些 flags：

```c
PCI_FILL_IDENT
PCI_FILL_CLASS
PCI_FILL_IRQ
PCI_FILL_BASES
PCI_FILL_SIZES
PCI_FILL_CLASS_EXT
PCI_FILL_SUBSYS
PCI_FILL_DRIVER
PCI_FILL_PARENT
```

如果 bindings 中包含 capability flags，则预留：

```c
PCI_FILL_CAPS
PCI_FILL_EXT_CAPS
```

但 capability 解析属于后续阶段；第一版只确认字段是否可用，或输出 capability 的 ID/offset 摘要。

`pci_fill_info()` 是延迟填充接口，返回值是 `known_fields` 位掩码。它不提供完整的逐字段 errno，因此 Rust 层必须保留 `Unknown` 原因。

## 错误策略

### 命令级错误

以下情况终止 `show`：

- libpci context 分配失败；
- 找不到任何可用的访问 backend；
- PCI 总线扫描失败；
- 目标 BDF 不存在；
- FFI 返回不可恢复错误。

### 字段级不可用

以下情况继续输出其他字段：

- 某字段未出现在 `known_fields`；
- 某字段对当前设备不适用；
- 当前用户无权读取某段配置空间；
- 当前 backend 不支持某字段。

text 输出示例：

```text
IRQ: <unavailable: unknown reason>
Driver: <not bound>
BAR4: <not applicable>
```

JSON 输出使用结构化状态，而不是把原因拼进普通字符串：

```json
{
  "irq": {
    "status": "unavailable",
    "reason": "unknown"
  },
  "driver": {
    "status": "not_bound"
  }
}
```

### libpci callback

libpci 的 `error` callback 是 C variadic callback，并具有“不返回”的语义；默认实现会把错误输出到 stderr 后退出。第一版不在 Rust 中直接接管这个 callback。

未来如果需要将 fatal error 转成 `Result`，应实现 C shim：

```text
C shim 设置 C 侧恢复边界
  ↓
调用 pci_init / pci_scan_bus / pci_fill_info
  ↓
C callback 格式化消息
  ↓
C shim 返回错误码和消息
  ↓
Rust 转换为 PciError
```

不能从 C callback 直接跨 Rust 栈执行 `longjmp`。

## 输出设计

### text

采用面向人阅读的分段格式：

```text
0000:00:05.0 Ethernet controller: Red Hat, Inc. Virtio network device

Identity:
  Vendor: 0x1af4 (Red Hat, Inc.)
  Device: 0x1000 (Virtio network device)
  Class:  0x0200 (Ethernet controller)
  Revision: 0x00

Resources:
  BAR0: ...

Kernel:
  Driver: virtio-pci
  IRQ: 11
```

字段不可用时显示状态和原因，不伪造默认值。

### JSON

JSON 保持机器可读结构：

- `address`
- `identity`
- `details`
- 每个字段的 `status`、`value` 或 `reason`

不把诊断原因只编码到人类文本中。

## PCI 学习节点

实现过程中按以下顺序解释：

1. BDF 为什么标识 PCI function；
2. PCI 配置空间和普通 MMIO resource 的区别；
3. Vendor/Device/Class/Revision 在配置空间中的位置；
4. BAR 如何描述设备需要的 I/O 或内存窗口；
5. IRQ 是传统配置字段还是 Linux 最终使用的中断视图；
6. driver binding 为什么属于 Linux device model/sysfs，而不是设备身份寄存器；
7. `pci_fill_info()` 为什么采用延迟填充和 bitmask；
8. capability list 如何通过配置空间 offset 连接起来。

## 真实环境验证

只使用 ECS 上的真实系统响应：

```bash
lspci -s 0000:00:05.0 -v
lspci -s 0000:00:05.0 -k
cat /sys/bus/pci/devices/0000:00:05.0/irq
readlink /sys/bus/pci/devices/0000:00:05.0/driver
```

验证重点：

- 目标地址正确匹配；
- 普通用户与 root 的字段差异被保留；
- 未绑定驱动与读取失败不混淆；
- text/JSON 字段语义一致；
- 不访问或修改 PCI 写接口。

## 后续扩展

- 配置空间 hex dump；
- PCI capability 结构化解析；
- sysfs 诊断辅助路径；
- C callback shim；
- TUI 详情面板；
- 只读拓扑树；
- 在明确的安全确认机制下再考虑写操作。
