import Foundation

struct SavedQuery: Identifiable, Codable, Equatable {
    let id: UUID
    var name: String
    var sql: String
    let createdAt: Date
    var updatedAt: Date

    var sqlPreview: String {
        let firstLine = sql.components(separatedBy: .newlines).first ?? sql
        return firstLine.count > 60 ? String(firstLine.prefix(60)) + "…" : firstLine
    }
}
