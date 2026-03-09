import Foundation

/// Key identifying a single cell by row and column index.
struct CellKey: Hashable {
    let row: Int
    let col: Int
}

/// State for the query results pane.
@Observable
final class ResultsViewModel {
    var currentResult: QueryResultData = .empty
    var sortedColumn: String?
    var sortDirection: FfiSortDirection = .ascending
    var filterText: String = ""
    var isFilterBarVisible: Bool = false

    /// The table that produced the current result (set by selectTable).
    var activeSchema: String?
    var activeTable: String?
    /// Primary key columns for the active table.
    var primaryKeys: [String] = []

    /// Cells edited locally but not yet committed to the database.
    var dirtyCells: [CellKey: String] = [:]

    /// Row indices marked for deletion but not yet committed.
    var pendingDeletions: Set<Int> = []

    /// Whether there are pending edits awaiting commit.
    var hasPendingEdits: Bool {
        !dirtyCells.isEmpty || !pendingDeletions.isEmpty
    }

    // MARK: - Tabs

    var tabs: [QueryTab] = []
    var activeTabId: String?

    /// Opens a tab for the given table, or switches to it if it already exists.
    /// Returns `true` if a new tab was created (caller should execute the query).
    @discardableResult
    func openTab(schema: String, tableName: String, sql: String, currentSql: String) -> Bool {
        let tabId = "\(schema).\(tableName)"

        if tabs.contains(where: { $0.id == tabId }) {
            switchToTab(id: tabId, currentSql: currentSql)
            return false
        }

        // Save current tab state before creating a new one
        saveCurrentTabState(sqlText: currentSql)

        let tab = QueryTab(
            id: tabId,
            schema: schema,
            tableName: tableName,
            sqlText: sql,
            result: .empty,
            sortedColumn: nil,
            sortDirection: .ascending,
            filterText: "",
            isFilterBarVisible: false,
            primaryKeys: [],
            dirtyCells: [:],
            pendingDeletions: []
        )
        tabs.append(tab)
        activeTabId = tabId

        // Apply new tab state to view
        activeSchema = schema
        activeTable = tableName
        currentResult = .empty
        sortedColumn = nil
        sortDirection = .ascending
        filterText = ""
        isFilterBarVisible = false
        primaryKeys = []
        dirtyCells = [:]
        pendingDeletions = []

        return true
    }

    /// Switches to an existing tab, saving the current state first.
    /// Returns the SQL text of the target tab so the caller can update the editor.
    @discardableResult
    func switchToTab(id: String, currentSql: String) -> String? {
        guard id != activeTabId,
              let targetIndex = tabs.firstIndex(where: { $0.id == id }) else { return nil }

        saveCurrentTabState(sqlText: currentSql)

        let tab = tabs[targetIndex]
        activeTabId = tab.id
        activeSchema = tab.schema
        activeTable = tab.tableName
        currentResult = tab.result
        sortedColumn = tab.sortedColumn
        sortDirection = tab.sortDirection
        filterText = tab.filterText
        isFilterBarVisible = tab.isFilterBarVisible
        primaryKeys = tab.primaryKeys
        dirtyCells = tab.dirtyCells
        pendingDeletions = tab.pendingDeletions

        return tab.sqlText
    }

    /// Closes a tab. If it was active, activates the nearest neighbor.
    /// Returns the SQL text of the newly active tab (or nil if no tabs remain).
    @discardableResult
    func closeTab(id: String) -> String? {
        guard let index = tabs.firstIndex(where: { $0.id == id }) else { return nil }

        let wasActive = (id == activeTabId)
        tabs.remove(at: index)

        if wasActive {
            if tabs.isEmpty {
                activeTabId = nil
                clearState()
                return nil
            } else {
                let newIndex = min(index, tabs.count - 1)
                let newTab = tabs[newIndex]
                activeTabId = newTab.id
                activeSchema = newTab.schema
                activeTable = newTab.tableName
                currentResult = newTab.result
                sortedColumn = newTab.sortedColumn
                sortDirection = newTab.sortDirection
                filterText = newTab.filterText
                isFilterBarVisible = newTab.isFilterBarVisible
                primaryKeys = newTab.primaryKeys
                dirtyCells = newTab.dirtyCells
                pendingDeletions = newTab.pendingDeletions
                return newTab.sqlText
            }
        }
        return nil
    }

    // MARK: - Tab state persistence

    /// Saves the current view state back into the active tab.
    private func saveCurrentTabState(sqlText: String) {
        guard let activeId = activeTabId,
              let index = tabs.firstIndex(where: { $0.id == activeId }) else { return }

        tabs[index].sqlText = sqlText
        tabs[index].result = currentResult
        tabs[index].sortedColumn = sortedColumn
        tabs[index].sortDirection = sortDirection
        tabs[index].filterText = filterText
        tabs[index].isFilterBarVisible = isFilterBarVisible
        tabs[index].primaryKeys = primaryKeys
        tabs[index].dirtyCells = dirtyCells
        tabs[index].pendingDeletions = pendingDeletions
    }

    // MARK: - Existing API

    func applyResult(_ result: QueryResultData) {
        currentResult = result
        dirtyCells.removeAll()
        pendingDeletions.removeAll()
        // Keep the active tab in sync
        if let activeId = activeTabId,
           let index = tabs.firstIndex(where: { $0.id == activeId })
        {
            tabs[index].result = result
            tabs[index].dirtyCells = [:]
            tabs[index].pendingDeletions = []
        }
    }

    func clear() {
        clearState()
        tabs.removeAll()
        activeTabId = nil
    }

    var pageDisplay: String {
        let page = currentResult.page + 1
        if currentResult.hasNextPage {
            return "Page \(page)"
        } else if currentResult.isEmpty {
            return "No results"
        } else {
            return "Page \(page) (last)"
        }
    }

    // MARK: - Private

    private func clearState() {
        currentResult = .empty
        sortedColumn = nil
        filterText = ""
        isFilterBarVisible = false
        activeSchema = nil
        activeTable = nil
        primaryKeys = []
        dirtyCells.removeAll()
        pendingDeletions.removeAll()
    }
}
