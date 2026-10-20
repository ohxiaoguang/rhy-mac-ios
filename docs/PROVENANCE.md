# 参考项目与适配范围

本项目参考的上游版本：

- [Mush8](https://github.com/c22dev/mush8)：`975040ba341efb99c5631292a0266ae26b28d960`，GPL-3.0。
- [D2R Mobile](https://github.com/jhaizhou-ops/d2r-mobile)：`6996690e4b1a228549aad0fc5cf6378ea7d365d3`，根项目 MIT，第三方目录各有自己的许可。
- [nxnative](https://github.com/seedds/nxnative)：`7e2c741b8d41eaab85838ff466adee327f1baa67`，MIT。阅读其移植指南、图形资源及命令诊断、纹理布局、音频实现；`audio/sdk.rs` 的 CPU/manual 渲染流程、SDK 20 锁和命令列表布局改编自 `dm/host/hle_audren.c`。许可保留在 `third_party/nxnative/LICENSE`。

## Mush8

保留其六个 Rust crate 的源码与 Cargo.lock，原有作者声明和 GPL 许可证保持在项目中。实际阅读了资源提取、离线改写、等价验证、动态装载、iOS 静态段生成、IPC 服务、Metal 渲染与着色器预编译路径，以及两个平台的原始启动器和构建脚本。

该项目原本固定 MK8DX 的三个模块、4.0.1 更新、旧 SDK NVN 地址和若干专用补丁。Rhythm 的构建流程没有复制 MK8DX 数据、着色器表或改写后程序。旧版本兼容代码在上游通用实现中仍可见，但主启动路径严格检查本游戏构建号，不会把其补丁套到当前模块上。

针对本次镜像新增：有界 XCI / HFS0 解析、截尾填充兼容、RELR 与 GNU hash、Clang 字节跳转表分派、实际模块发现、iOS 段及符号表生成、新版 SDK 启动回调、新 TIPC 服务管理器、新 NVN 布局转换、时间共享上下文、用户异常上下文、显示层和系统事件、HID 序列锁、只读存档接口、REV15 效果状态大小、LEA 着色器运算、严格着色器编译状态、命令列表引用生命周期与双端打包工具。

## D2R Mobile

参考其中文总览、宿主与图形/工具文档、宿主启动代码、构建配置和验证分层，采用“本机提取游戏内容、编译时生成 iOS 游戏代码、系统服务和前端分离、保存验证证据”的组织方式。未复制 D2R 的游戏地址、资产、CASC 系统、私有玩法、触屏界面或 shader_map。

LEA 处理还核对了该项目内 Ryujinx shader translator 的 `InstEmitIntegerArithmetic.cs`：取低 32 位的左移/取负/加法语义。这里以现有 Rust / MSL 翻译器的形式实现，带 carry 或条件输出的未覆盖形式会显式报错。

## 本地生成内容

`work/`、`target/`、`build/` 包含由用户本地游戏和工具链产生的程序、着色器、资源、截图与安装包，不作为源码的一部分。prod.keys 仅通过路径交给解密器，不写入源码、Info.plist 或安装包。Firmware 与 XCI 不会被复制到 IPA；iOS 运行时从应用 Documents 读取。
