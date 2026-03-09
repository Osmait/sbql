import AppKit
import SwiftUI

/// Applies SQL syntax highlighting to an NSTextStorage via its delegate callback.
final class SQLSyntaxHighlighter: NSObject, NSTextStorageDelegate {
    private let font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
    private let defaultColor = NSColor(SbqlTheme.Colors.textPrimary)
    private let keywordColor = NSColor(SbqlTheme.Colors.accent)
    private let stringColor = NSColor(SbqlTheme.Colors.success)
    private let numberColor = NSColor(SbqlTheme.Colors.warning)
    private let commentColor = NSColor(SbqlTheme.Colors.textTertiary)
    private let functionColor = NSColor(SbqlTheme.Colors.accentHover)

    // MARK: - Token sets

    private static let keywords: Set<String> = [
        "ABORT", "ADD", "ALL", "ALTER", "ANALYZE", "AND", "AS", "ASC",
        "AUTOINCREMENT", "BEGIN", "BETWEEN", "BY", "CASCADE", "CASE",
        "CHECK", "COLLATE", "COLUMN", "COMMIT", "CONFLICT", "CONSTRAINT",
        "CREATE", "CROSS", "CURRENT", "DATABASE", "DEFAULT", "DELETE",
        "DESC", "DISTINCT", "DO", "DROP", "ELSE", "END", "EXCEPT",
        "EXISTS", "EXPLAIN", "FALSE", "FILTER", "FIRST", "FOLLOWING",
        "FOR", "FOREIGN", "FROM", "FULL", "GLOB", "GROUP", "HAVING",
        "IF", "IN", "INDEX", "INNER", "INSERT", "INTERSECT", "INTO",
        "IS", "ISNULL", "JOIN", "KEY", "LAST", "LEFT", "LIKE", "LIMIT",
        "NATURAL", "NO", "NOCASE", "NOT", "NOTHING", "NOTNULL", "NULL",
        "NULLS", "OFFSET", "ON", "OR", "ORDER", "OUTER", "OVER",
        "PARTITION", "PRAGMA", "PRECEDING", "PRIMARY", "RANGE",
        "RECURSIVE", "REFERENCES", "RENAME", "REPLACE", "RESTRICT",
        "RETURNING", "RIGHT", "ROLLBACK", "ROW", "ROWS", "SELECT",
        "SET", "TABLE", "TEMP", "TEMPORARY", "THEN", "TO", "TRIGGER",
        "TRUE", "UNBOUNDED", "UNION", "UNIQUE", "UPDATE", "USING",
        "VACUUM", "VALUES", "VIEW", "WHEN", "WHERE", "WINDOW", "WITH",
    ]

    private static let functions: Set<String> = [
        "ABS", "ARRAY_AGG", "AVG", "BOOL_AND", "BOOL_OR", "CAST",
        "COALESCE", "COUNT", "CURRENT_DATE", "CURRENT_TIME",
        "CURRENT_TIMESTAMP", "DATE", "DATETIME", "DENSE_RANK",
        "FIRST_VALUE", "GENERATE_SERIES", "GROUP_CONCAT", "HEX",
        "IFNULL", "INSTR", "JSON", "JSON_ARRAY", "JSON_EXTRACT",
        "JSON_OBJECT", "LAG", "LAST_VALUE", "LEAD", "LENGTH", "LOWER",
        "LTRIM", "MAX", "MIN", "NOW", "NTH_VALUE", "NTILE", "NULLIF",
        "QUOTE", "RANDOM", "RANK", "ROUND", "ROW_NUMBER", "RTRIM",
        "STRING_AGG", "STRFTIME", "SUBSTR", "SUBSTRING", "SUM", "TIME",
        "TO_CHAR", "TO_DATE", "TO_NUMBER", "TO_TIMESTAMP", "TOTAL",
        "TRIM", "TYPEOF", "UNNEST", "UPPER", "ZEROBLOB",
    ]

    // MARK: - Regex

    private let tokenPattern: NSRegularExpression

    override init() {
        // Order matters: comments and strings must match before identifiers.
        let patterns = [
            "--[^\n]*", // single-line comment
            "/\\*[\\s\\S]*?\\*/", // multi-line comment
            "'(?:''|[^'])*'", // single-quoted string
            "\"(?:\"\"|[^\"])*\"", // double-quoted identifier
            "\\b\\d+(?:\\.\\d+)?\\b", // number
            "\\b[A-Za-z_][A-Za-z0-9_]*\\b", // identifier / keyword
        ]
        // swiftlint:disable:next force_try
        tokenPattern = try! NSRegularExpression(
            pattern: patterns.joined(separator: "|"),
            options: []
        )
        super.init()
    }

    // MARK: - NSTextStorageDelegate

    func textStorage(
        _ textStorage: NSTextStorage,
        didProcessEditing editedMask: NSTextStorageEditActions,
        range _: NSRange,
        changeInLength _: Int
    ) {
        guard editedMask.contains(.editedCharacters) else { return }
        highlightAll(textStorage)
    }

    /// Re-apply syntax colours across the full text storage.
    func highlightAll(_ textStorage: NSTextStorage) {
        let text = textStorage.string
        let fullRange = NSRange(location: 0, length: (text as NSString).length)
        guard fullRange.length > 0 else { return }

        textStorage.addAttributes(
            [.font: font, .foregroundColor: defaultColor],
            range: fullRange
        )

        tokenPattern.enumerateMatches(in: text, range: fullRange) { match, _, _ in
            guard let range = match?.range else { return }
            let token = (text as NSString).substring(with: range)
            if let color = self.color(for: token) {
                textStorage.addAttribute(.foregroundColor, value: color, range: range)
            }
        }
    }

    // MARK: - Private

    private func color(for token: String) -> NSColor? {
        if token.hasPrefix("--") || token.hasPrefix("/*") {
            return commentColor
        }
        if token.hasPrefix("'") {
            return stringColor
        }
        if token.hasPrefix("\"") {
            return stringColor
        }
        if let first = token.first, first.isNumber {
            return numberColor
        }
        let upper = token.uppercased()
        if Self.keywords.contains(upper) {
            return keywordColor
        }
        if Self.functions.contains(upper) {
            return functionColor
        }
        return nil
    }
}
