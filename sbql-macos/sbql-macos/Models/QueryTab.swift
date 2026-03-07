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

    /// Display label for the tab.
    var displayName: String {
        tableName ?? "Query"
    }
}
