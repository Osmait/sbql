import AppKit
import SwiftUI
import Neon
import SwiftTreeSitter
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

        let provider: TokenAttributeProvider = { token in
            let color: NSColor = switch token.name {
            case let kw where kw.hasPrefix("keyword"):
                NSColor(SbqlTheme.Colors.accent)
            case "string":
                NSColor(SbqlTheme.Colors.success)
            case "number", "float":
                NSColor(SbqlTheme.Colors.warning)
            case "comment":
                NSColor(SbqlTheme.Colors.textTertiary)
            case let fn where fn.hasPrefix("function"):
                NSColor(SbqlTheme.Colors.accentHover)
            case "operator":
                NSColor(SbqlTheme.Colors.textSecondary)
            case let t where t.hasPrefix("type"):
                NSColor(SbqlTheme.Colors.accent)
            case "boolean":
                NSColor(SbqlTheme.Colors.warning)
            case "attribute":
                NSColor(SbqlTheme.Colors.accentHover)
            case "parameter":
                NSColor(SbqlTheme.Colors.warning)
            case "variable":
                NSColor(SbqlTheme.Colors.textPrimary)
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
