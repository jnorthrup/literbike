import AppKit
import Foundation
import Network

private struct DselLane: Decodable {
    let title: String
    let route: String
    let model: String
    let host: String
    let provider: String
    let key: String?
}

private struct ToolbarEnvKey: Decodable {
    let name: String
    let is_set: Bool
}

private struct ToolbarEnvState: Decodable {
    let keys: [ToolbarEnvKey]
}

private struct ProviderKeyResolution: Decodable {
    let provider: String
    let env_key: String?
    let selected_env_key: String?
    let key_present: Bool
}

private struct KeymuxState: Decodable {
    let strategy: String
    let provider_keys: [ProviderKeyResolution]
}

private struct ToolbarState: Decodable {
    let dynamic_models: [String]
    let env: ToolbarEnvState
    let keymux: KeymuxState
    let lanes: [DselLane]
    let route: ToolbarRoute
}

private struct ToolbarRoute: Decodable {
    let provider: String?
    let model: String?
    let family: String
}

private final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem?
    private var lanes: [DselLane] = []
    private var refreshTimer: Timer?
    private var lastState: ToolbarState?

    func applicationDidFinishLaunching(_ notification: Notification) {
        setupStatusItem()

        fetchStatus()
        refreshTimer = Timer.scheduledTimer(withTimeInterval: 5.0, repeats: true) { [weak self] _ in
            self?.fetchStatus()
        }
    }

    private func fetchStatus() {
        guard let url = URL(string: "http://localhost:8888/toolbar/state") else { return }

        URLSession.shared.dataTask(with: url) { [weak self] data, _, _ in
            guard let data = data else { return }
            do {
                let state = try JSONDecoder().decode(ToolbarState.self, from: data)
                DispatchQueue.main.async {
                    self?.lastState = state
                    self?.updateMenu()
                    self?.updateTitle(state: state)
                }
            } catch {
                print("Decode error: \(error)")
            }
        }.resume()
    }

    private func updateTitle(state: ToolbarState) {
        let label = [state.route.provider, state.route.model].compactMap { $0 }.joined(separator: " / ")
        if !label.isEmpty {
            statusItem?.button?.title = " " + label.uppercased()
        } else {
            statusItem?.button?.title = ""
        }
    }

    private func setupStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = item.button {
            button.image = loadTemplateStatusIcon()
            button.imagePosition = .imageLeft
        }
        let menu = NSMenu()
        menu.addItem(NSMenuItem(title: "LOADING...", action: nil, keyEquivalent: ""))
        menu.addItem(.separator())
        menu.addItem(NSMenuItem(title: "QUIT", action: #selector(quit(_:)), keyEquivalent: "q"))
        item.menu = menu
        statusItem = item
    }

    private func updateMenu() {
        guard let state = lastState, let menu = statusItem?.menu else { return }
        menu.removeAllItems()

        // --- KEYMUX section: KEY → {models} ---
        let keymuxRoot = NSMenuItem(title: "KEYMUX", action: nil, keyEquivalent: "")
        let keymuxMenu = NSMenu()

        // Build provider→key mapping from keymux state
        var providerToKey = [String: String]()
        for pk in state.keymux.provider_keys where pk.key_present {
            if let envKey = pk.selected_env_key {
                providerToKey[pk.provider] = envKey
            }
        }

        // Also include DSEL lane keys
        for lane in state.lanes {
            if let laneKey = lane.key, !laneKey.isEmpty {
                providerToKey[lane.provider] = laneKey
            }
        }

        // Group models by their resolved key
        var keyToModels = [String: [String]]()
        for modelId in state.dynamic_models {
            let provider = String(modelId.split(separator: "/").first ?? "unknown")
            if let key = providerToKey[provider] {
                keyToModels[key, default: []].append(modelId)
            }
        }
        // Add DSEL lane models under their keys
        for lane in state.lanes {
            if let laneKey = lane.key, !laneKey.isEmpty {
                if !keyToModels[laneKey, default: []].contains(lane.model) {
                    keyToModels[laneKey, default: []].append(lane.model)
                }
            }
        }

        for (key, models) in keyToModels.sorted(by: { $0.key < $1.key }) {
            let keyItem = NSMenuItem(title: "\(key) (\(models.count))", action: nil, keyEquivalent: "")
            let keySub = NSMenu()

            for modelId in models.sorted() {
                let name = modelId.split(separator: "/").last.map(String.init) ?? modelId
                let modelItem = NSMenuItem(title: name, action: #selector(launchModelAction(_:)), keyEquivalent: "")
                modelItem.representedObject = modelId
                modelItem.target = self
                keySub.addItem(modelItem)
            }

            keyItem.submenu = keySub
            keymuxMenu.addItem(keyItem)
        }

        // Keys with no models yet — clickable to trigger draw-through fetch
        let usedKeys = Set(keyToModels.keys)
        for pk in state.keymux.provider_keys where pk.key_present {
            if let envKey = pk.selected_env_key, !usedKeys.contains(envKey) {
                let item = NSMenuItem(title: "\(envKey) — FETCH", action: #selector(fetchModelsAction(_:)), keyEquivalent: "")
                item.target = self
                keymuxMenu.addItem(item)
            }
        }

        keymuxRoot.submenu = keymuxMenu
        menu.addItem(keymuxRoot)

        menu.addItem(.separator())

        // --- PROVIDERS section: existing hierarchy ---
        let providersRoot = NSMenuItem(title: "PROVIDERS", action: nil, keyEquivalent: "")
        let providersMenu = NSMenu()

        var groupedModels = [String: [String]]()
        for modelId in state.dynamic_models {
            let provider = String(modelId.split(separator: "/").first ?? "unknown")
            groupedModels[provider, default: []].append(modelId)
        }

        for (provider, models) in groupedModels.sorted(by: { $0.key < $1.key }) {
            let providerItem = NSMenuItem(title: provider.uppercased(), action: nil, keyEquivalent: "")
            let providerSub = NSMenu()

            let modelsItem = NSMenuItem(title: "models", action: nil, keyEquivalent: "")
            let modelsSub = NSMenu()

            let v1Item = NSMenuItem(title: "V1", action: nil, keyEquivalent: "")
            let v1Sub = NSMenu()

            for modelId in models.sorted() {
                let name = modelId.split(separator: "/").last.map(String.init) ?? modelId
                let modelItem = NSMenuItem(title: name, action: #selector(launchModelAction(_:)), keyEquivalent: "")
                modelItem.representedObject = modelId
                modelItem.target = self
                v1Sub.addItem(modelItem)
            }

            v1Item.submenu = v1Sub
            modelsSub.addItem(v1Item)
            modelsItem.submenu = modelsSub
            providerSub.addItem(modelsItem)
            providerItem.submenu = providerSub
            providersMenu.addItem(providerItem)
        }

        providersRoot.submenu = providersMenu
        menu.addItem(providersRoot)

        menu.addItem(.separator())
        menu.addItem(NSMenuItem(title: "QUIT", action: #selector(quit(_:)), keyEquivalent: "q"))
    }

    @objc private func launchAction(_ sender: NSMenuItem) {
        guard let lane = sender.representedObject as? DselLane else { return }
        probeLaunch(host: lane.host, model: lane.model, route: lane.route)
    }

    @objc private func launchModelAction(_ sender: NSMenuItem) {
        guard let modelId = sender.representedObject as? String, let state = lastState else { return }
        let lane = state.lanes.first(where: { $0.model == modelId })
        let route = lane?.route ?? "/{localhost:8888,chat}/\(modelId)"
        let host = lane?.host ?? "localhost:8888"
        probeLaunch(host: host, model: modelId, route: route)
    }

    private func probeLaunch(host: String, model: String, route: String) {
        guard let url = URL(string: "http://\(host)/probe") else { return }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        let body: [String: String] = ["model": model, "route": route, "action": "launch"]
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)

        URLSession.shared.dataTask(with: request) { [weak self] _, _, _ in
            self?.fetchStatus()
        }.resume()
    }

    @objc private func fetchModelsAction(_ sender: NSMenuItem) {
        // Hit /v1/models to trigger draw-through for providers with 0 cached models
        guard let url = URL(string: "http://localhost:8888/v1/models") else { return }
        URLSession.shared.dataTask(with: url) { [weak self] _, _, _ in
            // After draw-through completes, refresh menu
            self?.fetchStatus()
        }.resume()
    }

    @objc private func quit(_ sender: Any?) { NSApp.terminate(nil) }

    private func loadTemplateStatusIcon() -> NSImage? {
        let cwd = FileManager.default.currentDirectoryPath
        let iconPath = cwd + "/literbike-vrod-icon.svg"
        print("DEBUG: Attempting to load icon from: \(iconPath)")

        if !FileManager.default.fileExists(atPath: iconPath) {
            print("DEBUG: Icon file does not exist at path: \(iconPath)")
            return NSImage(named: "NSActionTemplate")
        }

        guard let image = NSImage(contentsOfFile: iconPath) else {
            print("DEBUG: Failed to initialize NSImage from path: \(iconPath)")
            return NSImage(named: "NSActionTemplate")
        }

        image.isTemplate = true
        image.size = NSSize(width: 18, height: 18)
        print("DEBUG: Successfully loaded and configured icon.")
        return image
    }
}

let app = NSApplication.shared
private let delegate = AppDelegate()
app.setActivationPolicy(.accessory)
app.delegate = delegate
app.run()
