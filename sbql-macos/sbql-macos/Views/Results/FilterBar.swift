import AppKit
import SwiftUI

struct FilterBar: View {
    @Environment(AppViewModel.self) private var appVM

    var body: some View {
        @Bindable var results = appVM.results

        HStack(spacing: SbqlTheme.Spacing.sm) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 11))
                .foregroundStyle(SbqlTheme.Colors.textTertiary)

            FilterBarTextField(appVM: appVM)
                .frame(height: 20)

            if !results.filterText.isEmpty {
                Button {
                    results.filterText = ""
                    Task { await appVM.clearFilter() }
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 12))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.vertical, SbqlTheme.Spacing.sm)
        .background(SbqlTheme.Colors.surfaceElevated)
        .overlay(alignment: .bottom) {
            SbqlTheme.Colors.border.frame(height: 1)
        }
    }
}

// MARK: - NSViewRepresentable TextField

private struct FilterBarTextField: NSViewRepresentable {
    let appVM: AppViewModel

    func makeNSView(context: Context) -> NSTextField {
        let tf = NSTextField()
        tf.placeholderString = "Filter... (column:value or text)"
        tf.isBordered = false
        tf.drawsBackground = false
        tf.font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
        tf.textColor = NSColor(SbqlTheme.Colors.textPrimary)
        tf.focusRingType = .none
        tf.delegate = context.coordinator
        tf.stringValue = appVM.results.filterText
        context.coordinator.textField = tf

        DispatchQueue.main.async {
            tf.window?.makeFirstResponder(tf)
        }
        return tf
    }

    func updateNSView(_ tf: NSTextField, context: Context) {
        if tf.stringValue != appVM.results.filterText {
            tf.stringValue = appVM.results.filterText
            if appVM.results.filterText.isEmpty {
                context.coordinator.completionPanel.dismiss()
            }
        }
    }

    static func dismantleNSView(_: NSTextField, coordinator: Coordinator) {
        coordinator.completionPanel.dismiss()
        coordinator.debounceTask?.cancel()
        coordinator.valueSuggestTask?.cancel()
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(appVM: appVM)
    }

    // MARK: - Coordinator

    final class Coordinator: NSObject, NSTextFieldDelegate {
        let appVM: AppViewModel
        let completionPanel = CompletionPanel()
        var debounceTask: Task<Void, Never>?
        var valueSuggestTask: Task<Void, Never>?
        weak var textField: NSTextField?

        init(appVM: AppViewModel) {
            self.appVM = appVM
            super.init()
            completionPanel.onAccept = { [weak self] item in
                self?.acceptCompletion(item)
            }
        }

        // MARK: NSTextFieldDelegate

        func controlTextDidChange(_ obj: Notification) {
            guard let tf = obj.object as? NSTextField else { return }
            let text = tf.stringValue
            appVM.results.filterText = text

            updateCompletions(textField: tf)

            debounceTask?.cancel()
            debounceTask = Task { @MainActor [weak self] in
                do { try await Task.sleep(for: .milliseconds(300)) } catch { return }
                guard let self else { return }
                let q = text.trimmingCharacters(in: .whitespaces)
                if q.isEmpty {
                    await appVM.clearFilter()
                } else {
                    await appVM.applyFilter(query: q)
                }
            }
        }

        func control(
            _: NSControl,
            textView _: NSTextView,
            doCommandBy sel: Selector
        ) -> Bool {
            switch sel {
            case #selector(NSResponder.moveUp(_:)):
                guard completionPanel.isVisible else { return false }
                completionPanel.moveUp()
                return true

            case #selector(NSResponder.moveDown(_:)):
                guard completionPanel.isVisible else { return false }
                completionPanel.moveDown()
                return true

            case #selector(NSResponder.insertTab(_:)):
                guard completionPanel.isVisible else { return false }
                completionPanel.acceptSelected()
                return true

            case #selector(NSResponder.insertNewline(_:)):
                if completionPanel.isVisible {
                    completionPanel.acceptSelected()
                } else {
                    debounceTask?.cancel()
                    let q = appVM.results.filterText.trimmingCharacters(in: .whitespaces)
                    Task { @MainActor [weak self] in
                        guard let self else { return }
                        if q.isEmpty {
                            await appVM.clearFilter()
                        } else {
                            await appVM.applyFilter(query: q)
                        }
                    }
                }
                return true

            case #selector(NSResponder.cancelOperation(_:)):
                guard completionPanel.isVisible else { return false }
                completionPanel.dismiss()
                return true

            default:
                return false
            }
        }

        // MARK: Completion logic

        private func updateCompletions(textField tf: NSTextField) {
            let text = tf.stringValue
            guard !text.isEmpty else {
                completionPanel.dismiss()
                return
            }

            if let colonIdx = text.firstIndex(of: ":") {
                // Format is column:prefix → suggest values
                let column = String(text[text.startIndex ..< colonIdx])
                let prefix = String(text[text.index(after: colonIdx)...])
                let columns = appVM.results.currentResult.columns

                guard columns.contains(where: { $0.caseInsensitiveCompare(column) == .orderedSame }) else {
                    completionPanel.dismiss()
                    return
                }

                valueSuggestTask?.cancel()
                valueSuggestTask = Task { @MainActor [weak self] in
                    guard let self else { return }
                    let values = await appVM.suggestFilterValues(
                        column: column, prefix: prefix
                    )
                    guard !Task.isCancelled else { return }
                    let items = values.map {
                        CompletionItem(text: $0, detail: column, kind: .keyword)
                    }
                    if items.isEmpty {
                        completionPanel.dismiss()
                    } else {
                        completionPanel.show(
                            items: items, at: screenPointBelow(tf)
                        )
                    }
                }
            } else {
                // No colon → suggest columns matching prefix
                let lower = text.lowercased()
                let columns = appVM.results.currentResult.columns
                let items = columns
                    .filter { $0.lowercased().hasPrefix(lower) }
                    .prefix(10)
                    .map { CompletionItem(text: $0, detail: "column", kind: .column) }

                if items.isEmpty {
                    completionPanel.dismiss()
                } else {
                    completionPanel.show(items: Array(items), at: screenPointBelow(tf))
                }
            }
        }

        private func acceptCompletion(_ item: CompletionItem) {
            completionPanel.dismiss()

            let text = appVM.results.filterText

            if item.kind == .column {
                let newText = item.text + ":"
                appVM.results.filterText = newText
                textField?.stringValue = newText
                positionCursorAtEnd()
                // Trigger value suggestions for the selected column
                if let tf = textField {
                    updateCompletions(textField: tf)
                }
            } else {
                let newText: String = if let colonIdx = text.firstIndex(of: ":") {
                    String(text[text.startIndex ... colonIdx]) + item.text
                } else {
                    item.text
                }
                appVM.results.filterText = newText
                textField?.stringValue = newText
                positionCursorAtEnd()
                // Apply filter immediately when accepting a value
                debounceTask?.cancel()
                Task { @MainActor [weak self] in
                    await self?.appVM.applyFilter(query: newText)
                }
            }
        }

        private func positionCursorAtEnd() {
            if let editor = textField?.currentEditor() {
                let len = textField?.stringValue.count ?? 0
                editor.selectedRange = NSRange(location: len, length: 0)
            }
        }

        private func screenPointBelow(_ tf: NSTextField) -> NSPoint {
            guard let window = tf.window else { return .zero }
            let frame = tf.convert(tf.bounds, to: nil)
            return window.convertPoint(toScreen: NSPoint(x: frame.minX, y: frame.minY))
        }
    }
}
