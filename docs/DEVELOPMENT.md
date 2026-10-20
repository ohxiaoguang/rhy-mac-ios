# 开发与诊断

先按 README 准备自己的本地资源。`./build.sh --help` 列出构建命令；`prepare`、平台构建和着色器命令会按需编译本地工具，不自动下载游戏或依赖。

## 本地运行

```sh
NX_PHONE_PADS=0 NX_SHADER_CACHE=work/shaders-macosx ./target/release/nxhost
```

以下诊断入口只用于自己启动的进程：

| 环境变量 | 用途 |
| --- | --- |
| `NX_FPS=1` | 每秒输出帧率 |
| `NX_SHOW_FPS=1` | 在游戏画面左上角显示每秒更新的 FPS；两端启动页可直接勾选 |
| `NX_AUTOINPUT='4:A,7:A,10:A'` | 按相对秒数产生内部手柄短按，不是物理触屏测试 |
| `NX_INPUT_FILE=work/pad.txt` | 轮询一次性短按文件，内容如 `A`、`DOWN`、`PLUS` |
| `NX_DUMP_AT='3,10,20'` | 同步转存呈现方向的画面，影响帧时间和缓冲 |
| `NX_DUMP_AUDIO=work/audio/test.wav` | 记录交给输出设备的 PCM；先建立父目录 |
| `NX_HANDHELD=0` | 启用电视模式 |
| `NX_CHECK_LAYOUT=1` | 对照当前受支持 SDK 的纹理布局计算 |
| `NX_AUTOSTART=1` | iOS 资源齐全时从启动器直接开始游戏 |

```sh
mkdir -p work/audio
NX_FPS=1 NX_DUMP_AUDIO=work/audio/test.wav python3 tools/smoke.py --seconds 30
python3 tools/inspect_audio.py work/audio/test.wav
```

`smoke.py` 到时会终止自己启动的进程；写入 `work/stop-smoke` 也可以结束它。录音正在写入时不要直接作为稳定文件复制或分析。进程未崩溃、PCM 非静音或指令等价检查通过，都不能单独代表可玩性通过。

## 真机 UI 测试

`ios/DeviceTests/` 是独立的 UIKit 触控与截图驱动；配置为 `ios/device-tests.yml`。普通构建只使用 `ios/project.yml`，不安装测试驱动。

已取得设备测试授权、签名与安装名额后，可以自行生成测试工程：

```sh
xcodegen generate --spec ios/device-tests.yml
xcodebuild -project ios/Rhythm.xcodeproj -scheme RhythmDeviceTests \
  -destination 'platform=iOS,id=YOUR_DEVICE_UDID' \
  -derivedDataPath target/ios-xcode DEVELOPMENT_TEAM=YOUR_TEAM_ID test
```

测试前，应用容器内需要自己的完整资源。测试会安装额外的测试驱动、启动游戏、发出真实 UIKit 触控并保存截图；仍需人工审阅画面与声音。免费签名账户的应用安装名额可能影响运行。不要为腾出名额自动删除其他应用。

## 缓存与本地产物

- `target/`：Cargo、Swift 和 Xcode 编译缓存，可删除，之后构建会重新生成。
- `work/`：提取及改写后的模块、着色器和命令行存档。删除前备份 `work/save/`。
- `build/`：本地 `.app` / IPA、调试记录与安装包清单。
- `docs/local/`：不公开的详细本机记录。

这些目录都不进入 Git。`tools/package_release.py` 仅用于整理本地安装包和校验清单；它的名字不代表生成物可以上传到 GitHub Releases。iOS 包包含游戏派生代码，公开发布仅包含本项目源码。
