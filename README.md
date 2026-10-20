# rhy-mac-ios

**English** | [简体中文](README.zh-CN.md)

An ARM64 native compatibility-layer research project for Apple Silicon macOS and iOS/iPadOS, currently targeting **Rhythm Heaven Groove 1.0.0**. It is based on [Mush8](https://github.com/c22dev/mush8), follows the two-platform organization of [D2R Mobile](https://github.com/jhaizhou-ops/d2r-mobile), and draws on the texture layout and audio implementation of [nxnative](https://github.com/seedds/nxnative).

This repository is for sharing techniques around the compatibility layer, offline instruction rewriting and Metal graphics adaptation. **Only source code and build tooling are published — no games, keys, firmware, game-derived code or ready-to-run installers.** Running it requires files you are entitled to use; this project does not provide downloads, extraction steps or acquisition instructions for them.

## How does it work?

The Switch and Apple Silicon both use ARM64. `nxpatch` rewrites, offline, the instructions that cannot execute directly in Apple user space; `nxhost` implements the system services, file, save, input and audio interfaces the game depends on, and translates NVN graphics calls into Metal. Shaders are translated and compiled by local tools.

macOS loads the locally rewritten program at launch. For iOS, the rewritten modules are placed into the read-only executable segment of the Mach-O at build time; no JIT is used on the device to generate game code. As a result, **an IPA built locally contains code derived from your game files and is not a pure source artifact that can be published publicly, even if it carries no original XCI.** This repository does not upload such artifacts.

## Requirements

### Running

| Platform | Current deployment target | Hardware and notes |
| --- | --- | --- |
| macOS | **macOS 14.0 or later** | Apple Silicon (M series); Intel Macs are not supported |
| iPhone | **iOS 26.0 or later** | ARM64 physical device; valid signing required; no standalone iPhone test conclusions yet |
| iPad | **iPadOS 26.0 or later** | ARM64 physical device; runtime checked on an iPad Air (M2) and manually accepted by the user |

The versions above are the minimums in the build configuration and do not mean that every device, minor OS release and stage has been verified. A device with at least 8 GB of memory is recommended; that is a capacity recommendation, not a measured minimum memory requirement. The iOS Simulator is not supported.

Validated environments: Apple M1 / macOS 15.8; iPad Air (M2) / iPadOS 27.0.1. See the [validation record](docs/VALIDATION.md) (in Chinese) for the detailed scope.

### Building from source

- An Apple Silicon Mac and a full Xcode; **Xcode 26.3** has been verified. Xcode 26.3 requires the build host to run **a supported version of macOS 15.6 or later**, which differs from the app's own macOS 14 deployment target; for other Xcode versions check [Apple's system requirements](https://developer.apple.com/xcode/system-requirements/).
- Rust / Cargo; macOS builds are verified with Rust 1.99.1, and the iOS scripts are pinned to Rust 1.98.1.
- Python 3, CMake, XcodeGen, and Xcode's Metal compiler tools.
- iOS builds need the `aarch64-apple-ios` target; installing on a device requires your own development signing or another supported signing flow.

Example dependency setup (skip anything you already have):

```sh
brew install cmake xcodegen
rustup toolchain install stable 1.98.1
rustup target add aarch64-apple-ios --toolchain 1.98.1
cargo fetch --locked
```

Install Rust through [rustup](https://rustup.rs/). The build scripts use Cargo in offline mode, so dependencies must be fetched before the first build. If Xcode reports missing Metal tools, install the corresponding components through Xcode.

## Local files

Only the build matching title ID `0100D9F01D474000`, version `1.0.0`, is supported. The tools also verify the module build identifiers; other regions or newer versions are not compatible merely because the file name matches.

Prepare the following in your own game folder:

```text
your-game-folder/
  your-game.xci    # exactly one XCI in the folder, any file name
  prod.keys
  Firmware/
```

Only use files you are lawfully entitled to hold, process and use. Owning a game does not automatically grant every right to copy, modify or redistribute it; the applicable rules depend on the law and agreements that apply. Do not upload these files to this repository's issues, pull requests, attachments or releases.

## Build

Clone the repository and point it at your own file folder:

```sh
git clone https://github.com/ohxiaoguang/rhy-mac-ios.git
cd rhy-mac-ios
./build.sh configure /path/to/your-game-folder
./build.sh prepare
```

`configure` creates the local resource links, and `prepare` extracts the modules from the local image and rewrites them offline. It does not download the game and does not modify the original files.

### macOS

```sh
./build.sh shaders macosx
./build.sh macos
```

The output is `build/Rhythm.app`. Open it, select your own file folder, click "Prepare Game", and then "Launch Game" once it finishes. The first preparation and shader compilation take a while.

The launcher's workspace, saves and logs live in `~/Library/Application Support/Rhythm/`. If you start the host directly from the command line, the workspace is `work/` in the current directory.

### iOS / iPadOS

```sh
./build.sh shaders iphoneos
./build.sh ios
```

The output is a local `build/Rhythm.ipa`, ad-hoc signed by default, so you must re-sign it yourself before installing. You can also build with your own development team:

```sh
TEAM=YOUR_TEAM_ID BUNDLE_ID=your.bundle.identifier ./build.sh ios --signed
# when a new device has to be added to the development provisioning profile:
TEAM=YOUR_TEAM_ID BUNDLE_ID=your.bundle.identifier \
  ./build.sh ios --signed --device YOUR_DEVICE_UDID
```

A signed build with `--device` lets Xcode register the given device and update the provisioning profile; the build script itself does not install the app.

After installing, import your own XCI, `prod.keys` and `Firmware` folder in the app, or copy them into the app's Documents through Finder file sharing. An imported XCI is stored as `base.xci`. The game version on the device must match the one used at build time.

Logs are in Files → Rhythm → `work/nxhost.log`, and saves are in `work/save/`. The validity period, device count and capability limits of development signing are governed by Apple's account and signing policies.

## Controls

GameController pads are supported; on iOS, on-screen touch buttons appear when no controller is connected. The left pad has four directional buttons and supports simultaneous input with the right-side buttons. Mac keyboard mapping:

| Key | Game input |
| --- | --- |
| X / Return | A / confirm |
| Z / Backspace | B / back |
| C / V | X / Y |
| Arrow keys | D-pad |
| WASD | Left stick |
| Q / E | L / R |
| 1 / 3 | ZL / ZR |
| Tab / = | + / pause |
| - | −, skips practice according to the game's rules during practice |

Both launchers offer a **Handheld / Docked** mode selector and a **Show FPS** toggle. Preferences are saved for the next launch. FPS appears in the top-left corner of the game window and updates once a second; it is off by default.

Handheld mode reports 1280×720; docked mode reports 1920×1080 and may require the game's TV latency calibration. Changes take effect at the next game launch. For command-line use, `NX_HANDHELD=0` selects docked mode and `NX_SHOW_FPS=1` enables the FPS overlay.

## Status and limitations

0.3.0 adds saved launcher preferences for handheld/docked mode and the FPS overlay, plus four directional touch buttons in place of the iOS virtual stick. Mac launcher persistence and the FPS overlay have been checked in a running game; the updated iOS controls still need device testing. See [validation scope](docs/VALIDATION.md).

0.2.0 fixed the observed atlas misalignment, leftover text, muted audio, runaway frame rate and save-append issues. On Mac, the first single-player stage has been checked from practice through the actual game, the results screen and the return, at roughly 60 fps; on iPad, launch plus video and audio output have been checked, and the user confirmed basic playability by hand.

This is not a full compatibility promise: all stages, multiplayer, different controllers, precise rhythm input latency, background resume and long-session memory stability have not been fully verified. Synchronized screenshots and the first load disturb frame timing and audio buffering, so debug recordings should not be treated as end-to-end audio/video latency measurements.

This project targets one identified build. It is not a general Switch emulator and makes no promise that upstream Mush8's networking or other special features work here.

## Development and layout

```sh
./build.sh test
# requires your own local game modules:
./target/release/nxpatch verify work/game/exefs/main work/game/exefs/sdk
python3 tools/smoke.py --seconds 25
```

A process that keeps running only proves it started; judge playability together with what you see, hear and control. Diagnostic switches and local replay are described in the [development notes](docs/DEVELOPMENT.md) (in Chinese).

| Directory | Contents |
| --- | --- |
| `crates/nxabi` | Conventions shared by the host and the rewritten code |
| `crates/nxfs` | Container, file system and module parsing |
| `crates/nxpatch` | Offline ARM64 rewriting and equivalence verification |
| `crates/nxscan` / `crates/nxshader` | Instruction scanning, shader translation tools |
| `crates/nxhost` | System services, audio, input, NVN / Metal compatibility layer |
| `launcher` / `ios` | Native launcher and iOS project configuration |
| `tools` | Local preparation, build, verification and source release checks |
| `third_party` | Third-party license notices that must ship with the source |

`target/`, `work/` and `build/` are all local generated content and never enter Git. `target/` can be deleted and will be regenerated on the next build; keep `work/save/` to avoid losing command-line saves.

## License and notices

This project is released under **GPL-3.0**, inheriting Mush8's license and original author notices. The nxnative-derived portion additionally retains the MIT license; see [LICENSE](LICENSE), [NOTICE](NOTICE) and the [provenance notes](docs/PROVENANCE.md). Dependencies remain under their own licenses.

This project is independent technical research and has no affiliation, authorization, sponsorship or endorsement relationship with Nintendo, Apple or the rights holders of the game. Names and trademarks are used only to identify compatibility. The source license does not cover the game program, images, sounds, fonts, firmware, keys or other third-party content.

"Technical sharing" or a disclaimer does not by itself remove copyright, anti-circumvention, contractual or trademark liability. Follow applicable law, agreements and third-party licenses; the software is provided "as is" under its licenses, without warranties of playability, non-infringement or fitness for a particular purpose. See the [publishing notes](docs/PUBLISHING.md) for the scope of the source release.
