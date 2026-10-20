# rhy-mac-ios

[English](README.md) | **简体中文**

一个面向 Apple Silicon macOS 和 iOS/iPadOS 的 ARM64 原生兼容层研究项目，当前适配 **Rhythm Heaven Groove 1.0.0**。基于 [Mush8](https://github.com/c22dev/mush8)，参考 [D2R Mobile](https://github.com/jhaizhou-ops/d2r-mobile) 的双端组织方式，以及 [nxnative](https://github.com/seedds/nxnative) 的纹理布局和音频实现。

本仓库用于分享兼容层、离线指令改写和 Metal 图形适配的技术。**仅发布源码和构建工具，不提供游戏、密钥、固件、游戏派生代码或可直接运行的安装包。** 运行需要自行准备有权使用的文件；本项目不提供这些文件的下载、提取或获取教程。

## 它是如何工作的？

Switch 和 Apple Silicon 都使用 ARM64。`nxpatch` 离线改写不能直接在 Apple 用户态执行的指令；`nxhost` 实现游戏依赖的系统服务、文件、存档、输入和音频接口，并把 NVN 图形调用转换为 Metal。着色器由本地工具翻译和编译。

macOS 在启动时装载本地改写后的程序。iOS 版本在构建时将改写模块放入 Mach-O 的只读可执行段，不在设备上使用 JIT 生成游戏代码。因此，**本地生成的 IPA 含有来自用户游戏文件的代码，即使没有携带原始 XCI，也不属于可直接公开发布的纯源码产物。** 本仓库不上传这些产物。

## 系统要求

### 运行

| 平台 | 当前部署目标 | 硬件及说明 |
| --- | --- | --- |
| macOS | **macOS 14.0 或更高** | Apple Silicon（M 系列）；不支持 Intel Mac |
| iPhone | **iOS 26.0 或更高** | ARM64 真机；需要有效签名，尚无独立 iPhone 测试结论 |
| iPad | **iPadOS 26.0 或更高** | ARM64 真机；已在 iPad Air（M2）做运行检查并由使用者手动验收 |

以上是构建配置中的最低版本，不表示所有设备、系统小版本和关卡均已验证。建议使用至少 8 GB 内存的设备；这是容量建议，不是实测的最低内存要求。当前不支持 iOS 模拟器。

已验证环境：Apple M1 / macOS 15.8；iPad Air（M2）/ iPadOS 27.0.1。详细范围见 [验证记录](docs/VALIDATION.md)。

### 从源码构建

- Apple Silicon Mac 和完整 Xcode，已验证 **Xcode 26.3**。Xcode 26.3 要求构建主机运行 **macOS 15.6 或更高的受支持版本**，与应用本身的 macOS 14 部署目标不同；其他 Xcode 版本请核对 [Apple 的系统要求](https://developer.apple.com/xcode/system-requirements/)。
- Rust / Cargo；Mac 构建验证版本为 Rust 1.99.1，iOS 脚本固定使用 Rust 1.98.1。
- Python 3、CMake、XcodeGen，以及 Xcode 的 Metal 编译工具。
- iOS 构建需要 `aarch64-apple-ios` 目标；安装真机需要自己的开发签名或其他受支持的签名流程。

依赖准备示例（已有的工具无需重复安装）：

```sh
brew install cmake xcodegen
rustup toolchain install stable 1.98.1
rustup target add aarch64-apple-ios --toolchain 1.98.1
cargo fetch --locked
```

Rust 请通过 [rustup](https://rustup.rs/) 安装。构建脚本使用离线 Cargo 模式，首次构建前需要完成依赖下载。若 Xcode 提示缺少 Metal 工具，请通过 Xcode 安装对应组件。

## 本地文件

当前仅支持标题 ID `0100D9F01D474000`、版本 `1.0.0` 的对应构建。工具还会校验模块构建标识；其他地区或更新版本不能仅凭文件名相同就认定兼容。

在自己的游戏目录中准备：

```text
your-game-folder/
  your-game.xci    # 目录中仅有一个 XCI，文件名不限
  prod.keys
  Firmware/
```

仅使用你依法有权持有、处理和使用的文件。购买游戏不当然意味着获得所有复制、改写或分发权限，相关规则取决于适用法律和协议。不要向本仓库的 Issue、PR、附件或 Release 上传这些文件。

## 构建

先克隆并配置自己的文件目录：

```sh
git clone https://github.com/ohxiaoguang/rhy-mac-ios.git
cd rhy-mac-ios
./build.sh configure /path/to/your-game-folder
./build.sh prepare
```

`configure` 建立本地资源链接，`prepare` 从本地镜像提取模块并离线改写；不下载游戏，也不改写原始文件。

### macOS

```sh
./build.sh shaders macosx
./build.sh macos
```

输出为 `build/Rhythm.app`。打开后选择自己的文件目录，点击“准备游戏”，完成后“启动游戏”。首次准备和着色器编译需要一些时间。

启动器的工作区、存档和日志位于 `~/Library/Application Support/Rhythm/`。命令行直接启动宿主时，工作区是当前目录下的 `work/`。

### iOS / iPadOS

```sh
./build.sh shaders iphoneos
./build.sh ios
```

输出为本地 `build/Rhythm.ipa`，默认仅做临时签名，需要自行重签才能安装。也可以使用自己的开发团队签名构建：

```sh
TEAM=YOUR_TEAM_ID BUNDLE_ID=your.bundle.identifier ./build.sh ios --signed
# 新设备需要出现在开发描述文件中时：
TEAM=YOUR_TEAM_ID BUNDLE_ID=your.bundle.identifier \
  ./build.sh ios --signed --device YOUR_DEVICE_UDID
```

带 `--device` 的签名构建会允许 Xcode 注册指定设备并更新描述文件；构建脚本本身不会自动安装应用。

安装后在 App 中导入自己的 XCI、`prod.keys` 和 `Firmware` 文件夹，也可通过 Finder 文件共享复制到应用 Documents。XCI 导入后保存为 `base.xci`。设备上需要与构建时一致的游戏版本。

日志位于 Files → Rhythm → `work/nxhost.log`，存档位于 `work/save/`。开发签名的期限、设备数量和能力限制由 Apple 的账户与签名策略决定。

## 操作

支持 GameController 手柄；iOS 在没有连接手柄时显示触屏按键，左侧为上下左右方向按键，可与右侧按键同时操作。Mac 键盘映射如下：

| 按键 | 游戏输入 |
| --- | --- |
| X / Return | A / 确认 |
| Z / Backspace | B / 返回 |
| C / V | X / Y |
| 方向键 | 十字键 |
| WASD | 左摇杆 |
| Q / E | L / R |
| 1 / 3 | ZL / ZR |
| Tab / = | + / 暂停 |
| - | −，练习中可按游戏规则跳过练习 |

两端启动页均可选择“掌机模式”或“底座模式”，并勾选“显示 FPS”；选择自动保存，下次启动沿用。FPS 在游戏画面左上角每秒更新，默认关闭。

默认掌机模式、1280×720；底座模式向游戏报告 1920×1080，可能需要完成游戏的电视延迟校准。设置在启动时生效，更改后需要重启游戏。命令行可用 `NX_HANDHELD=0` 选择底座模式，`NX_SHOW_FPS=1` 显示 FPS。

## 当前状态与限制

0.3.0 新增启动页掌机／底座模式和 FPS 显示设置，自动保存偏好；iOS 左侧改为四个方向按键。Mac 已验证启动偏好持久化及实际游戏 FPS 显示，新版 iOS 触控布局仍待真机复测，详见[验证记录](docs/VALIDATION.md)。

0.2.0 已修复观察到的图集错位、文字残留、静音、帧率失控和存档追加问题。Mac 已检查首个单人关卡从练习到正式游戏、结算和返回的流程，常规约 60 fps；iPad 已完成启动和图像、音频输出检查，使用者手动体验后确认按基本可玩验收通过。

这不是完整兼容性承诺：全部关卡、多人、不同手柄、精确节奏输入延迟、后台恢复和长时间内存稳定性尚未全面验证。同步截图及首次加载会干扰帧时间和音频缓冲，不应把调试录音直接当作端到端音画延迟测量。

本项目针对单个已识别构建，不是通用 Switch 模拟器，也没有承诺上游 Mush8 的联网或其他专用功能在这里可用。

## 开发与目录

```sh
./build.sh test
# 需要自己的本地游戏模块：
./target/release/nxpatch verify work/game/exefs/main work/game/exefs/sdk
python3 tools/smoke.py --seconds 25
```

进程持续运行只代表启动检查，需要结合画面、声音和操作结果判断可玩性。诊断开关和本地回放方式见 [开发说明](docs/DEVELOPMENT.md)。

| 目录 | 内容 |
| --- | --- |
| `crates/nxabi` | 宿主与改写代码共享约定 |
| `crates/nxfs` | 容器、文件系统和模块解析 |
| `crates/nxpatch` | 离线 ARM64 改写与等价验证 |
| `crates/nxscan` / `crates/nxshader` | 指令扫描、着色器翻译工具 |
| `crates/nxhost` | 系统服务、音频、输入、NVN / Metal 兼容层 |
| `launcher` / `ios` | 原生启动器和 iOS 工程配置 |
| `tools` | 本地准备、构建、验证及源码发布检查 |
| `third_party` | 需要随源码保留的第三方许可声明 |

`target/`、`work/` 和 `build/` 都是本地生成内容，不进入 Git。`target/` 可以删除，下次构建会重新生成；保留 `work/save/`，避免丢失命令行运行的存档。

## 许可与声明

本项目以 **GPL-3.0** 发布，继承 Mush8 的许可证和原作者声明。nxnative 改编部分另保留 MIT 许可；详见 [LICENSE](LICENSE)、[NOTICE](NOTICE) 和 [来源说明](docs/PROVENANCE.md)。依赖项继续适用各自许可证。

本项目是独立的技术研究，与 Nintendo、Apple 或游戏权利人没有隶属、授权、赞助或认可关系。相关名称和商标仅用于识别兼容性。源码许可不涵盖游戏程序、图像、声音、字体、固件、密钥或其他第三方内容。

“技术分享”或免责声明不会自动消除版权、反规避、合同或商标方面的责任。请遵守适用法律、协议和第三方许可；软件按原许可证以“现状”提供，不作可玩性、无侵权或特定用途担保。源码发布范围见 [发布说明](docs/PUBLISHING.md)。
