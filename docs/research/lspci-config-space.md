# lspci `-x` / `-xxx` / `-xxxx` 配置空间读取调研

## 结论摘要

`lspci` 对配置空间采用的是“初始化少量缓存、按需增量读取、允许部分成功”的策略，而不是每次直接读取 4096 字节。

这对 Rust 版本有三个直接启发：

- 配置空间读取应该是 lazy/on-demand 的；
- 读取结果需要表达 partial success，而不能只有成功/失败两种状态；
- 配置空间缓存应该属于一次 `PciSession`/设备检查生命周期，而不是每个字段读取时重新创建。

## 三种 dump 范围

官方 man page 对选项的定义是：

- `-x`：标准配置空间的前 64 字节；CardBus bridge 为前 128 字节；
- `-xxx`：完整的标准 PCI 配置空间，即 256 字节；root-only；
- `-xxxx`：扩展 PCI 配置空间，即最多 4096 字节；root-only。

来源：

- [pciutils lspci.man](https://kernel.googlesource.com/pub/scm/utils/pciutils/pciutils/%2B/361f6a15069059c048c5c4e49239f9e5726a4ea9/lspci.man)
- [官方 lspci.c 参数说明](https://kernel.googlesource.com/pub/scm/utils/pciutils/pciutils/%2B/838893724a780554c3e73110d8b06294140ad0e3/lspci.c)

源码中不是分别解析三个选项，而是每遇到一次 `-x` 就执行：

```c
opt_hex++;
```

因此 `opt_hex` 的值决定读取深度：

| `opt_hex` | 行为 |
| ---: | --- |
| 1 或 2 | 输出初始缓存，即 64 字节；CardBus 为 128 字节 |
| 3 | 尝试补齐到 256 字节 |
| 4 或更高 | 在 256 字节基础上尝试补齐到 4096 字节 |

## 设备初始化与初始缓存

`scan_device()` 为每个设备创建一个独立的配置空间缓存：

```c
d->config_cached = d->config_bufsize = 64;
d->config = xmalloc(64);
d->present = xmalloc(64);
memset(d->present, 1, 64);
```

随后读取配置空间起始的 64 字节：

```c
if (!d->no_config_access && !pci_read_block(p, 0, d->config, 64))
{
  d->no_config_access = 1;
  d->config_cached = d->config_bufsize = 0;
  memset(d->present, 0, 64);
}
```

如果是 CardBus bridge，源码再尝试读取后续 64 字节，并把缓存长度提升到 128。

初始化完成后，lspci 调用：

```c
pci_setup_cache(p, d->config, d->config_cached);
```

这会把 lspci 的配置空间缓存交给 libpci，使 libpci 后续解析 capability 或配置寄存器时可以复用已读取的数据。

来源：[lspci.c `scan_device()`](https://kernel.googlesource.com/pub/scm/utils/pciutils/pciutils/%2B/838893724a780554c3e73110d8b06294140ad0e3/lspci.c)

## `config_fetch()` 的增量读取

核心函数是：

```c
int config_fetch(struct device *d, unsigned int pos, unsigned int len)
```

它的流程如下：

1. 检查请求范围是否已经在 `present[]` 中；
2. 跳过已经读取的前缀和后缀；
3. 如果仍有缺失范围，按需扩大 `config` 和 `present` 缓冲区；
4. 使用 `pci_read_block(d->dev, pos, d->config + pos, len)` 读取；
5. 只有整段读取成功时，才把对应的 `present[]` 标记为已读取；
6. 返回 libpci 的成功/失败结果。

如果读取范围超过当前缓冲区，缓冲区会不断扩大，源码使用的目标是容纳 `pos + len`。

来源：[lspci.c `config_fetch()`](https://kernel.googlesource.com/pub/scm/utils/pciutils/pciutils/%2B/838893724a780554c3e73110d8b06294140ad0e3/lspci.c)

## `show_hex_dump()` 的部分成功语义

实际 dump 逻辑是：

```c
cnt = d->config_cached;
if (opt_hex >= 3 && config_fetch(d, cnt, 256-cnt))
{
  cnt = 256;
  if (opt_hex >= 4 && config_fetch(d, 256, 4096-256))
    cnt = 4096;
}
```

这意味着：

- 读取 256 字节失败时，仍然输出已经缓存的 64/128 字节；
- 读取 4096 字节失败时，仍然输出已经成功读取的 256 字节；
- 工具不会因为扩展空间不可读而丢弃前面已经读取成功的数据。

最终按 16 字节一行输出，行首为配置空间 offset：

```text
00: xx xx xx xx xx xx xx xx xx xx xx xx xx xx xx xx
10: xx xx xx xx xx xx xx xx xx xx xx xx xx xx xx xx
```

来源：[lspci.c `show_hex_dump()`](https://kernel.googlesource.com/pub/scm/utils/pciutils/pciutils/%2B/838893724a780554c3e73110d8b06294140ad0e3/lspci.c)

## libpci 与 Linux sysfs 的边界

libpci 的 `linux-sysfs` backend：

1. 打开 `/sys/bus/pci/devices/<BDF>/config`；
