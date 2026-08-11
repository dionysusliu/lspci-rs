# CLI Enhancement Design: Color Output and Topology Tree

日期：2026-08-11
状态：已与用户确认（brainstorming 定稿）

## 目标

增强 CLI 输出：语义着色（TTY 自动检测 + `--color` 控制）与设备拓扑树
子命令（`tree`，对齐 `lspci -t` 形态）。零新增依赖。

## 明确不做

- 交互式 TUI / Web UI（用户已明确推迟）
- JSON 输出着色（JSON 永不着色）
- 配置空间写入
- 不引入新的 Rust 依赖（纯 ANSI 转义码 + `std::io::IsTerminal`）

## 验证环境

- **myece**：list/show/tree、管道自动无色、`--color always` 强制彩色
- **dev48**：桥拓扑与 `sudo lspci -t` 对照
- 回归：现有 text/JSON 输出内容不变（仅追加颜色码）

## 架构（方案 1：集中式渲染层）

### 1. 着色层 `crates/lspci-rs/src/color.rs`

```rust
pub enum ColorMode { Auto, Always, Never }

impl ColorMode {
    /// Auto → stdout 是否 TTY（std::io::IsTerminal）
    pub fn enabled(self) -> bool;
}
```

语义着色函数（启用时包裹 ANSI 码，否则原样返回）：

| 语义 | 颜色 |
| --- | --- |
| 设备地址 | 青色加粗 |
| 字段名（vendor/device/class 等） | 暗色 |
| ID 数值（0x...） | 暗色 |
| 设备/厂商名称 | 默认色 |
| 不可读/失败值（`<unavailable: ...>`） | 红色 |
| capability 名 | 绿色 |
| `disabled` / `not-applicable` | 暗色 |
| config dump 偏移列 | 暗色 |

### 2. output.rs 参数化

- `render_text`、`render_inspection_text` 增加 `color: ColorMode` 参数；
  内部所有 writeln 经着色函数包装
- JSON 渲染不变、永不着色
- `--color auto|always|never` 全局参数（clap global = true），默认 auto；
  list / show / tree 三个子命令生效

### 3. tree 子命令 `crates/lspci-rs/src/tree.rs`

- 数据源：`session.scan()`（list 级，不逐设备读配置空间）
- 拓扑推导：对每个桥设备读取 header 总线窗口
  （0x19 secondary bus、0x1a subordinate bus，各 1 字节，myece 可读区
  内）；子设备按"bus 号落在某桥的 [secondary, subordinate] 区间内、
  且取最内层桥"归属
- 输出 lspci -t 风格：

```text
-[0000:00]-+-00.0 Intel 440FX PMC
           +-1f.0-[01-04]---- ...
           +-05.0 Virtio network device
```

- 桥节点显示 `-[secondary-subordinate]` 子总线区间
- 无桥窗口数据（不可读）的桥降级为普通节点
- 受 `--color` 控制（与 list/show 同一着色层）

### 4. CLI 接线

- `cli.rs`：新增 `Tree { format: OutputFormat, color: ColorMode }`
  子命令；`--color` 提升为全局参数（现有 list/show 的 format 参数不变）
- `main.rs`：`run_tree` 调用 scan + tree 渲染

## 错误处理

- 桥窗口读取失败：桥降级为普通节点，不中断 tree 输出
- 着色永不失败：ColorMode::enabled 判定只依赖 stdout TTY 检测

## 验证

1. **myece**：
   - `list` / `show` / `tree` 终端运行有颜色
   - 管道重定向（`| head`）自动无颜色
   - `--color always | head` 强制保留颜色码
   - `--color never` 终端内无颜色
   - JSON 输出无任何 ANSI 码
2. **dev48**：`tree` 输出与 `sudo lspci -t` 拓扑结构对照
3. **回归**：现有 text 输出除颜色码外内容逐行不变

## 全局约束

- 不引入新依赖；不改变 list/show 的文本内容语义。
- 分支策略：从 main 切 `sdd/cli-enhancement`，完成后走
  finishing-a-development-branch。
