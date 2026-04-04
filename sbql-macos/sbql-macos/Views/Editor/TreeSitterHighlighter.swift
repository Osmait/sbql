import AppKit
import Neon
import SwiftTreeSitter
import SwiftUI
import TreeSitterSql

/// Tree-sitter based syntax highlighter using Neon's TextViewHighlighter.
/// Falls back to the regex-based SQLSyntaxHighlighter if initialization fails.
final class TreeSitterHighlighter {
    let highlighter: TextViewHighlighter

    init(textView: NSTextView) throws {
        let font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)

        textView.typingAttributes = [
            .foregroundColor: NSColor(SbqlTheme.Colors.textPrimary),
            .font: font,
        ]

        let sqlConfig = try LanguageConfiguration(
            tree_sitter_sql(),
            name: "SQL"
        )

        // Colors resolved dynamically inside the closure so they update with theme changes
        let provider: TokenAttributeProvider = { token in
            let color: NSColor = switch token.name {
            case "keyword", "keyword.operator":
                NSColor(SbqlTheme.Colors.accent)
            case "conditional":
                NSColor(SbqlTheme.Colors.accentHover)
            case "storageclass":
                NSColor(SbqlTheme.Colors.success)
            case "function.call":
                NSColor(SbqlTheme.Colors.accentHover)
            case "type", "type.builtin", "type.qualifier":
                NSColor(SbqlTheme.Colors.warning)
            case "string":
                NSColor(SbqlTheme.Colors.success)
            case "number", "float", "boolean":
                NSColor(SbqlTheme.Colors.warning)
            case "operator":
                NSColor(SbqlTheme.Colors.danger)
            case "field", "parameter":
                NSColor(SbqlTheme.Colors.accentHover)
            case "attribute":
                NSColor(SbqlTheme.Colors.success)
            case "variable":
                NSColor(SbqlTheme.Colors.textPrimary)
            case "comment", "spell":
                NSColor(SbqlTheme.Colors.textTertiary)
            case "punctuation.bracket", "punctuation.delimiter":
                NSColor(SbqlTheme.Colors.textTertiary)
            default:
                NSColor(SbqlTheme.Colors.textPrimary)
            }
            return [.foregroundColor: color, .font: font]
        }

        let config = TextViewHighlighter.Configuration(
            languageConfiguration: sqlConfig,
            attributeProvider: provider,
            languageProvider: { _ in nil },
            locationTransformer: { _ in nil }
        )

        highlighter = try TextViewHighlighter(textView: textView, configuration: config)
    }
}
