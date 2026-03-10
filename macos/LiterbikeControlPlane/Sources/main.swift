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

private final class AppDelegate: NSObject, NSApplicationDelegate, WKNavigationDelegate, NSWindowDelegate {
    private var statusItem: NSStatusItem?
    private var window: NSWindow?
    private var webView: WKWebView?
    private var server: StaticAssetServer?

    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            guard let resourceRoot = Bundle.main.resourceURL?.appendingPathComponent("ControlPlaneResources") else {
                throw NSError(
                    domain: "LiterbikeControlPlane",
                    code: 2,
                    userInfo: [NSLocalizedDescriptionKey: "Missing ControlPlaneResources bundle directory."]
                )
            }

            let server = StaticAssetServer(resourceRoot: resourceRoot)
            try server.start()
            self.server = server
        } catch {
            NSAlert(error: error).runModal()
        }

        setupStatusItem()
        setupWindow()
        showWindow(nil)
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag {
            showWindow(nil)
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
    private func showWindow(_ sender: Any?) {
        guard let window else { return }
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    @objc
    private func reloadControlPlane(_ sender: Any?) {
        webView?.reload()
    }

    @objc
    private func copyControlPlaneURL(_ sender: Any?) {
        guard let url = server?.baseURL else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(url.absoluteString, forType: .string)
    }

    @objc
    private func quit(_ sender: Any?) {
        NSApp.terminate(nil)
    }

    private func setupStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = item.button {
            button.image = loadTemplateStatusIcon()
            button.imagePosition = .imageOnly
            button.toolTip = "Literbike Control Plane"
        }

        let menu = NSMenu()
        menu.addItem(NSMenuItem(title: "Open Control Plane", action: #selector(showWindow(_:)), keyEquivalent: ""))
        menu.addItem(NSMenuItem(title: "Reload", action: #selector(reloadControlPlane(_:)), keyEquivalent: ""))
        menu.addItem(NSMenuItem(title: "Copy Local URL", action: #selector(copyControlPlaneURL(_:)), keyEquivalent: ""))
        menu.addItem(.separator())
        menu.addItem(NSMenuItem(title: "Quit", action: #selector(quit(_:)), keyEquivalent: ""))
        for item in menu.items {
            item.target = self
        }

        item.menu = menu
        statusItem = item
    }

    private func setupWindow() {
        let webView = WKWebView(frame: .zero)
        webView.navigationDelegate = self
        self.webView = webView

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1180, height: 860),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.center()
        window.title = "Literbike Control Plane"
        window.contentView = webView
        window.delegate = self
        window.isReleasedWhenClosed = false
        self.window = window

        if let url = server?.baseURL {
            webView.load(URLRequest(url: url))
        } else {
            webView.loadHTMLString(
                """
                <html><body style="font-family: -apple-system; padding: 24px;">
                <h1>Control plane failed to start</h1>
                <p>The local asset server did not bind. Check the menu-bar app logs and retry.</p>
                </body></html>
                """,
                baseURL: nil
            )
        }
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
