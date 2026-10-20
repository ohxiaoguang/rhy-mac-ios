// Modified for rhy-mac-ios on 2026-10-20; see NOTICE and docs/PROVENANCE.md.
import AppKit
import SwiftUI
import Combine

private let workspace = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Rhythm")
private let game = workspace.appendingPathComponent("work/game")
private let logURL = workspace.appendingPathComponent("rhythm.log")

final class LauncherModel: ObservableObject {
    @Published var folder = UserDefaults.standard.string(forKey: "gameFolder") ?? ""
    @Published var handheld = UserDefaults.standard.object(forKey: "handheld") as? Bool ?? true {
        didSet { UserDefaults.standard.set(handheld, forKey: "handheld") }
    }
    @Published var showFPS = UserDefaults.standard.bool(forKey: "showFPS") {
        didSet { UserDefaults.standard.set(showFPS, forKey: "showFPS") }
    }
    @Published var busy = false
    @Published var ready = false
    @Published var status = "请选择包含 XCI、prod.keys 和 Firmware 的目录。"
    private var process: Process?
    private func tool(_ name: String) -> URL { Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/\(name)") }
    func choose() {
        let panel=NSOpenPanel();panel.canChooseDirectories=true;panel.canChooseFiles=false
        if panel.runModal() == .OK, let url=panel.url { folder=url.path;ready=false;UserDefaults.standard.set(folder,forKey:"gameFolder") }
    }
    private func files() throws -> (URL,URL,URL) {
        let root=URL(fileURLWithPath:folder)
        let images=try FileManager.default.contentsOfDirectory(at:root,includingPropertiesForKeys:nil).filter { $0.pathExtension.lowercased()=="xci" }
        guard images.count==1 else { throw NSError(domain:"Rhythm",code:1,userInfo:[NSLocalizedDescriptionKey:"目录中需要且仅能有一个 XCI 文件"] ) }
        let keys=root.appendingPathComponent("prod.keys"),firmware=root.appendingPathComponent("Firmware")
        guard FileManager.default.fileExists(atPath:keys.path),FileManager.default.fileExists(atPath:firmware.path) else {
            throw NSError(domain:"Rhythm",code:2,userInfo:[NSLocalizedDescriptionKey:"缺少 prod.keys 或 Firmware"])
        }
        return (images[0],keys,firmware)
    }
    private func execute(_ name:String,_ args:[String],capture:Bool=false) throws -> String {
        let p=Process();p.executableURL=tool(name);p.arguments=args;p.currentDirectoryURL=workspace
        var environment=ProcessInfo.processInfo.environment
        environment["NX_PHONE_PADS"]="0";environment["NX_SHADER_CACHE"]=workspace.appendingPathComponent("work/shaders-macosx").path
        p.environment=environment
        let pipe=Pipe()
        let log=try FileHandle(forWritingTo:logURL);try log.seekToEnd()
        defer { try? log.close() }
        p.standardOutput=capture ? pipe : log;p.standardError=log
        try p.run()
        let data=capture ? pipe.fileHandleForReading.readDataToEndOfFile() : Data()
        p.waitUntilExit()
        guard p.terminationStatus==0 else { throw NSError(domain:"Rhythm",code:Int(p.terminationStatus),userInfo:[NSLocalizedDescriptionKey:"\(name) 执行失败，请查看日志"] ) }
        return String(decoding:data,as:UTF8.self)
    }
    func prepare() {
        busy=true;ready=false;status="正在核实镜像并准备程序…"
        DispatchQueue.global(qos:.userInitiated).async { [self] in
            do {
                try FileManager.default.createDirectory(at:workspace,withIntermediateDirectories:true)
                if !FileManager.default.fileExists(atPath:logURL.path) { FileManager.default.createFile(atPath:logURL.path,contents:nil) }
                let shaderCache=workspace.appendingPathComponent("work/shaders-macosx")
                try FileManager.default.createDirectory(at:shaderCache,withIntermediateDirectories:true)
                if let bundled=Bundle.main.resourceURL?.appendingPathComponent("shaders-macosx") {
                    for source in try FileManager.default.contentsOfDirectory(at:bundled,includingPropertiesForKeys:nil) {
                        let destination=shaderCache.appendingPathComponent(source.lastPathComponent)
                        if FileManager.default.fileExists(atPath:destination.path) { try FileManager.default.removeItem(at:destination) }
                        try FileManager.default.copyItem(at:source,to:destination)
                    }
                }
                let (base,keys,_)=try files()
                let info=try execute("nxfs",["--keys",keys.path,"info",base.path],capture:true)
                guard info.contains("Program program=0100d9f01d474000") else { throw NSError(domain:"Rhythm",code:3,userInfo:[NSLocalizedDescriptionKey:"镜像标题 ID 不符"] ) }
                _=try execute("nxfs",["--keys",keys.path,"extract","--base",base.path,"--out",game.path])
                let exefs=game.appendingPathComponent("exefs")
                let modules=try FileManager.default.contentsOfDirectory(at:exefs,includingPropertiesForKeys:nil).filter {
                    guard $0.lastPathComponent != "rtld",let handle=try? FileHandle(forReadingFrom:$0) else {return false}
                    defer {try? handle.close()};return (try? handle.read(upToCount:4))==Data("NSO0".utf8)
                }.sorted { $0.lastPathComponent < $1.lastPathComponent }
                _=try execute("nxpatch",["rewrite","--out",game.appendingPathComponent("patched").path]+modules.map(\.path))
                DispatchQueue.main.async { [self] in self.busy=false;self.ready=true;self.status="游戏已准备，可以启动。"}
            } catch {DispatchQueue.main.async { [self] in self.busy=false;self.status=error.localizedDescription}}
        }
    }
    func launch() {
        do {
            let (base,keys,firmware)=try files()
            let p=Process();p.executableURL=tool("nxhost");p.currentDirectoryURL=workspace
            p.arguments=["--base",base.path,"--keys",keys.path,"--firmware",firmware.path]
            var environment=ProcessInfo.processInfo.environment;environment["NX_PHONE_PADS"]="0"
            environment["NX_HANDHELD"] = handheld ? "1" : "0"
            environment["NX_SHOW_FPS"] = showFPS ? "1" : "0"
            environment["NX_SHADER_CACHE"]=workspace.appendingPathComponent("work/shaders-macosx").path;p.environment=environment
            let log=try FileHandle(forWritingTo:logURL);try log.seekToEnd();p.standardOutput=log;p.standardError=log
            p.terminationHandler={ [weak self] p in try? log.close();DispatchQueue.main.async {self?.busy=false;self?.status="游戏进程已结束（\(p.terminationStatus)）。日志保存在本机。"}}
            try p.run();process=p;busy=true;status="游戏进程运行中。"
        } catch {status=error.localizedDescription}
    }
}
@main struct RhythmApp: App {
    @StateObject private var model=LauncherModel()
    var body: some Scene {
        WindowGroup {
            VStack(alignment:.leading,spacing:20) {
                HStack {Image(systemName:"waveform.circle.fill").font(.system(size:56)).foregroundStyle(.orange);VStack(alignment:.leading){Text("Rhythm").font(.largeTitle.bold());Text("Rhythm Heaven Groove · macOS").foregroundStyle(.secondary)}}
                Text("选择游戏文件目录，准备后启动。X / Return 确认，Z / Backspace 返回；也支持游戏手柄。")
                Text(model.folder).font(.system(.callout,design:.monospaced)).textSelection(.enabled)
                HStack(spacing: 24) {
                    Picker("启动模式", selection: $model.handheld) {
                        Text("掌机模式").tag(true)
                        Text("底座模式").tag(false)
                    }.pickerStyle(.segmented)
                    Toggle("显示 FPS", isOn: $model.showFPS).toggleStyle(.checkbox)
                }.disabled(model.busy)
                HStack {Button("选择游戏目录",action:model.choose);Button("准备游戏",action:model.prepare);Button("启动游戏",action:model.launch).buttonStyle(.borderedProminent).disabled(!model.ready)}.disabled(model.busy)
                if model.busy {ProgressView()}
                Text(model.status).foregroundStyle(.secondary).textSelection(.enabled)
                HStack {Button("查看日志"){NSWorkspace.shared.open(logURL)};Button("打开存档目录"){try? FileManager.default.createDirectory(at:workspace.appendingPathComponent("work/save"),withIntermediateDirectories:true);NSWorkspace.shared.open(workspace.appendingPathComponent("work/save"))}}
            }.padding(32).frame(width:660)
        }.windowResizability(.contentSize)
    }
}
