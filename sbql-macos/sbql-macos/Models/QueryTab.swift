import Foundation

/// Captures the complete state of a single query tab.
struct QueryTab: Identifiable {
    let id: String
    let schema: String?
    let tableName: String?
    var sqlText: String
    var result: QueryResultData
    var sortedColumn: String?
    var sortDirection: FfiSortDirection
    var filterText: String
    var isFilterBarVisible: Bool
    var primaryKeys: [String]
    var dirtyCells: [CellKey: String]
    var pendingDeletions: Set<Int>

    /// Override set when the executed query targets a different table than the tab was opened for.
    var displayNameOverride: String?

    /// Display label for the tab.
    var displayName: String {
        displayNameOverride ?? tableName ?? "Query"
    }
}
