import SwiftUI
import AppKit

/// NSTextView wrapper for SQL editing with monospace font and Cmd+Enter support.
struct SQLEditorView: NSViewRepresentable {
    @Environment(AppViewModel.self) private var appVM

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSTextView.scrollableTextView()
        guard let textView = scrollView.documentView as? NSTextView else {
            return scrollView
        }

        context.coordinator.textView = textView
        textView.delegate = context.coordinator
        textView.isRichText = false
        textView.allowsUndo = true
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false

        // Appearance
        textView.backgroundColor = NSColor(SbqlTheme.Colors.surface)
        textView.textColor = NSColor(SbqlTheme.Colors.textPrimary)
        textView.insertionPointColor = NSColor(SbqlTheme.Colors.accent)
        textView.font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
        textView.textContainerInset = NSSize(width: 12, height: 10)

        // Line wrapping
        textView.isHorizontallyResizable = false
        textView.textContainer?.widthTracksTextView = true

        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.borderType = .noBorder
        scrollView.backgroundColor = NSColor(SbqlTheme.Colors.surface)

        // Syntax highlighting: try tree-sitter, fall back to regex
        if let tsHighlighter = try? TreeSitterHighlighter(textView: textView) {
            context.coordinator.treeSitterHighlighter = tsHighlighter
        } else {
            textView.textStorage?.delegate = context.coordinator.regexHighlighter
        }

        // Initial text
        textView.string = appVM.editor.sqlText

        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard let textView = scrollView.documentView as? NSTextView else { return }
        context.coordinator.appVM = appVM
        if textView.string != appVM.editor.sqlText {
            context.coordinator.suppressCompletions = true
            let selection = textView.selectedRanges
            textView.string = appVM.editor.sqlText
            textView.selectedRanges = selection
            context.coordinator.suppressCompletions = false
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(appVM: appVM)
    }

    class Coordinator: NSObject, NSTextViewDelegate {
        var appVM: AppViewModel
        weak var textView: NSTextView?

        let regexHighlighter = SQLSyntaxHighlighter()
        var treeSitterHighlighter: TreeSitterHighlighter?
        let completionPanel = CompletionPanel()
        var completionRange: NSRange?
        var suppressCompletions = false

        init(appVM: AppViewModel) {
            self.appVM = appVM
            super.init()
            completionPanel.onAccept = { [weak self] item in
                self?.insertCompletion(item)
            }
        }

        func textDidChange(_ notification: Notification) {
            guard let textView = notification.object as? NSTextView else { return }
            appVM.editor.sqlText = textView.string

            if !suppressCompletions {
                updateCompletions(for: textView)
            }
        }

        func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if completionPanel.isVisible {
                if commandSelector == #selector(NSResponder.moveUp(_:)) {
                    completionPanel.moveUp()
                    return true
                }
                if commandSelector == #selector(NSResponder.moveDown(_:)) {
                    completionPanel.moveDown()
                    return true
                }
                if commandSelector == #selector(NSResponder.insertTab(_:)) {
                    completionPanel.acceptSelected()
                    return true
                }
                if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                    if !NSEvent.modifierFlags.contains(.command) {
                        completionPanel.acceptSelected()
                        return true
                    }
                }
                if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
                    completionPanel.dismiss()
                    return true
                }
            }

            // Cmd+Enter to execute
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                if NSEvent.modifierFlags.contains(.command) {
                    completionPanel.dismiss()
                    Task { @MainActor in
                        await self.appVM.runQuery()
                    }
                    return true
                }
            }
            return false
        }

        // MARK: - Completion logic

        func updateCompletions(for textView: NSTextView) {
            let text = textView.string
            let cursorLocation = textView.selectedRange().location
            guard cursorLocation > 0, cursorLocation <= text.count else {
                completionPanel.dismiss()
                return
            }

            // Walk back to find word prefix
            let nsText = text as NSString
            var start = cursorLocation
            while start > 0 {
                let ch = nsText.character(at: start - 1)
                let scalar = Unicode.Scalar(ch)!
                if CharacterSet.alphanumerics.contains(scalar) || scalar == "_" {
                    start -= 1
                } else {
                    break
                }
            }

            let prefixRange = NSRange(location: start, length: cursorLocation - start)
            let prefix = nsText.substring(with: prefixRange)

            guard prefix.count >= 1 else {
                completionPanel.dismiss()
                return
            }

            let tables = appVM.diagram.diagramData.tables
            let items = SQLCompletionProvider.completions(prefix: prefix, tables: tables)

            // Dismiss if empty or single exact match
            if items.isEmpty || (items.count == 1 && items[0].text.caseInsensitiveCompare(prefix) == .orderedSame) {
                completionPanel.dismiss()
                return
            }

            completionRange = prefixRange

            // Position panel below caret
            var actualRange = NSRange()
            let caretRect = textView.firstRect(forCharacterRange: textView.selectedRange(), actualRange: &actualRange)
            let screenPoint = NSPoint(x: caretRect.origin.x, y: caretRect.origin.y)

            completionPanel.show(items: items, at: screenPoint)
        }

        func insertCompletion(_ item: CompletionItem) {
            guard let textView = textView, let range = completionRange else { return }
            suppressCompletions = true
            textView.insertText(item.text, replacementRange: range)
            suppressCompletions = false
            completionPanel.dismiss()
        }
    }
}
