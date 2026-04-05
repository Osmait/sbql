import Foundation
import os

final class QueryStorageService {
    static let shared = QueryStorageService()

    private let historyURL: URL
    private let queriesURL: URL
    private let maxHistory = 500

    private init() {
        let base = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/sbql")
        try? FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
        historyURL = base.appendingPathComponent("history.json")
        queriesURL = base.appendingPathComponent("queries.json")
    }

    // MARK: - History

    func loadHistory() -> [QueryHistoryEntry] {
        guard let data = try? Data(contentsOf: historyURL) else { return [] }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        do {
            return try decoder.decode([QueryHistoryEntry].self, from: data)
        } catch {
            os_log(.error, "Failed to decode history: %{public}@", error.localizedDescription)
            return []
        }
    }

    func saveHistory(_ entries: [QueryHistoryEntry]) {
        let trimmed = Array(entries.prefix(maxHistory))
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? encoder.encode(trimmed) else { return }
        do {
            try data.write(to: historyURL, options: .atomic)
        } catch {
            os_log(.error, "Failed to save history: %{public}@", error.localizedDescription)
        }
    }

    // MARK: - Saved Queries

    func loadSavedQueries() -> [SavedQuery] {
        guard let data = try? Data(contentsOf: queriesURL) else { return [] }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        do {
            return try decoder.decode([SavedQuery].self, from: data)
        } catch {
            os_log(.error, "Failed to decode saved queries: %{public}@", error.localizedDescription)
            return []
        }
    }

    func saveSavedQueries(_ queries: [SavedQuery]) {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? encoder.encode(queries) else { return }
        do {
            try data.write(to: queriesURL, options: .atomic)
        } catch {
            os_log(.error, "Failed to save queries: %{public}@", error.localizedDescription)
        }
    }
}
