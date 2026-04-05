import AppKit
import SwiftUI

/// NSTextView wrapper for SQL editing with monospace font and Cmd+Enter support.
struct SQLEditorView: NSViewRepresentable {
    var activeTheme: ThemeName
    @Environment(AppViewModel.self) private var appVM

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        let textView = MultiCursorTextView()
        textView.autoresizingMask = [.width, .height]
        textView.minSize = NSSize(width: 0, height: 0)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        scrollView.documentView = textView

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
        let _ = activeTheme

        // Update theme colors
        let newBg = NSColor(SbqlTheme.Colors.surface)
        let themeChanged = textView.backgroundColor != newBg
        textView.backgroundColor = newBg
        textView.insertionPointColor = NSColor(SbqlTheme.Colors.accent)
        scrollView.backgroundColor = newBg

        // Recreate syntax highlighter when theme changes
        if themeChanged {
            textView.typingAttributes = [
                .foregroundColor: NSColor(SbqlTheme.Colors.textPrimary),
                .font: NSFont.monospacedSystemFont(ofSize: 13, weight: .regular),
            ]
            if let tsHighlighter = try? TreeSitterHighlighter(textView: textView) {
                context.coordinator.treeSitterHighlighter = tsHighlighter
            } else if let storage = textView.textStorage {
                // Regex fallback: re-delegate and force re-highlight
                storage.delegate = context.coordinator.regexHighlighter
                let range = NSRange(location: 0, length: storage.length)
                storage.edited(.editedAttributes, range: range, changeInLength: 0)
            }
        }

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

        func textView(_: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
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
            guard let textView, let range = completionRange else { return }
            suppressCompletions = true
            textView.insertText(item.text, replacementRange: range)
            suppressCompletions = false
            completionPanel.dismiss()
        }
    }
}

// MARK: - Multi-Cursor NSTextView

/// Custom NSTextView that supports Option+Click to add multiple insertion points.
class MultiCursorTextView: NSTextView {
    private var extraCursors: [Int] = [] // character indices of additional cursors

    override func mouseDown(with event: NSEvent) {
        if event.modifierFlags.contains(.option) && !event.modifierFlags.contains(.shift) {
            // Option+Click: add a cursor at clicked position
            let point = convert(event.locationInWindow, from: nil)
            let index = characterIndexForInsertion(at: point)

            if !extraCursors.contains(index) {
                extraCursors.append(index)
            }

            // Build multiple selection ranges (zero-length = insertion point)
            var ranges = [NSValue(range: selectedRange())]
            for cursor in extraCursors {
                let safeIndex = min(cursor, string.count)
                ranges.append(NSValue(range: NSRange(location: safeIndex, length: 0)))
            }
            setSelectedRanges(ranges, affinity: .downstream, stillSelecting: false)
            return
        }

        // Normal click: reset extra cursors
        extraCursors.removeAll()
        super.mouseDown(with: event)
    }

    override func insertText(_ insertString: Any, replacementRange: NSRange) {
        if selectedRanges.count > 1, let text = insertString as? String {
            // Insert at all cursor positions (reverse order to preserve indices)
            let ranges = selectedRanges.map(\.rangeValue).sorted { $0.location > $1.location }
            for range in ranges {
                super.insertText(text, replacementRange: range)
            }
            extraCursors.removeAll()
            return
        }
        super.insertText(insertString, replacementRange: replacementRange)
    }

    override func deleteBackward(_ sender: Any?) {
        if selectedRanges.count > 1 {
            let ranges = selectedRanges.map(\.rangeValue).sorted { $0.location > $1.location }
            for range in ranges {
                let deleteRange = range.length > 0 ? range : NSRange(location: max(0, range.location - 1), length: 1)
                if deleteRange.location >= 0 && NSMaxRange(deleteRange) <= string.count {
                    replaceCharacters(in: deleteRange, with: "")
                }
            }
            extraCursors.removeAll()
            return
        }
        super.deleteBackward(sender)
    }
}
