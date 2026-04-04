import Foundation

@Observable
final class SavedQueriesViewModel {
    var queries: [SavedQuery] = []
    var searchText: String = ""
    var isShowingSaveSheet: Bool = false
    var saveSheetSQL: String = ""

    private let storage = QueryStorageService.shared

    var filteredQueries: [SavedQuery] {
        guard !searchText.isEmpty else { return queries }
        return queries.filter {
            $0.name.localizedCaseInsensitiveContains(searchText) ||
            $0.sql.localizedCaseInsensitiveContains(searchText)
        }
    }

    func load() {
        queries = storage.loadSavedQueries()
    }

    func save(name: String, sql: String) {
        let query = SavedQuery(id: UUID(), name: name, sql: sql, createdAt: Date(), updatedAt: Date())
        queries.insert(query, at: 0)
        storage.saveSavedQueries(queries)
    }

    func rename(id: UUID, newName: String) {
        guard let idx = queries.firstIndex(where: { $0.id == id }) else { return }
        queries[idx].name = newName
        queries[idx].updatedAt = Date()
        storage.saveSavedQueries(queries)
    }

    func delete(id: UUID) {
        queries.removeAll { $0.id == id }
        storage.saveSavedQueries(queries)
    }

    func duplicate(id: UUID) {
        guard let query = queries.first(where: { $0.id == id }) else { return }
        save(name: "\(query.name) (copy)", sql: query.sql)
    }
}
