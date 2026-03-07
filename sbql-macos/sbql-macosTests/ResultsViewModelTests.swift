import Testing
@testable import sbql_macos

struct ResultsViewModelTests {

    // MARK: - hasPendingEdits

    @Test func hasPendingEditsFalseWhenClean() {
        let vm = ResultsViewModel()
        #expect(vm.hasPendingEdits == false)
    }

    @Test func hasPendingEditsTrueWithDirtyCells() {
        let vm = ResultsViewModel()
        vm.dirtyCells[CellKey(row: 0, col: 0)] = "new"
        #expect(vm.hasPendingEdits == true)
    }

    @Test func hasPendingEditsTrueWithPendingDeletions() {
        let vm = ResultsViewModel()
        vm.pendingDeletions.insert(0)
        #expect(vm.hasPendingEdits == true)
    }

    // MARK: - pageDisplay

    @Test func pageDisplayEmpty() {
        let vm = ResultsViewModel()
        vm.currentResult = .empty
        #expect(vm.pageDisplay == "No results")
    }

    @Test func pageDisplayHasNextPage() {
        let vm = ResultsViewModel()
        vm.currentResult = QueryResultData(
            columns: ["id"],
            rows: [["1"]],
            page: 0,
            hasNextPage: true
        )
        #expect(vm.pageDisplay == "Page 1")
    }

    @Test func pageDisplayLastPage() {
        let vm = ResultsViewModel()
        vm.currentResult = QueryResultData(
            columns: ["id"],
            rows: [["1"]],
            page: 2,
            hasNextPage: false
        )
        #expect(vm.pageDisplay == "Page 3 (last)")
    }

    // MARK: - openTab

    @Test func openTabCreatesNewTab() {
        let vm = ResultsViewModel()
        let isNew = vm.openTab(
            schema: "public",
            tableName: "users",
            sql: "SELECT * FROM users",
            currentSql: ""
        )
        #expect(isNew == true)
        #expect(vm.tabs.count == 1)
        #expect(vm.activeTabId == "public.users")
        #expect(vm.activeSchema == "public")
        #expect(vm.activeTable == "users")
    }

    @Test func openTabSwitchesToExisting() {
        let vm = ResultsViewModel()
        vm.openTab(schema: "public", tableName: "users", sql: "SELECT * FROM users", currentSql: "")
        let isNew = vm.openTab(schema: "public", tableName: "users", sql: "SELECT * FROM users", currentSql: "")
        #expect(isNew == false)
        #expect(vm.tabs.count == 1)
    }

    // MARK: - switchToTab

    @Test func switchToTabSavesAndRestores() {
        let vm = ResultsViewModel()
        vm.openTab(schema: "s", tableName: "t1", sql: "SQL1", currentSql: "")
        vm.currentResult = QueryResultData(columns: ["a"], rows: [["1"]], page: 0, hasNextPage: false)

        vm.openTab(schema: "s", tableName: "t2", sql: "SQL2", currentSql: "SQL1")
        #expect(vm.activeTabId == "s.t2")

        let sql = vm.switchToTab(id: "s.t1", currentSql: "SQL2")
        #expect(sql == "SQL1")
        #expect(vm.activeTabId == "s.t1")
        #expect(vm.currentResult.columns == ["a"])
    }

    // MARK: - closeTab

    @Test func closeTabNonActive() {
        let vm = ResultsViewModel()
        vm.openTab(schema: "s", tableName: "t1", sql: "SQL1", currentSql: "")
        vm.openTab(schema: "s", tableName: "t2", sql: "SQL2", currentSql: "SQL1")
        #expect(vm.activeTabId == "s.t2")

        let sql = vm.closeTab(id: "s.t1")
        #expect(sql == nil) // non-active tab closed, no switch needed
        #expect(vm.tabs.count == 1)
        #expect(vm.activeTabId == "s.t2")
    }

    @Test func closeTabActiveWithOthers() {
        let vm = ResultsViewModel()
        vm.openTab(schema: "s", tableName: "t1", sql: "SQL1", currentSql: "")
        vm.openTab(schema: "s", tableName: "t2", sql: "SQL2", currentSql: "SQL1")

        let sql = vm.closeTab(id: "s.t2")
        #expect(sql != nil) // should switch to remaining tab
        #expect(vm.tabs.count == 1)
        #expect(vm.activeTabId == "s.t1")
    }

    @Test func closeTabLastTab() {
        let vm = ResultsViewModel()
        vm.openTab(schema: "s", tableName: "t1", sql: "SQL1", currentSql: "")

        let sql = vm.closeTab(id: "s.t1")
        #expect(sql == nil)
        #expect(vm.tabs.isEmpty)
        #expect(vm.activeTabId == nil)
    }

    // MARK: - applyResult

    @Test func applyResultClearsDirtyState() {
        let vm = ResultsViewModel()
        vm.dirtyCells[CellKey(row: 0, col: 0)] = "dirty"
        vm.pendingDeletions.insert(1)

        let result = QueryResultData(columns: ["id"], rows: [["1"]], page: 0, hasNextPage: false)
        vm.applyResult(result)

        #expect(vm.dirtyCells.isEmpty)
        #expect(vm.pendingDeletions.isEmpty)
        #expect(vm.currentResult == result)
    }

    // MARK: - clear

    @Test func clearResetsEverything() {
        let vm = ResultsViewModel()
        vm.openTab(schema: "s", tableName: "t", sql: "SQL", currentSql: "")
        vm.currentResult = QueryResultData(columns: ["a"], rows: [["1"]], page: 0, hasNextPage: false)
        vm.sortedColumn = "a"
        vm.filterText = "filter"

        vm.clear()

        #expect(vm.tabs.isEmpty)
        #expect(vm.activeTabId == nil)
        #expect(vm.currentResult == .empty)
        #expect(vm.sortedColumn == nil)
        #expect(vm.filterText == "")
    }
}
