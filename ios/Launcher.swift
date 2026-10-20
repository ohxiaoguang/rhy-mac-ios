// Modified for rhy-mac-ios on 2026-10-20; see NOTICE and docs/PROVENANCE.md.
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import AVFAudio

@_cdecl("rhythm_audio_session")
public func rhythmAudioSession() {
    do {
        let session = AVAudioSession.sharedInstance()
        try session.setCategory(.playback, mode: .default)
        try session.setPreferredSampleRate(48_000)
        try session.setPreferredIOBufferDuration(0.005)
        try session.setActive(true)
    } catch { NSLog("[audio] session setup failed: %@", error.localizedDescription) }
}

private let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
private struct RhythmLauncher: View {
    let play: @convention(c) () -> Void
    @State private var importing = false
    @State private var importingFolder = false
    @State private var copying = false
    @State private var message = ""
    @State private var ready = false
    private func scan() {
        ready = ["base.xci", "prod.keys", "Firmware"].allSatisfy {
            FileManager.default.fileExists(atPath: documents.appendingPathComponent($0).path)
        }
    }
    private func startGame() {
        rhythmAudioSession()
        launcher?.willMove(toParent: nil)
        launcher?.view.removeFromSuperview()
        launcher?.removeFromParent()
        launcher = nil
        play()
    }
    var body: some View {
        HStack(spacing: 40) {
            VStack(alignment: .leading, spacing: 12) {
                Image(systemName: "waveform.circle.fill").font(.system(size: 72)).foregroundStyle(.orange)
                Text("Rhythm").font(.largeTitle.bold())
                Text("Rhythm Heaven Groove · 原生兼容层").foregroundStyle(.secondary)
                Text("技术研究版本：兼容性验证范围见项目文档。\n需要与构建时一致的 1.0.0 游戏镜像。").font(.callout)
            }
            VStack(alignment: .leading, spacing: 16) {
                Text("游戏文件").font(.title2.bold())
                Text("导入 XCI 和 prod.keys；Firmware 文件夹也可通过 Finder 文件共享复制。资源与存档保存在本机 Documents 中。")
                Button("导入 XCI / prod.keys") { importing = true }.disabled(copying)
                Button("导入 Firmware 文件夹") { importingFolder = true }.disabled(copying)
                if copying { ProgressView("正在复制，请保持应用打开…") }
                Text(message).font(.caption).textSelection(.enabled)
                Button("启动游戏", action: startGame).buttonStyle(.borderedProminent).disabled(!ready || copying)
                Text("日志：Files → Rhythm → work → nxhost.log").font(.caption).foregroundStyle(.secondary)
            }.frame(maxWidth: 440)
        }
        .padding(32).frame(maxWidth: .infinity, maxHeight: .infinity)
        .onAppear {
            scan()
            if ready && ProcessInfo.processInfo.environment["NX_AUTOSTART"] == "1" {
                DispatchQueue.main.async { startGame() }
            }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.data], allowsMultipleSelection: true) { receive($0) }
        .fileImporter(isPresented: $importingFolder, allowedContentTypes: [.folder], allowsMultipleSelection: false) { receive($0) }
    }
    private func receive(_ result: Result<[URL], Error>) {
        switch result {
        case .failure(let error): message = error.localizedDescription
        case .success(let urls):
            copying = true
            DispatchQueue.global(qos: .userInitiated).async {
                var status = "导入完成"
                do {
                    for source in urls {
                        let scoped = source.startAccessingSecurityScopedResource()
                        defer { if scoped { source.stopAccessingSecurityScopedResource() } }
                        let folder = (try source.resourceValues(forKeys: [.isDirectoryKey])).isDirectory == true
                        let name: String
                        if folder && source.lastPathComponent == "Firmware" { name = "Firmware" }
                        else if source.pathExtension.lowercased() == "xci" { name = "base.xci" }
                        else if source.lastPathComponent == "prod.keys" { name = "prod.keys" }
                        else { throw CocoaError(.fileReadUnsupportedScheme) }
                        let dest = documents.appendingPathComponent(name)
                        if source.standardizedFileURL == dest.standardizedFileURL { continue }
                        let stage = documents.appendingPathComponent(".import-\(UUID().uuidString)")
                        defer { try? FileManager.default.removeItem(at: stage) }
                        try FileManager.default.copyItem(at: source, to: stage)
                        if FileManager.default.fileExists(atPath: dest.path) {
                            _ = try FileManager.default.replaceItemAt(dest, withItemAt: stage)
                        } else { try FileManager.default.moveItem(at: stage, to: dest) }
                    }
                } catch { status = error.localizedDescription }
                DispatchQueue.main.async { copying = false; message = status; scan() }
            }
        }
    }
}
private var launcher: UIViewController?
@_cdecl("rhythm_show_launcher")
public func rhythmShowLauncher(_ pointer: UnsafeMutableRawPointer, _ play: @convention(c) () -> Void, _ controller: @convention(c) () -> Void) {
    let parent = Unmanaged<UIViewController>.fromOpaque(pointer).takeUnretainedValue()
    let host = UIHostingController(rootView: RhythmLauncher(play: play))
    launcher = host
    parent.addChild(host); host.view.frame = parent.view.bounds
    host.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    parent.view.addSubview(host.view); host.didMove(toParent: parent)
}
