import Foundation

@Observable
final class QueryHistoryViewModel {
    var entries: [QueryHistoryEntry] = []
    var searchText: String = ""

    private var storage: QueryStorageService { QueryStorageService.shared }

    var filteredEntries: [QueryHistoryEntry] {
        guard !searchText.isEmpty else { return entries }
        return entries.filter {
            $0.sql.localizedCaseInsensitiveContains(searchText) ||
            $0.connectionName.localizedCaseInsensitiveContains(searchText)
        }
    }

    func load() {
        entries = storage.loadHistory()
    }

    func addEntry(sql: String, connectionName: String, connectionId: String, durationMs: Int64, rowCount: Int) {
        let entry = QueryHistoryEntry(
            id: UUID(), sql: sql, connectionName: connectionName,
            connectionId: connectionId, timestamp: Date(),
            durationMs: durationMs, rowCount: rowCount
        )
        entries.insert(entry, at: 0)
        if entries.count > 500 { entries = Array(entries.prefix(500)) }
        storage.saveHistory(entries)
    }

    func clearHistory() {
        entries.removeAll()
        storage.saveHistory(entries)
    }
}
