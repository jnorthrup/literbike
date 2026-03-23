import AppKit
import Foundation

// MARK: - Server Response (ACTUAL format from localhost:8888)

private struct ServerRoute: Decodable {
    let provider: String?
    let model: String?
    let family: String
}

private struct ServerEnv: Decodable {
    let recognized_keys: Int
    let unknown_keys: Int
    let confidence: String
}

private struct ToolbarState: Decodable {
    let route: ServerRoute
    let env: ServerEnv
}

// MARK: - Provider Config (from DSEL - ONLY these are hardcoded)

private let providerHosts: [(String, String, String)] = [
    ("anthropic",  "https://api.anthropic.com/v1",                           "ANTHROPIC_API_KEY"),
    ("openai",     "https://api.openai.com/v1",                              "OPENAI_API_KEY"),
    ("google",     "https://generativelanguage.googleapis.com/v1beta/openai", "GOOGLE_API_KEY"),
    ("gemini",     "https://generativelanguage.googleapis.com/v1beta/openai", "GOOGLE_API_KEY"),
    ("groq",       "https://api.groq.com/openai/v1",                         "GROQ_API_KEY"),
    ("openrouter", "https://openrouter.ai/api/v1",                           "OPENROUTER_API_KEY"),
    ("mistral",    "https://api.mistral.ai/v1",                              "MISTRAL_API_KEY"),
    ("xai",        "https://api.x.ai/v1",                                    "XAI_API_KEY"),
    ("cerebras",   "https://api.cerebras.ai/v1",                             "CEREBRAS_API_KEY"),
    ("kilocode",   "https://api.kilocode.ai",                                "KILOCODE_API_KEY"),
    ("opencode",   "https://api.opencode.ai",                                "OPENCODE_API_KEY"),
    ("zai",        "https://api.z.ai/v1",                                    "ZAI_API_KEY"),
    ("nvidia",     "https://api.nvidia.com/v1",                              "NVIDIA_API_KEY"),
    ("moonshot",   "https://api.moonshot.cn/v1",                             "MOONSHOT_API_KEY"),
    ("ollama",     "http://localhost:11434/v1",                              ""),
    ("lmstudio",   "http://localhost:1234/v1",                               ""),
]

// MARK: - App

private final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem?
    private var currentProvider: String = ""
    private var currentModel: String = ""
    
    func applicationDidFinishLaunching(_ notification: Notification) {
        setupStatusItem()
        fetchStatus()
        Timer.scheduledTimer(withTimeInterval: 5.0, repeats: true) { [weak self] _ in
            self?.fetchStatus()
        }
    }
    
    // MARK: - Grandfather's Icon
    
    private func loadTemplateStatusIcon() -> NSImage? {
        let paths = [
            Bundle.main.resourcePath.flatMap { $0 + "/literbike-vrod-icon.svg" },
            Bundle.main.resourcePath.flatMap { $0 + "/Resources/literbike-vrod-icon.svg" },
            FileManager.default.currentDirectoryPath + "/literbike-vrod-icon.svg",
            FileManager.default.currentDirectoryPath + "/macos/LiterbikeControlPlane/Resources/literbike-vrod-icon.svg",
        ].compactMap { $0 }
        
        for path in paths {
            if FileManager.default.fileExists(atPath: path),
               let image = NSImage(contentsOfFile: path) {
                image.isTemplate = true
                image.size = NSSize(width: 18, height: 18)
                return image
            }
        }
        return nil
    }
    
    // MARK: - Data
    
    private func fetchStatus() {
        guard let url = URL(string: "http://localhost:8888/toolbar/state") else { return }
        URLSession.shared.dataTask(with: url) { [weak self] data, _, _ in
            guard let data = data,
                  let state = try? JSONDecoder().decode(ToolbarState.self, from: data) else { return }
            DispatchQueue.main.async {
                self?.currentProvider = state.route.provider ?? ""
                self?.currentModel = state.route.model ?? ""
                self?.updateTitle()
                self?.updateMenu()
            }
        }.resume()
    }
    
    // MARK: - UI
    
    private func setupStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        item.button?.image = loadTemplateStatusIcon()
        item.button?.imagePosition = .imageLeft
        statusItem = item
        updateMenu()
    }
    
    private func updateTitle() {
        if !currentProvider.isEmpty {
            statusItem?.button?.title = " \(currentProvider.uppercased())"
        }
    }
    
    private func updateMenu() {
        let menu = NSMenu()
        let env = ProcessInfo.processInfo.environment
        
        // Active route
        if !currentProvider.isEmpty {
            let active = NSMenuItem(title: "✓ \(currentProvider.uppercased())", action: nil, keyEquivalent: "")
            active.isEnabled = false
            menu.addItem(active)
            if !currentModel.isEmpty {
                let modelItem = NSMenuItem(title: "  → \(currentModel)", action: nil, keyEquivalent: "")
                modelItem.isEnabled = false
                menu.addItem(modelItem)
            }
            menu.addItem(.separator())
        }
        
        // Provider tree: host mappings from DSEL
        menu.addItem(NSMenuItem(title: "PROVIDERS", action: nil, keyEquivalent: ""))
        
        for (name, host, keyEnv) in providerHosts {
            let hasKey = !keyEnv.isEmpty && env[keyEnv] != nil && !env[keyEnv]!.isEmpty
            let indicator = hasKey ? "✓" : "○"
            let item = NSMenuItem(title: "\(indicator) \(name)", action: nil, keyEquivalent: "")
            
            // Submenu with host
            let sub = NSMenu()
            let hostItem = NSMenuItem(title: host, action: nil, keyEquivalent: "")
            hostItem.isEnabled = false
            sub.addItem(hostItem)
            
            if hasKey {
                let keyItem = NSMenuItem(title: "Key: \(keyEnv)", action: nil, keyEquivalent: "")
                keyItem.isEnabled = false
                sub.addItem(keyItem)
            }
            
            item.submenu = sub
            menu.addItem(item)
        }
        
        menu.addItem(.separator())
        menu.addItem(NSMenuItem(title: "REFRESH", action: #selector(refresh(_:)), keyEquivalent: "r"))
        menu.addItem(NSMenuItem(title: "QUIT", action: #selector(quit(_:)), keyEquivalent: "q"))
        
        statusItem?.menu = menu
    }
    
    @objc private func refresh(_ sender: NSMenuItem) {
        fetchStatus()
    }
    
    @objc private func quit(_ sender: NSMenuItem) {
        NSApp.terminate(nil)
    }
}

private let app = NSApplication.shared
private let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
