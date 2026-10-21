import AppKit
import Darwin
import Foundation

struct Browser {
    let id: String
    let name: String
    let bundleID: String
    let dataDirectory: String

    var localState: URL? {
        // Foundation's home-directory APIs return the app container in a sandbox.
        // Resolving the account directory locates the picker; it grants no access.
        guard let directory = getpwuid(getuid())?.pointee.pw_dir else { return nil }
        let home = URL(fileURLWithPath: String(cString: directory), isDirectory: true)
        return home.appendingPathComponent("Library/Application Support")
            .appendingPathComponent(dataDirectory).appendingPathComponent("Local State")
    }
}

struct Profile: Decodable { let id: String; let name: String }
struct ScanResponse: Decodable { let profiles: [Profile]?; let error: String? }

final class BrowserControls {
    let browser: Browser
    let status = NSTextField(labelWithString: "未授权")
    let profilesMenu = NSPopUpButton(frame: .zero, pullsDown: false)
    let openButton = NSButton(title: "验证浏览器打开", target: nil, action: nil)
    var profiles: [Profile] = []
    init(_ browser: Browser) { self.browser = browser }
}

final class Probe: NSObject, NSApplicationDelegate {
    let browsers = [
        Browser(id: "chrome", name: "Google Chrome", bundleID: "com.google.Chrome", dataDirectory: "Google/Chrome"),
        Browser(id: "brave", name: "Brave", bundleID: "com.brave.Browser", dataDirectory: "BraveSoftware/Brave-Browser")
    ]
    var window: NSWindow!
    var controls: [BrowserControls] = []
    let logView = NSTextView()
    var events: [[String: Any]] = []
    var sandboxVerified = false

    func applicationDidFinishLaunching(_ notification: Notification) {
        buildWindow()
        let sentinel = Bundle.main.bundleURL.deletingLastPathComponent().appendingPathComponent("ungranted.txt")
        do {
            _ = try Data(contentsOf: sentinel)
            record("sandbox-control-failed", ["detail": "外部测试文件可读，不能认定沙盒有效"])
        } catch {
            let nsError = error as NSError
            let denied = nsError.code == NSFileReadNoPermissionError
                || (nsError.userInfo[NSUnderlyingErrorKey] as? NSError)?.code == Int(EPERM)
            sandboxVerified = denied
            record(denied ? "sandbox-control-denied" : "sandbox-control-failed",
                   ["detail": error.localizedDescription, "errorCode": nsError.code])
        }
        if sandboxVerified {
            checkBookmarks()
            for row in controls { row.openButton.isEnabled = true }
            record("profile-launch-unsupported", ["detail": "NSWorkspace 在 App Sandbox 中忽略 arguments，不能用此 API 指定 Profile"])
        }
        NSApp.activate(ignoringOtherApps: true)
    }

    func buildWindow() {
        let menu = NSMenu()
        let item = NSMenuItem()
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: "退出验证应用", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        item.submenu = appMenu
        menu.addItem(item)
        NSApp.mainMenu = menu
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 850, height: 650),
                          styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.title = "LinkPilot Sandbox Probe"
        window.center()
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 14
        stack.translatesAutoresizingMaskIntoConstraints = false
        let content = window.contentView!
        content.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 20),
            stack.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -20),
            stack.topAnchor.constraint(equalTo: content.topAnchor, constant: 20),
            stack.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -20)
        ])
        let title = NSTextField(labelWithString: "浏览器 Profile 沙盒验证")
        title.font = .boldSystemFont(ofSize: 21)
        stack.addArrangedSubview(title)
        let explanation = NSTextField(wrappingLabelWithString:
            "只读授权浏览器的 Local State 元数据文件。关闭并重新打开本应用可验证授权是否持久。打开测试通过系统 URL API 发送 example.com 测试页。NSWorkspace 在沙盒中忽略启动参数，因此本路径无法指定 Profile。")
        stack.addArrangedSubview(explanation)
        explanation.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        for (index, browser) in browsers.enumerated() {
            let row = BrowserControls(browser)
            controls.append(row)
            let heading = NSTextField(labelWithString: browser.name)
            heading.font = .boldSystemFont(ofSize: 15)
            stack.addArrangedSubview(heading)
            let actions = NSStackView()
            actions.spacing = 10
            let authorize = NSButton(title: "授权 Local State…", target: self, action: #selector(authorize(_:)))
            authorize.tag = index
            actions.addArrangedSubview(authorize)
            row.profilesMenu.widthAnchor.constraint(equalToConstant: 360).isActive = true
            row.profilesMenu.addItem(withTitle: "未读取 Profile")
            row.profilesMenu.isEnabled = false
            actions.addArrangedSubview(row.profilesMenu)
            row.openButton.target = self
            row.openButton.action = #selector(openBrowser(_:))
            row.openButton.tag = index
            row.openButton.isEnabled = false
            actions.addArrangedSubview(row.openButton)
            stack.addArrangedSubview(actions)
            stack.addArrangedSubview(row.status)
        }
        stack.addArrangedSubview(NSButton(title: "重新读取保存的授权", target: self, action: #selector(recheck)))
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder
        logView.isEditable = false
        logView.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        logView.isVerticallyResizable = true
        logView.autoresizingMask = [.width]
        scroll.documentView = logView
        stack.addArrangedSubview(scroll)
        scroll.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 180).isActive = true
        window.makeKeyAndOrderFront(nil)
    }

    func record(_ event: String, _ fields: [String: Any] = [:]) {
        var entry = fields
        entry["event"] = event
        entry["time"] = ISO8601DateFormatter().string(from: Date())
        events.append(entry)
        let text = ([event] + fields.keys.sorted().map { "\($0)=\(fields[$0]!)" }).joined(separator: "  ")
        logView.string += text + "\n"
        logView.scrollToEndOfDocument(nil)
        do {
            let directory = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
                .appendingPathComponent("LinkPilotSandboxProbe")
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let report: [String: Any] = ["pid": ProcessInfo.processInfo.processIdentifier,
                                        "sandboxHome": NSHomeDirectory(), "events": events]
            let data = try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: directory.appendingPathComponent("run-\(ProcessInfo.processInfo.processIdentifier).json"), options: .atomic)
        } catch {
            logView.string += "report-write-failed: \(error.localizedDescription)\n"
        }
    }

    func readProfiles(_ file: URL, row: BrowserControls, restored: Bool) throws {
        guard let expected = row.browser.localState else {
            throw failure("无法定位当前账户的浏览器数据目录")
        }
        record("metadata-selected", ["browser": row.browser.id, "selectedPath": file.path,
                                     "expectedPath": expected.path])
        guard file.standardizedFileURL == expected.standardizedFileURL else {
            throw failure("请选择该浏览器目录中的 Local State 文件")
        }
        guard file.startAccessingSecurityScopedResource() else {
            throw failure("无法激活文件授权，请重新选择 Local State")
        }
        defer { file.stopAccessingSecurityScopedResource() }
        let response: ScanResponse = try file.path.withCString { path in
            guard let output = linkpilot_probe_profiles(path) else { throw failure("解析器未返回结果") }
            defer { linkpilot_probe_free(output) }
            return try JSONDecoder().decode(ScanResponse.self, from: Data(String(cString: output).utf8))
        }
        if let error = response.error { throw failure(error) }
        guard let profiles = response.profiles else { throw failure("解析器返回格式错误") }
        let bookmark = try file.bookmarkData(options: [.withSecurityScope, .securityScopeAllowOnlyReadAccess],
                                             includingResourceValuesForKeys: nil, relativeTo: nil)
        UserDefaults.standard.set(bookmark, forKey: "bookmark.\(row.browser.id)")
        // This probe is deliberately restarted immediately after granting access.
        guard UserDefaults.standard.synchronize() else { throw failure("保存授权失败") }
        row.profiles = profiles
        row.profilesMenu.removeAllItems()
        for profile in profiles { row.profilesMenu.addItem(withTitle: "\(profile.id) · \(profile.name)") }
        if profiles.isEmpty { row.profilesMenu.addItem(withTitle: "未发现 Profile") }
        row.profilesMenu.isEnabled = !profiles.isEmpty
        row.status.stringValue = "\(restored ? "已恢复保存的授权" : "已保存只读授权")：\(profiles.count) 个 Profile"
        record("profiles-read", ["browser": row.browser.id, "count": profiles.count,
                                 "profileIDs": profiles.map(\.id), "restored": restored])
    }

    func failure(_ text: String) -> NSError {
        NSError(domain: "LinkPilotSandboxProbe", code: 1, userInfo: [NSLocalizedDescriptionKey: text])
    }

    func failed(_ error: Error, row: BrowserControls) {
        row.status.stringValue = "失败：\(error.localizedDescription)；可重新授权"
        row.profiles = []
        row.profilesMenu.isEnabled = false
        record("access-failed", ["browser": row.browser.id, "detail": error.localizedDescription])
    }

    @objc func authorize(_ sender: NSButton) {
        let row = controls[sender.tag]
        guard sandboxVerified else { failed(failure("沙盒负向验证未通过"), row: row); return }
        guard let expected = row.browser.localState else {
            failed(failure("无法定位当前账户的浏览器数据目录"), row: row); return
        }
        let panel = NSOpenPanel()
        panel.title = "选择 \(row.browser.name) 的 Local State"
        panel.message = "仅授权读取此元数据文件，用于枚举 Profile。"
        panel.prompt = "只读授权"
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowsMultipleSelection = false
        panel.directoryURL = expected.deletingLastPathComponent()
        panel.beginSheetModal(for: window) { [weak self] result in
            guard let self = self else { return }
            guard result == .OK, let file = panel.url else {
                self.record("authorization-cancelled", ["browser": row.browser.id]); return
            }
            do { try self.readProfiles(file, row: row, restored: false) }
            catch { self.failed(error, row: row) }
        }
    }

    func checkBookmarks() {
        for row in controls {
            guard let data = UserDefaults.standard.data(forKey: "bookmark.\(row.browser.id)") else {
                record("authorization-needed", ["browser": row.browser.id]); continue
            }
            do {
                var stale = false
                let file = try URL(resolvingBookmarkData: data, options: [.withSecurityScope, .withoutUI],
                                   relativeTo: nil, bookmarkDataIsStale: &stale)
                if stale { record("bookmark-stale", ["browser": row.browser.id]) }
                try readProfiles(file, row: row, restored: true)
            } catch { failed(error, row: row) }
        }
    }

    @objc func recheck() { if sandboxVerified { checkBookmarks() } }

    @objc func openBrowser(_ sender: NSButton) {
        let row = controls[sender.tag]
        guard sandboxVerified else { return }
        guard let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: row.browser.bundleID) else {
            row.status.stringValue = "打开失败：未找到浏览器应用"
            record("browser-open-failed", ["browser": row.browser.id, "detail": "未找到浏览器应用"])
            return
        }
        var test = URLComponents(string: "https://example.com/")!
        test.fragment = "linkpilot-sandbox-\(row.browser.id)-\(UUID().uuidString)"
        let testURL = test.url!.absoluteString
        let configuration = NSWorkspace.OpenConfiguration()
        // Sandbox callers cannot pass command-line arguments. Open URLs through
        // the native URL API; this deliberately makes no profile-selection claim.
        record("browser-open-request", ["browser": row.browser.id, "appPath": app.path, "testURL": testURL])
        NSWorkspace.shared.open([test.url!], withApplicationAt: app,
                                configuration: configuration) { [weak self] running, error in
            DispatchQueue.main.async {
                guard let self = self else { return }
                if let error = error {
                    let nsError = error as NSError
                    self.record("browser-open-failed", ["browser": row.browser.id, "detail": error.localizedDescription,
                                                        "errorDomain": nsError.domain, "errorCode": nsError.code])
                    row.status.stringValue = "打开失败：\(error.localizedDescription)"
                } else {
                    self.record("browser-open-submitted", ["browser": row.browser.id,
                                                           "pid": running?.processIdentifier ?? 0])
                    row.status.stringValue = "打开请求已发送；未指定 Profile，请核对测试标签页"
                }
            }
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

let app = NSApplication.shared
let delegate = Probe()
app.delegate = delegate
app.setActivationPolicy(.regular)
app.run()
