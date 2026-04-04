import Foundation

struct QueryHistoryEntry: Identifiable, Codable, Equatable {
    let id: UUID
    let sql: String
    let connectionName: String
    let connectionId: String
    let timestamp: Date
    let durationMs: Int64
    let rowCount: Int

    var sqlPreview: String {
        let firstLine = sql.components(separatedBy: .newlines).first ?? sql
        return firstLine.count > 60 ? String(firstLine.prefix(60)) + "…" : firstLine
    }
}
