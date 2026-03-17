import AppKit
import Foundation
import Network
import WebKit

private struct StaticAsset {
    let relativePath: String
    let contentType: String
}

private final class StaticAssetServer {
    private let resourceRoot: URL
    private let queue = DispatchQueue(label: "com.literbike.control-plane.server")
    private var listener: NWListener?
    private(set) var baseURL: URL?

    private let routes: [String: StaticAsset] = [
        "/": StaticAsset(relativePath: "index.html", contentType: "text/html; charset=utf-8"),
        "/index.html": StaticAsset(relativePath: "index.html", contentType: "text/html; charset=utf-8"),
        "/index.css": StaticAsset(relativePath: "index.css", contentType: "text/css; charset=utf-8"),
        "/configs/agent-host-free-lanes.dsel": StaticAsset(
            relativePath: "configs/agent-host-free-lanes.dsel",
            contentType: "text/plain; charset=utf-8"
        ),
        "/bw_test_pattern.png": StaticAsset(relativePath: "bw_test_pattern.png", contentType: "image/png"),
        "/literbike-vrod-icon.svg": StaticAsset(
            relativePath: "literbike-vrod-icon.svg",
            contentType: "image/svg+xml"
        ),
    ]

    init(resourceRoot: URL) {
        self.resourceRoot = resourceRoot
    }

    func start() throws {
        for portValue in [41731, 41732, 41733, 41734, 41735] {
            let port = NWEndpoint.Port(rawValue: UInt16(portValue))!
            let candidate = try NWListener(using: .tcp, on: port)
            let startup = DispatchSemaphore(value: 0)
            var startupError: NWError?

            candidate.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    startup.signal()
                case .failed(let error):
                    startupError = error
                    startup.signal()
                default:
                    break
                }
            }

            candidate.newConnectionHandler = { [weak self] connection in
                self?.handle(connection)
            }

            candidate.start(queue: queue)

            if startup.wait(timeout: .now() + 2) == .success, startupError == nil {
                listener = candidate
                baseURL = URL(string: "http://127.0.0.1:\(portValue)/")
                return
            }

            candidate.cancel()
        }

        throw NSError(
            domain: "LiterbikeControlPlane",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: "Failed to bind a local control-plane port."]
        )
    }

    func stop() {
        listener?.cancel()
        listener = nil
    }

    private func handle(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { [weak self] data, _, _, _ in
            guard let self else {
                connection.cancel()
                return
            }
            let response = self.response(for: data)
            connection.send(content: response, completion: .contentProcessed { _ in
                connection.cancel()
            })
        }
    }

    private func response(for data: Data?) -> Data {
        guard
            let data,
            let request = String(data: data, encoding: .utf8),
            let requestLine = request.split(separator: "\r\n", maxSplits: 1).first
        else {
            return httpResponse(status: "400 Bad Request", contentType: "text/plain; charset=utf-8", body: Data("bad request\n".utf8))
        }

        let parts = requestLine.split(separator: " ")
        guard parts.count >= 2 else {
            return httpResponse(status: "400 Bad Request", contentType: "text/plain; charset=utf-8", body: Data("bad request\n".utf8))
        }

        let rawPath = String(parts[1])
        let path = rawPath.split(separator: "?", maxSplits: 1).first.map(String.init) ?? rawPath

        guard let asset = routes[path] else {
            return httpResponse(status: "404 Not Found", contentType: "text/plain; charset=utf-8", body: Data("not found\n".utf8))
        }

        let fileURL = resourceRoot.appendingPathComponent(asset.relativePath)
        let body = (try? Data(contentsOf: fileURL)) ?? Data()
        if body.isEmpty, !FileManager.default.fileExists(atPath: fileURL.path) {
            return httpResponse(
                status: "500 Internal Server Error",
                contentType: "text/plain; charset=utf-8",
                body: Data("missing bundled asset: \(asset.relativePath)\n".utf8)
            )
        }

        return httpResponse(status: "200 OK", contentType: asset.contentType, body: body)
    }

    private func httpResponse(status: String, contentType: String, body: Data) -> Data {
        let header = """
        HTTP/1.1 \(status)\r
        Content-Type: \(contentType)\r
        Content-Length: \(body.count)\r
        Cache-Control: no-cache\r
        Connection: close\r
        \r
        """
        var response = Data(header.utf8)
        response.append(body)
        return response
    }
}

private struct DselLane {
    let title: String
    let route: String
    let model: String
    let host: String
}

private final class AppDelegate: NSObject, NSApplicationDelegate, WKNavigationDelegate, NSWindowDelegate, WKScriptMessageHandler {
    private var statusItem: NSStatusItem?
    private var window: NSWindow?
    private var webView: WKWebView?
    private var server: StaticAssetServer?
    private var lanes: [DselLane] = []

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        if message.name == "updateTitle", let title = message.body as? String {
            statusItem?.button?.title = " " + title
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            guard let resourceRoot = Bundle.main.resourceURL?.appendingPathComponent("ControlPlaneResources") else {
                throw NSError(
                    domain: "LiterbikeControlPlane",
                    code: 2,
                    userInfo: [NSLocalizedDescriptionKey: "Missing ControlPlaneResources bundle directory."]
                )
            }

            // Read DSEL directly from bundle
            let dselUrl = resourceRoot.appendingPathComponent("configs/agent-host-free-lanes.dsel")
            if let content = try? String(contentsOf: dselUrl, encoding: .utf8) {
                self.lanes = content.split(separator: "\n").compactMap { line -> DselLane? in
                    let trimmed = String(line).trimmingCharacters(in: .whitespaces)
                    if trimmed.isEmpty || trimmed.hasPrefix("#") { return nil }
                    
                    guard let end = trimmed.firstIndex(of: "}"),
                          let slash = trimmed[end...].firstIndex(of: "/") else { return nil }
                          
                    let model = String(trimmed[trimmed.index(after: slash)...])
                    let meta = trimmed[trimmed.index(after: trimmed.firstIndex(of: "{")!)..<end]
                    let host = meta.split(separator: ",").first.map(String.init) ?? "localhost:8888"
                    
                    return DselLane(title: trimmed, route: trimmed, model: model, host: host)
                }
            }

            let server = StaticAssetServer(resourceRoot: resourceRoot)
            try server.start()
            self.server = server
        } catch {
            NSAlert(error: error).runModal()
            NSApp.terminate(nil)
        }

        setupStatusItem()
        setupWindow()
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag {
            window?.makeKeyAndOrderFront(nil)
        }
        return true
    }

    func applicationWillTerminate(_ notification: Notification) {
        server?.stop()
    }

    func windowWillClose(_ notification: Notification) {
        NSApp.hide(nil)
    }

    @objc
    private func quit(_ sender: Any?) {
        NSApp.terminate(nil)
    }

    @objc
    private func launchLaneAction(_ sender: NSMenuItem) {
        guard let lane = sender.representedObject as? DselLane else { return }
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        let js = "launchLane('\(lane.route)', '\(lane.host)', '\(lane.model)')"
        webView?.evaluateJavaScript(js, completionHandler: nil)
    }

    private func setupStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = item.button {
            button.image = loadTemplateStatusIcon()
            button.imagePosition = .imageLeft
            button.toolTip = "Literbike Control Plane"
        }

        let menu = NSMenu()
        
        // MAP DSEL TO MENU
        for lane in lanes {
            let menuItem = NSMenuItem(title: lane.title, action: #selector(launchLaneAction(_:)), keyEquivalent: "")
            menuItem.representedObject = lane
            menuItem.target = self
            menu.addItem(menuItem)
        }
        
        menu.addItem(.separator())
        menu.addItem(NSMenuItem(title: "Quit", action: #selector(quit(_:)), keyEquivalent: "q"))

        item.menu = menu
        statusItem = item
    }

    private func setupWindow() {
        let config = WKWebViewConfiguration()
        config.userContentController.add(self, name: "updateTitle")
        
        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = self
        // webView.setValue(false, forKey: "drawsBackground")
        self.webView = webView

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 800, height: 32),
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.center()
        window.title = "Literbike Signal Ticker"
        window.contentView = webView
        window.delegate = self
        window.isReleasedWhenClosed = false
        window.level = .floating // Stay on top
        self.window = window

        if let url = server?.baseURL {
            webView.load(URLRequest(url: url))
        }
        window.makeKeyAndOrderFront(nil)
    }

    private func loadTemplateStatusIcon() -> NSImage? {
        guard let iconURL = Bundle.main.resourceURL?.appendingPathComponent("StatusIconTemplate.png") else {
            return nil
        }
        let image = NSImage(contentsOf: iconURL)
        image?.isTemplate = true
        image?.size = NSSize(width: 18, height: 18)
        return image
    }
}

let app = NSApplication.shared
private let delegate = AppDelegate()
app.setActivationPolicy(.accessory)
app.delegate = delegate
app.run()
