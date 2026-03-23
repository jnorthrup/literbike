import AppKit
import Foundation

// MARK: - Models from Server

private struct ModelList: Decodable {
    let data: [Model]
    let object: String
}

private struct Model: Decodable {
    let id: String
    let object: String?
}

// MARK: - Toolbar State

private struct ServerRoute: Decodable {
    let provider: String?
    let model: String?
}

private struct ToolbarState: Decodable {
    let route: ServerRoute
}

// MARK: - App

private final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem?
    private var models: [String] = []
    private var currentProvider: String = ""
    private var currentModel: String = ""
    
    func applicationDidFinishLaunching(_ notification: Notification) {
        setupStatusItem()
        fetchModels()
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
    
    private func fetchModels() {
        guard let url = URL(string: "http://localhost:8888/v1/models") else { return }
        URLSession.shared.dataTask(with: url) { [weak self] data, _, _ in
            guard let data = data,
                  let list = try? JSONDecoder().decode(ModelList.self, from: data) else { return }
            DispatchQueue.main.async {
                self?.models = list.data.map { $0.id }.sorted()
                self?.updateMenu()
            }
        }.resume()
    }
    
    private func fetchStatus() {
        guard let url = URL(string: "http://localhost:8888/toolbar/state") else { return }
        URLSession.shared.dataTask(with: url) { [weak self] data, _, _ in
            guard let data = data,
                  let state = try? JSONDecoder().decode(ToolbarState.self, from: data) else { return }
            DispatchQueue.main.async {
                self?.currentProvider = state.route.provider ?? ""
                self?.currentModel = state.route.model ?? ""
                self?.updateTitle()
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
        
        // Models from server /v1/models
        menu.addItem(NSMenuItem(title: "MODELS (\(models.count))", action: nil, keyEquivalent: ""))
        
        // Group by provider prefix
        var byProvider: [String: [String]] = [:]
        for modelId in models {
            let parts = modelId.split(separator: "/")
            let provider = parts.first.map(String.init) ?? "unknown"
            byProvider[provider, default: []].append(modelId)
        }
        
        for (provider, providerModels) in byProvider.sorted(by: { $0.key < $1.key }) {
            let providerItem = NSMenuItem(title: provider.uppercased(), action: nil, keyEquivalent: "")
            let providerMenu = NSMenu()
            
            for modelId in providerModels.prefix(20) { // Limit to 20 per provider
                let name = modelId.split(separator: "/").dropFirst().joined(separator: "/")
                let modelItem = NSMenuItem(title: name, action: #selector(selectModel(_:)), keyEquivalent: "")
                modelItem.representedObject = modelId
                modelItem.target = self
                providerMenu.addItem(modelItem)
            }
            
            if providerModels.count > 20 {
                let moreItem = NSMenuItem(title: "... and \(providerModels.count - 20) more", action: nil, keyEquivalent: "")
                moreItem.isEnabled = false
                providerMenu.addItem(moreItem)
            }
            
            providerItem.submenu = providerMenu
            menu.addItem(providerItem)
        }
        
        menu.addItem(.separator())
        menu.addItem(NSMenuItem(title: "REFRESH", action: #selector(refresh(_:)), keyEquivalent: "r"))
        menu.addItem(NSMenuItem(title: "QUIT", action: #selector(quit(_:)), keyEquivalent: "q"))
        
        statusItem?.menu = menu
    }
    
    @objc private func selectModel(_ sender: NSMenuItem) {
        guard let modelId = sender.representedObject as? String else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(modelId, forType: .string)
        currentModel = modelId
        updateTitle()
    }
    
    @objc private func refresh(_ sender: NSMenuItem) {
        fetchModels()
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
