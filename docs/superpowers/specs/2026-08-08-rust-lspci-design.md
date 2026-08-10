# Rust lspci 设计

日期：2026-08-08  
状态：已完成设计审阅，等待用户审阅文档  
第一阶段：只读 PCI 枚举 CLI

## 1. 目标与范围

这个项目使用 Rust 构建一个现代化的 `lspci`，同时作为学习 PCI 设备管理、Rust FFI 和 Linux 设备模型的工程。

首个垂直切片只完成一个目标：

> 使用 Rust 通过 `libpci` 枚举真实 Linux 系统上的 PCI 设备，并提供文本和 JSON 输出。

第一阶段的约束：

- 只支持 Linux。
- 只读、非特权，不要求 `sudo`。
- 使用系统提供的 `libpci`，不 vendoring `pciutils`/`libpci`。
- 以 Ali ECS 作为真实运行环境。
- 使用 `bindgen` 生成原始绑定，再手写安全 Rust 封装。
- 用户自己负责编码；本设计用于指导实现、拆解和后续审查。

第一阶段不实现 TUI，但会为未来的树型拓扑 TUI 保留稳定的 session 和 snapshot 边界。

## 2. 已确认的架构选择

项目采用 Cargo workspace，分为三个 crate：

```text
lspci-rs CLI
    │ 参数解析、输出格式、退出码
    ▼
pci 安全库
    │ 创建上下文、扫描设备、转换 Rust 类型
    ▼
pci-sys
    │ bindgen 原始绑定、ABI 与链接配置
    ▼
libpci + Linux PCI 系统
```

### `pci-sys`

`pci-sys` 只提供 `bindgen` 生成的 C ABI 声明、相关类型和链接配置。它是唯一允许直接接触 `unsafe` C API 的 crate，不承担设备语义和 CLI 逻辑。

### `pci`

`pci` 提供安全边界和领域类型，负责：

- 创建、初始化和释放 `libpci` 访问上下文。
- 扫描 PCI 总线并请求首个切片所需的信息。
- 检查 C 返回值和空指针。
- 将 C 内存中的地址、ID 和名称复制为拥有型 Rust 数据。
- 将名称缺失、权限不足和系统条件不满足映射为明确的占位状态。
- 不向上层暴露裸指针或依赖 C 生命周期的字符串。

### `lspci-rs`

CLI 只负责参数解析、调用 `pci`、渲染文本/JSON 和设置进程退出码。它不重新访问 `libpci`，也不依赖 `libpci` 内部结构。

## 3. Session 与 snapshot 生命周期

核心访问对象命名为 `PciSession`。它拥有一个长生命周期的 `libpci` 上下文；`scan()` 生成不依赖 C 指针的设备 snapshot。

`list` 的生命周期是：

```text
创建 PciSession → 初始化 libpci → 扫描 → 复制为 snapshot → 渲染 → 释放 session
```

这是“一次命令一个 session”，不是“每个设备一次 session”。

未来 TUI 的生命周期是：

```text
创建 PciSession → 进入事件循环 → 按需 refresh/rescan → 替换 snapshot → 退出时释放
```

普通移动、筛选、排序和展开操作只访问当前 snapshot，不触发 C 调用。只有显式刷新、定时刷新或事件驱动的失效提示才重新扫描。

扫描先保持同步、单线程，不承诺 `PciSession: Send + Sync`，避免在未确认 `libpci` 线程语义前提供错误保证。

官方 `pciutils/lspci` 源码也采用一次进程内创建访问上下文、初始化、扫描并最终清理的模式；`libpci` 访问上下文本身持有设备列表和名称数据库状态：

- [pciutils lspci.c](https://kernel.googlesource.com/pub/scm/utils/pciutils/pciutils/%2B/v3.2.1/lspci.c)
- [pciutils lib/pci.h](https://kernel.googlesource.com/pub/scm/utils/pciutils/pciutils/%2B/refs/tags/v3.1.0/lib/pci.h)

## 4. CLI 契约

首个入口固定为：

```text
lspci-rs list
lspci-rs list --format text
lspci-rs list --format json
lspci-rs --help
lspci-rs --version
```

第一阶段不兼容传统 `lspci` 短参数，避免同时维护两套语义。未来可以增加兼容层，但不属于本切片。

## 5. 设备数据模型

`PciDevice` 是拥有型数据，包含：

```text
address:
  domain: u16
  bus: u8
  slot: u8
  function: u8
  display: "0000:00:1f.3"

vendor_id: u16
device_id: u16
class_id: u16

vendor_name: String
device_name: String
class_name: String
```

地址字符串由结构化地址规范化生成，不从 C 的展示字符串反向解析。原始 ID 保留数值语义；JSON 渲染时使用固定宽度十六进制字符串，保留前导零并避免消费者误解。

可读名称统一使用尖括号占位符表达状态：

- `<unknown>`：名称数据库无法解析。
- `<permission denied>`：后续需要权限的可选字段无法读取。
- `<not available>`：字段在当前系统或设备上不存在/不可用。

第一阶段主要会产生 `<unknown>`。JSON 也使用这些字符串，不使用 `null`，从而保留状态信息并保持字段类型稳定。原始地址和 ID 永远是结构化字段，不使用占位符。

首个切片只包含 BDF、厂商/设备/类别 ID 和对应名称；不包含配置空间全文、驱动、模块、NUMA、链路能力或修改操作。

## 6. 错误策略

- 访问上下文初始化失败：命令失败，stderr 给出原因，返回非零退出码。
- 总线扫描失败：命令失败，不输出伪造的设备集合。
- 单个设备的必需数值信息无法读取：报告设备地址和原因，终止本次扫描。
- 名称解析失败：保留原始 ID，名称为 `<unknown>`。
- 后续可选字段因权限或系统条件不可读：使用对应尖括号占位符，不影响其他设备。

`Drop` 负责释放 C 资源。CLI 不需要知道 `libpci` 的释放细节。

## 7. 构建与部署

已通过只读 SSH 确认 Ali ECS 环境：

- 架构：`x86_64`
- 系统：Alibaba Linux 3，EL8 用户态体系
- 内核：`5.10.134-19.7.al8.x86_64`
- glibc：`2.32`
- 已安装 `pciutils-3.8.0`、`pciutils-libs-3.8.0`
- `/usr/sbin/lspci` 可用
- 未安装 `pciutils-devel` 和 Rust

发布目标固定为 `x86_64-unknown-linux-gnu`。ECS 只运行二进制，不承担 Rust 编译。

构建链：

```text
macOS 本地编码
      │
      └─ EL8 兼容 Linux builder/sysroot
                │
                └─ GitHub Actions 构建 release binary
                           │
                           └─ SSH 上传到 Ali ECS
                                      │
                                      └─ 真实 smoke check
```

构建环境需要系统 `libpci` 开发文件、`pkg-config`、Clang/libclang、Rust toolchain 和 `bindgen` 所需的头文件。运行环境只需要兼容的动态 `libpci.so.3` 和目标系统运行库。

不能直接假定最新 Ubuntu runner 构建的二进制能够运行在 glibc 2.32 上，因为构建机可能引入更高版本的 glibc 符号。GitHub Actions 和本地 macOS 都应使用同一个 EL8 兼容 builder 或 sysroot；构建后仍以 ECS 运行结果为最终依据。

第一阶段优先动态链接系统 `libpci`，让实际运行验证目标服务器的库和 ABI，不追求静态打包。

## 8. 真实验证标准

不构造 PCI fixture，也不把完整自动化测试体系作为首阶段交付重点。最低验证路径：

```text
cargo build
cargo run -- list
cargo run -- list --format json
```

在 EL8 兼容 builder 构建 release binary，上传到 ECS 后确认：

1. 普通用户无需 Rust、`pciutils-devel` 或 `sudo` 即可运行。
2. `list` 能初始化 `libpci` 并返回真实设备集合。
3. 输出包含规范化 BDF、原始 ID 和名称或占位符。
4. JSON 可解析，且文本与 JSON 的设备集合一致。
5. 初始化/扫描错误有清晰原因并返回非零退出码。
6. 用系统 `lspci` 或 `/sys/bus/pci/devices` 做人工交叉核对。
7. 记录构建目标、发行版、内核、glibc 和 `libpci` 版本。

Ali ECS 不一定提供真实 PCI 热插拔场景，因此事件通知不阻塞第一阶段验证。

## 9. 未来 TUI 与事件模型

TUI 优先采用树型拓扑布局：

```text
domain → bus → bridge → device
```

建议键位：

- `↑/↓`：移动当前节点。
- `←/→`：折叠/展开。
- `Enter`：查看设备详情。
- `r`：使用现有 session 刷新 snapshot。
- `q`：退出。

顶部保留轻量状态栏，显示 domain 数、设备数、刷新时间和错误/权限提示；不把大盘式视图作为主导航。

Linux 内核可以通过 kobject uevent 通知用户态设备添加、移除和部分变化；PCI hotplug 子系统也会向用户态发布 uevent。`libpci` 本身没有事件订阅 API，未来应由独立的 Linux 事件适配层监听 uevent/netlink 或 `libudev`：

- [Linux PCI Hotplug Support Library](https://docs.kernel.org/5.17/driver-api/pci/pci.html)
- [Linux kobject uevent](https://docs.kernel.org/6.16/core-api/kobject.html)

事件只表示“snapshot 可能失效”，不代表完整状态。TUI 应保留手动刷新，并可增加低频定时刷新。任意 PCI 配置空间变化不保证产生通用事件，因此不能完全依赖事件流。

PCI 树不能直接由 `libpci` 的线性设备链假设得到；后续需要结合 bridge 的 bus range 或 Linux sysfs 拓扑推导。首个 `list` 切片只输出平面设备集合，不承担拓扑构建。

## 10. 明确非目标

以下内容不属于首个实现计划：

- TUI 和键盘事件循环。
- uevent/netlink 监听。
- 配置空间详细读取。
- 设备启停、配置写入、驱动绑定/解绑。
- 传统 `lspci` 参数兼容。
- BSD、macOS 或其他非 Linux 后端。
- mock backend 和人工 fixture。
- 完整自动化测试体系或 CI 门禁。

这些内容保留为后续独立阶段，避免首个 FFI 学习切片过早扩大。
