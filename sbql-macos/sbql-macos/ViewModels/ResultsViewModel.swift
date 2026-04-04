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

    /// Snapshot for data diff comparison.
    var snapshot: QueryResultData?
    var diffResult: DiffResult?
    var isDiffMode: Bool = false

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

    /// Closes all tabs and resets state.
    func closeAllTabs() {
        tabs.removeAll()
        activeTabId = nil
        clearState()
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
        // Preserve totalCount from page 0 when navigating to subsequent pages
        if result.totalCount == nil, let existingTotal = currentResult.totalCount {
            currentResult = QueryResultData(
                columns: result.columns,
                rows: result.rows,
                page: result.page,
                hasNextPage: result.hasNextPage,
                totalCount: existingTotal
            )
        } else {
            currentResult = result
        }
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

    /// Updates the active tab's display name based on the table referenced in the SQL.
    func updateActiveTabName(forSQL sql: String) {
        guard let activeId = activeTabId,
              let index = tabs.firstIndex(where: { $0.id == activeId }) else { return }

        // Match FROM schema.table or FROM table (handles qualified names)
        let pattern = #"(?i)\bFROM\s+([\w]+(?:\.[\w]+)?)"#
        guard let range = sql.range(of: pattern, options: .regularExpression) else { return }
        let matched = String(sql[range])
        // Extract everything after FROM
        let parts = matched.split(separator: " ", maxSplits: 1)
        guard parts.count == 2 else { return }
        let qualified = String(parts[1]).trimmingCharacters(in: .whitespaces)
        // Take just the table name (last component after dot)
        let extractedName = qualified.contains(".")
            ? String(qualified.split(separator: ".").last ?? Substring(qualified))
            : qualified

        if extractedName.lowercased() != (tabs[index].tableName ?? "").lowercased() {
            tabs[index].displayNameOverride = extractedName
        } else {
            tabs[index].displayNameOverride = nil
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

    // MARK: - Snapshot & Diff

    func takeSnapshot() {
        snapshot = currentResult
    }

    func computeDiff() {
        guard let snapshot else { return }
        let current = currentResult
        var added = Set<Int>()
        var removed = [[String]]()
        var changed = [CellKey: (old: String, new: String)]()

        // PK-based diff if available
        if let pkCol = primaryKeys.first,
           let pkIdxSnap = snapshot.columns.firstIndex(of: pkCol),
           let pkIdxCurr = current.columns.firstIndex(of: pkCol) {
            let snapByPK = Dictionary(uniqueKeysWithValues: snapshot.rows.enumerated().map { ($1[pkIdxSnap], ($0, $1)) })
            let currByPK = Dictionary(uniqueKeysWithValues: current.rows.enumerated().map { ($1[pkIdxCurr], ($0, $1)) })

            for (pk, (rowIdx, row)) in currByPK {
                if let (_, oldRow) = snapByPK[pk] {
                    for (colIdx, col) in current.columns.enumerated() {
                        if let snapColIdx = snapshot.columns.firstIndex(of: col),
                           oldRow[snapColIdx] != row[colIdx] {
                            changed[CellKey(row: rowIdx, col: colIdx)] = (old: oldRow[snapColIdx], new: row[colIdx])
                        }
                    }
                } else { added.insert(rowIdx) }
            }
            for (pk, (_, row)) in snapByPK where currByPK[pk] == nil { removed.append(row) }
        } else {
            // Index-based fallback
            let minRows = min(snapshot.rows.count, current.rows.count)
            for rowIdx in 0..<minRows {
                for colIdx in 0..<min(snapshot.columns.count, current.columns.count) {
                    if snapshot.rows[rowIdx][colIdx] != current.rows[rowIdx][colIdx] {
                        changed[CellKey(row: rowIdx, col: colIdx)] = (old: snapshot.rows[rowIdx][colIdx], new: current.rows[rowIdx][colIdx])
                    }
                }
            }
            for rowIdx in minRows..<current.rows.count { added.insert(rowIdx) }
            for rowIdx in minRows..<snapshot.rows.count { removed.append(snapshot.rows[rowIdx]) }
        }

        diffResult = DiffResult(addedRows: added, removedRows: removed, changedCells: changed)
        isDiffMode = true
    }

    func clearDiff() { diffResult = nil; isDiffMode = false }

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
