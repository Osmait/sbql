import Testing
@testable import sbql_macos

struct AppViewModelTests {

    @Test func initialState() {
        let vm = AppViewModel()
        #expect(vm.activeTab == .query)
        #expect(vm.toastMessage == nil)
        #expect(vm.toastIsError == false)
        // Sub-ViewModels should be initialized
        #expect(vm.results.currentResult.isEmpty)
        #expect(vm.editor.sqlText == "")
        #expect(vm.diagram.diagramData.tables.isEmpty)
        #expect(vm.connections.connections.isEmpty)
    }

    @Test func discardEdits_clearsState() {
        let vm = AppViewModel()
        vm.results.dirtyCells = [CellKey(row: 0, col: 1): "new"]
        vm.results.pendingDeletions = [0, 2]

        vm.discardEdits()

        #expect(vm.results.dirtyCells.isEmpty)
        #expect(vm.results.pendingDeletions.isEmpty)
    }

    @Test func showToast_setsMessage() {
        let vm = AppViewModel()
        vm.showToast("Connected")

        #expect(vm.toastMessage == "Connected")
        #expect(vm.toastIsError == false)
    }

    @Test func showError_setsErrorMessage() {
        let vm = AppViewModel()
        let error = SbqlFfiError.Core(msg: "connection refused")
        vm.showError(error)

        #expect(vm.toastMessage != nil)
        #expect(vm.toastIsError == true)
    }

    @Test func activeTab_enumCases() {
        #expect(AppViewModel.ActiveTab.query.rawValue == "Query")
        #expect(AppViewModel.ActiveTab.diagram.rawValue == "Diagram")
        #expect(AppViewModel.ActiveTab.allCases.count == 2)
    }
}
