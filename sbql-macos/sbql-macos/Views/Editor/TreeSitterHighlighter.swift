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

        // Catppuccin Mocha palette for syntax highlighting
        let mauve = NSColor(Color(hex: 0xCBA6F7)) // keywords
        let red = NSColor(Color(hex: 0xF38BA8)) // operators, delimiters
        let peach = NSColor(Color(hex: 0xFAB387)) // numbers, booleans
        let green = NSColor(Color(hex: 0xA6E3A1)) // strings
        let yellow = NSColor(Color(hex: 0xF9E2AF)) // types, builtins
        let blue = NSColor(Color(hex: 0x89B4FA)) // functions
        let sapphire = NSColor(Color(hex: 0x74C7EC)) // fields, parameters
        let teal = NSColor(Color(hex: 0x94E2D5)) // attributes, storage
        let lavender = NSColor(Color(hex: 0xB4BEFE)) // conditionals
        let flamingo = NSColor(Color(hex: 0xF2CDCD)) // variables
        let overlay1 = NSColor(Color(hex: 0x7F849C)) // comments
        let surface2 = NSColor(Color(hex: 0x585B70)) // punctuation
        let text = NSColor(Color(hex: 0xCDD6F4)) // default

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
