import AppKit
import SwiftUI

/// Manages the NSStatusItem (menu bar icon) and its popover.
final class MenuBarManager: NSObject {
    private var statusItem: NSStatusItem?
    private var popover: NSPopover?
    private var appVM: AppViewModel

    init(appVM: AppViewModel) {
        self.appVM = appVM
        super.init()
        setupStatusItem()
    }

    private func setupStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        if let button = statusItem?.button {
            button.image = NSImage(systemSymbolName: "cylinder.split.1x2", accessibilityDescription: "sbql")
            button.image?.size = NSSize(width: 16, height: 16)
            button.action = #selector(togglePopover)
            button.target = self
            updateButtonTitle()
        }

        popover = NSPopover()
        popover?.behavior = .transient
        popover?.contentSize = NSSize(width: 380, height: 520)
        popover?.animates = true

        let contentView = MenuBarPopoverView()
            .environment(appVM)
            .environment(ThemeManager.shared)
        popover?.contentViewController = NSHostingController(rootView: contentView)
    }

    func updateButtonTitle() {
        guard let button = statusItem?.button else { return }
        if let conn = appVM.connections.activeConnection {
            button.title = " \(conn.name)"
            button.image = NSImage(systemSymbolName: "cylinder.split.1x2.fill", accessibilityDescription: "sbql - connected")
        } else {
            button.title = ""
            button.image = NSImage(systemSymbolName: "cylinder.split.1x2", accessibilityDescription: "sbql")
        }
    }

    @objc private func togglePopover() {
        guard let button = statusItem?.button, let popover else { return }
        if popover.isShown {
            popover.performClose(nil)
        } else {
            // Refresh the content view to get latest state
            let contentView = MenuBarPopoverView()
                .environment(appVM)
                .environment(ThemeManager.shared)
            popover.contentViewController = NSHostingController(rootView: contentView)
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
            popover.contentViewController?.view.window?.makeKey()
        }
    }
}
