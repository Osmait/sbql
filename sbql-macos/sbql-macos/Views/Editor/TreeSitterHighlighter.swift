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

        // Syntax colors derived from current theme palette
        let mauve = NSColor(SbqlTheme.Colors.accent) // keywords
        let red = NSColor(SbqlTheme.Colors.danger) // operators, delimiters
        let peach = NSColor(SbqlTheme.Colors.warning) // numbers, booleans
        let green = NSColor(SbqlTheme.Colors.success) // strings
        let yellow = NSColor(SbqlTheme.Colors.warning) // types, builtins
        let blue = NSColor(SbqlTheme.Colors.accentHover) // functions
        let sapphire = NSColor(SbqlTheme.Colors.accentHover) // fields, parameters
        let teal = NSColor(SbqlTheme.Colors.success) // attributes, storage
        let lavender = NSColor(SbqlTheme.Colors.accentHover) // conditionals
        let flamingo = NSColor(SbqlTheme.Colors.textPrimary) // variables
        let overlay1 = NSColor(SbqlTheme.Colors.textTertiary) // comments
        let surface2 = NSColor(SbqlTheme.Colors.textTertiary) // punctuation
        let text = NSColor(SbqlTheme.Colors.textPrimary) // default

        let provider: TokenAttributeProvider = { token in
            let color: NSColor = switch token.name {
            // Keywords: SELECT, FROM, WHERE, JOIN, etc.
            case "keyword", "keyword.operator":
                mauve
            // Conditionals: CASE, WHEN, THEN, ELSE
            case "conditional":
                lavender
            // Storage: TEMP, MATERIALIZED
            case "storageclass":
                teal
            // Functions: COUNT, SUM, etc.
            case "function.call":
                blue
            // Types: INT, VARCHAR, JSON, etc.
            case "type", "type.builtin", "type.qualifier":
                yellow
            // Strings
            case "string":
                green
            // Numbers
            case "number", "float":
                peach
            // Booleans: TRUE, FALSE
            case "boolean":
                peach
            // Operators: =, <, >, +, -, etc.
            case "operator":
                red
            // Fields: column names
            case "field":
                sapphire
            // Parameters
            case "parameter":
                sapphire
            // Attributes: ASC, DESC, DEFAULT
            case "attribute":
                teal
            // Variables / aliases
            case "variable":
                flamingo
            // Comments
            case "comment", "spell":
                overlay1
            // Punctuation
            case "punctuation.bracket", "punctuation.delimiter":
                surface2
            default:
                text
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
