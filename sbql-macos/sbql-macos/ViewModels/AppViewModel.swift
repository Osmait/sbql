import SwiftUI

/// Root observable that owns all sub-ViewModels and coordinates app state.
@Observable
final class AppViewModel {
    let connections = ConnectionsViewModel()
    let editor      = EditorViewModel()
    let results     = ResultsViewModel()
    let diagram     = DiagramViewModel()

    var activeTab: ActiveTab = .query
    var toastMessage: String?
    var toastIsError: Bool = false

    private let service = SbqlService.shared

    enum ActiveTab: String, CaseIterable {
        case query   = "Query"
        case diagram = "Diagram"
    }

    private static let lastConnectionKey = "lastConnectionId"

    // MARK: - Lifecycle

    func onAppear() {
        connections.loadFromDisk()

        // Auto-connect to the last used connection
        if let lastId = UserDefaults.standard.string(forKey: Self.lastConnectionKey),
           connections.connections.contains(where: { $0.id == lastId }) {
            Task { await connect(id: lastId) }
        }
    }

    // MARK: - Connection flow

    func connect(id: String) async {
        do {
            try await service.connect(id: id)
            connections.markConnected(id: id)
            UserDefaults.standard.set(id, forKey: Self.lastConnectionKey)
            showToast("Connected")
        } catch {
            showError(error)
            return
        }

        // Load tables separately so a connect success is always reported.
        await refreshTables()

        // Load schema metadata for autocomplete
        Task { await loadDiagram() }
    }

    func refreshTables() async {
        do {
            let tables = try await service.listTables()
            connections.tables = tables
        } catch {
            connections.tables = []
            showError(error)
        }
    }

    func disconnect(id: String) async {
        do {
            try await service.disconnect(id: id)
            connections.markDisconnected(id: id)
            connections.tables = []
            results.clear()
            editor.lastQueryDuration = nil
            UserDefaults.standard.removeObject(forKey: Self.lastConnectionKey)
            showToast("Disconnected")
        } catch {
            showError(error)
        }
    }

    // MARK: - Table selection

    func selectTable(_ table: TableEntryModel) async {
        let sql = "SELECT * FROM \(table.qualified)"
        let isNew = results.openTab(
            schema: table.schema,
            tableName: table.name,
            sql: sql,
            currentSql: editor.sqlText
        )
        editor.sqlText = sql

        if isNew {
            await runQuery()

            // Fetch PKs for cell editing (non-blocking)
            do {
                let pks = try await service.getPrimaryKeys(schema: table.schema, table: table.name)
                results.primaryKeys = pks
                // Sync PKs to the tab
                if let idx = results.tabs.firstIndex(where: { $0.id == table.qualified }) {
                    results.tabs[idx].primaryKeys = pks
                }
            } catch {
                results.primaryKeys = []
            }
        }
    }

    // MARK: - Query flow

    func runQuery() async {
        let sql = editor.sqlText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !sql.isEmpty else { return }

        editor.isExecuting = true
        let start = ContinuousClock.now
        do {
            let result = try await service.executeQuery(sql: sql)
            editor.lastQueryDuration = ContinuousClock.now - start
            results.applyResult(result)
        } catch {
            editor.lastQueryDuration = ContinuousClock.now - start
            showError(error)
        }
        editor.isExecuting = false
    }

    func fetchPage(_ page: UInt32) async {
        do {
            let result = try await service.fetchPage(page)
            results.applyResult(result)
        } catch {
            showError(error)
        }
    }

    // MARK: - Sort

    func applyOrder(column: String, direction: FfiSortDirection) async {
        do {
            let result = try await service.applyOrder(column: column, direction: direction)
            results.applyResult(result)
        } catch {
            showError(error)
        }
    }

    func clearOrder() async {
        do {
            let result = try await service.clearOrder()
            results.applyResult(result)
        } catch {
            showError(error)
        }
    }

    // MARK: - Filter

    func applyFilter(query: String) async {
        do {
            let result = try await service.applyFilter(query: query)
            results.applyResult(result)
        } catch {
            showError(error)
        }
    }

    func clearFilter() async {
        do {
            let result = try await service.clearFilter()
            results.applyResult(result)
        } catch {
            showError(error)
        }
    }

    func suggestFilterValues(column: String, prefix: String) async -> [String] {
        do {
            let result = try await service.suggestFilterValues(
                column: column, prefix: prefix, limit: 10, token: 0
            )
            return result.items
        } catch {
            return []
        }
    }

    // MARK: - Dirty-cell commit / discard

    func commitEdits() async {
        guard let schema = results.activeSchema,
              let table = results.activeTable,
              let pkCol = results.primaryKeys.first else { return }

        let result = results.currentResult
        guard let pkIdx = result.columns.firstIndex(of: pkCol) else { return }

        let deletions = results.pendingDeletions
        let edits = results.dirtyCells

        // Execute deletes first
        for row in deletions {
            guard row < result.rows.count else { continue }
            let pkVal = result.rows[row][pkIdx]
            do {
                try await service.deleteRow(
                    schema: schema, table: table,
                    pkCol: pkCol, pkVal: pkVal
                )
            } catch {
                showError(error)
            }
        }

        // Execute updates (skip cells belonging to deleted rows)
        for (key, newVal) in edits {
            guard !deletions.contains(key.row) else { continue }
            let pkVal = result.rows[key.row][pkIdx]
            let targetCol = result.columns[key.col]
            await updateCell(schema: schema, table: table,
                             pkCol: pkCol, pkVal: pkVal,
                             targetCol: targetCol, newVal: newVal)
        }

        results.dirtyCells.removeAll()
        results.pendingDeletions.removeAll()

        let count = deletions.count
        if count > 0 {
            showToast("\(count) row\(count == 1 ? "" : "s") deleted")
        }

        await fetchPage(results.currentResult.page)
    }

    func discardEdits() {
        results.dirtyCells.removeAll()
        results.pendingDeletions.removeAll()
    }

    // MARK: - Mutations

    func updateCell(
        schema: String, table: String,
        pkCol: String, pkVal: String,
        targetCol: String, newVal: String
    ) async {
        do {
            try await service.updateCell(
                schema: schema, table: table,
                pkCol: pkCol, pkVal: pkVal,
                targetCol: targetCol, newVal: newVal
            )
            showToast("Cell updated")
        } catch {
            showError(error)
        }
    }

    func deleteRow(
        schema: String, table: String,
        pkCol: String, pkVal: String
    ) async {
        do {
            try await service.deleteRow(
                schema: schema, table: table,
                pkCol: pkCol, pkVal: pkVal
            )
            showToast("Row deleted")
            // Re-fetch current page
            await fetchPage(results.currentResult.page)
        } catch {
            showError(error)
        }
    }

    // MARK: - Diagram

    func loadDiagram() async {
        diagram.isLoading = true
        do {
            let data = try await service.loadDiagram()
            diagram.diagramData = data
        } catch {
            showError(error)
        }
        diagram.isLoading = false
    }

    // MARK: - Toast

    func showToast(_ message: String) {
        toastIsError = false
        toastMessage = message
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(2.5))
            if self.toastMessage == message {
                self.toastMessage = nil
            }
        }
    }

    func showError(_ error: Error) {
        toastIsError = true
        toastMessage = error.localizedDescription
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(4))
            if self.toastMessage == error.localizedDescription {
                self.toastMessage = nil
            }
        }
    }
}
