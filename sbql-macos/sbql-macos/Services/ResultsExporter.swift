import AppKit
import UniformTypeIdentifiers

/// Exports query result data to CSV, JSON, or SQL INSERT formats.
enum ExportFormat: String, CaseIterable {
    case csv = "CSV"
    case json = "JSON"
    case sql = "SQL INSERT"

    var fileExtension: String {
        switch self {
        case .csv: return "csv"
        case .json: return "json"
        case .sql: return "sql"
        }
    }

    var icon: String {
        switch self {
        case .csv: return "tablecells"
        case .json: return "curlybraces"
        case .sql: return "cylinder"
        }
    }

    /// Convert to FFI enum for Rust-side streaming export.
    var ffi: FfiExportFormat {
        switch self {
        case .csv: return .csv
        case .json: return .json
        case .sql: return .sqlInsert
        }
    }
}

enum ResultsExporter {
    // MARK: - CSV

    static func toCSV(columns: [String], rows: [[String]]) -> String {
        var lines: [String] = []
        lines.append(columns.map { escapeCSV($0) }.joined(separator: ","))
        for row in rows {
            lines.append(row.map { escapeCSV($0) }.joined(separator: ","))
        }
        return lines.joined(separator: "\n")
    }

    private static func escapeCSV(_ value: String) -> String {
        if value.contains(",") || value.contains("\"") || value.contains("\n") {
            return "\"\(value.replacingOccurrences(of: "\"", with: "\"\""))\""
        }
        return value
    }

    // MARK: - JSON

    static func toJSON(columns: [String], rows: [[String]]) -> String {
        var objects: [[String: String]] = []
        for row in rows {
            var obj: [String: String] = [:]
            for (i, col) in columns.enumerated() {
                obj[col] = i < row.count ? row[i] : ""
            }
            objects.append(obj)
        }
        guard let data = try? JSONSerialization.data(
            withJSONObject: objects,
            options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        ) else { return "[]" }
        return String(data: data, encoding: .utf8) ?? "[]"
    }

    // MARK: - SQL INSERT

    static func toSQL(columns: [String], rows: [[String]], tableName: String) -> String {
        guard !rows.isEmpty else { return "-- No data to export" }

        let colList = columns.map { "\"\($0)\"" }.joined(separator: ", ")
        var lines: [String] = []
        for row in rows {
            let values = row.map { escapeSQL($0) }.joined(separator: ", ")
            lines.append("INSERT INTO \"\(tableName)\" (\(colList)) VALUES (\(values));")
        }
        return lines.joined(separator: "\n")
    }

    private static func escapeSQL(_ value: String) -> String {
        if value.isEmpty { return "NULL" }
        // Check if it looks like a number
        if Double(value) != nil { return value }
        // Check booleans
        if value == "true" || value == "false" { return value.uppercased() }
        // Otherwise quote as string
        return "'\(value.replacingOccurrences(of: "'", with: "''"))'"
    }

    // MARK: - Save to file

    static func export(
        format: ExportFormat,
        columns: [String],
        rows: [[String]],
        tableName: String
    ) {
        let content: String
        switch format {
        case .csv: content = toCSV(columns: columns, rows: rows)
        case .json: content = toJSON(columns: columns, rows: rows)
        case .sql: content = toSQL(columns: columns, rows: rows, tableName: tableName)
        }

        let panel = NSSavePanel()
        panel.title = "Export Results"
        panel.nameFieldStringValue = "\(tableName).\(format.fileExtension)"
        panel.allowedContentTypes = [UTType(filenameExtension: format.fileExtension) ?? .plainText]
        panel.canCreateDirectories = true

        guard panel.runModal() == .OK, let url = panel.url else { return }

        do {
            try content.write(to: url, atomically: true, encoding: .utf8)
        } catch {
            let alert = NSAlert(error: error)
            alert.runModal()
        }
    }
}
