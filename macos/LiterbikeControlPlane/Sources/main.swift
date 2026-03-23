import AppKit
import Foundation

// MARK: - Provider Config from JSON

private struct ProviderConfig: Decodable {
    let base_url: String
    let key_env: String
    let format: String
    let models: [String: ModelConfig]?
}

private struct ModelConfig: Decodable {
    let endpoint: String
    let format: String
}

private struct ProvidersRoot: Decodable {
    let providers: [String: ProviderConfig]
}

// MARK: - Server Response

private struct ServerRoute: Decodable {
    let provider: String?
    let model: String?
    let family: String
}

private struct ToolbarState: Decodable {
    let route: ServerRoute
}

// MARK: - App

private final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem?
    private var providers: [String: ProviderConfig] = [:]
    private var currentProvider: String = ""
    private var currentModel: String = ""
    
    func applicationDidFinishLaunching(_ notification: Notification) {
        loadProviders()
        setupStatusItem()
        fetchStatus()
        Timer.scheduledTimer(withTimeInterval: 5.0, repeats: true) { [weak self] _ in
            self?.fetchStatus()
        }
    }
    
    // MARK: - Load Providers from JSON
    
    private func loadProviders() {
        let paths = [
            Bundle.main.resourcePath.flatMap { $0 + "/providers.json" },
            Bundle.main.resourcePath.flatMap { $0 + "/Resources/providers.json" },
            FileManager.default.currentDirectoryPath + "/macos/LiterbikeControlPlane/Resources/providers.json",
        ].compactMap { $0 }
        
        for path in paths {
            if let data = try? Data(contentsOf: URL(fileURLWithPath: path)),
               let root = try? JSONDecoder().decode(ProvidersRoot.self, from: data) {
                providers = root.providers
                return
            }
        }
        print("Failed to load providers.json")
    }
    
    // MARK: - Grandfather's Icon
    
    private func loadTemplateStatusIcon() -> NSImage? {
        let paths = [
            Bundle.main.resourcePath.flatMap { $0 + "/literbike-vrod-icon.svg" },
            Bundle.main.resourcePath.flatMap { $0 + "/Resources/literbike-vrod-icon.svg" },
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
        
        // Provider tree from JSON
        menu.addItem(NSMenuItem(title: "PROVIDERS", action: nil, keyEquivalent: ""))
        
        for (name, config) in providers.sorted(by: { $0.key < $1.key }) {
            let hasKey = !config.key_env.isEmpty && env[config.key_env] != nil && !env[config.key_env]!.isEmpty
            let indicator = hasKey ? "✓" : "○"
            
            // Build display name with format indicator
            var displayName = "\(indicator) \(name)"
            if config.format == "dynamic", let models = config.models, !models.isEmpty {
                displayName += " [\(models.count) models]"
            }
            
            let item = NSMenuItem(title: displayName, action: nil, keyEquivalent: "")
            
            // Submenu
            let sub = NSMenu()
            
            // Base URL
            let urlItem = NSMenuItem(title: config.base_url, action: nil, keyEquivalent: "")
            urlItem.isEnabled = false
            sub.addItem(urlItem)
            
            // Format
            let formatItem = NSMenuItem(title: "format: \(config.format)", action: nil, keyEquivalent: "")
            formatItem.isEnabled = false
            sub.addItem(formatItem)
            
            // Key
            if !config.key_env.isEmpty {
                let keyItem = NSMenuItem(title: "key: \(config.key_env)", action: nil, keyEquivalent: "")
                keyItem.isEnabled = false
                sub.addItem(keyItem)
            }
            
            // Model-specific endpoints for dynamic providers
            if let models = config.models {
                sub.addItem(.separator())
                for (modelName, modelConfig) in models.sorted(by: { $0.key < $1.key }) {
                    let modelItem = NSMenuItem(
                        title: "\(modelName) → \(modelConfig.endpoint) [\(modelConfig.format)]",
                        action: nil,
                        keyEquivalent: ""
                    )
                    modelItem.isEnabled = false
                    sub.addItem(modelItem)
                }
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
        loadProviders()
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
