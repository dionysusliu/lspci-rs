# Capability Protocol Decoders Progress

更新时间：2026-08-10

## 当前工作区

- 真实代码源：ECS 容器 `95c90e05ab1a` 的 `/workspace`
- ECS 分支：`sdd/capability-decoders`（基于 `main`）
- 设计规范：`docs/superpowers/specs/2026-08-10-capability-decoders-design.md`
- 实现计划：`docs/superpowers/plans/2026-08-10-capability-decoders.md`

## 已提交实现

| 阶段 | ECS commit | 内容 |
| --- | --- | --- |
| Task 1 | `a83b8bc` | `PciCapabilityContent` 类型、`content` 字段、`ConfigSpaceSnapshot::read` 纯读取 |
| Task 2 | `94195e0` | PM / MSI / MSI-X / Vendor Specific decoder |
| Task 3 | `fd64b56` | PCIe decoder |
| Task 4 | `ba0645e` | `decode_content` 分发 + `inspect()` 接线 |
| Task 5 | `a1127d9` | text/JSON content 渲染 |
| 修复 | `1553068` | 解码前预取 capability payload（64 字节 + vendor 长度追加） |

## 计划外修复

真机验证发现：walker 只读取每个 capability 的 2 字节头，snapshot 缺少
payload 字节，导致 decoder 全部返回 None。修复：`inspect()` 在解码前对每个
Valid 节点预取 `[offset, offset+0x40)`，vendor specific 按长度字节追加预取。
该修复未在原 spec/plan 中预见，属于实现期发现的设计缺口。

## dev48 真机验证结果（2026-08-10）

验证设备与对照（`sudo lspci -vv`）：

| 设备 | capability | 结果 |
| --- | --- | --- |
| 0000:00:03.0 virtio console | MSI-X | ✅ Enable+ Count=2 Masked- table=BAR1+0x0 pba=BAR1+0x800 全部一致 |
| 0000:00:04.0 QEMU NVMe | MSI-X / PCIe / Vendor Specific | ✅ Count=5、table BAR0+0x2000、PBA BAR0+0x2c00；Express v2 Endpoint、LnkCap 16GT/s x16、LnkSta 8GT/s x16（downgraded）；Vendor Len=64 原始字节一致 |
| 0000:00:1f.0 PCI-PCI bridge | MSI | ✅ Enable- Count=1/1 Maskable+ 64bit+ Address/Data 一致 |

**PM 无法验证**：dev48 上无任何 Power Management capability 设备
（`lspci -vv | grep -c "Power Management"` = 0）。PM decoder 仅通过编译，
未做真机对照。

扩展配置空间在 dev48 上同样不可读（extended chain 报 ReadError），
与 myece 一致。

## myece 不回归检查

- `cargo fmt --all -- --check`：通过
- `cargo check --workspace`：通过
- `list`：9/9 设备
- `show --config standard`：输出与上一切片一致
- `git diff --check`：通过

## 二进制分发路径（备忘）

myece 与 dev48 网络不通，二进制经本机中转：

```bash
podman cp 95c90e05ab1a:/workspace/target/x86_64-unknown-linux-gnu/debug/lspci-rs /tmp/lspci-rs
sftp myece <<< "get /tmp/lspci-rs <本地路径>"
sftp dev48 <<< "put <本地路径> /tmp/lspci-rs"
```

容器与 dev48 同为 Alibaba Cloud Linux 3，debug 二进制直接可运行。
注意：scp 在本会话权限环境下会被 kill，改用 sftp。

## 下一步

1. whole-branch review，决定是否合并回 `main`。
2. PM decoder 待找到有 PM 的设备后补充验证。
3. 后续切片可选：扩展 capability decoder（AER/SR-IOV）、header 字段解读。
