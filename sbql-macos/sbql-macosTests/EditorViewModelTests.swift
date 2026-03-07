import Testing
@testable import sbql_macos

struct EditorViewModelTests {

    @Test func initialState() {
        let vm = EditorViewModel()
        #expect(vm.sqlText == "")
        #expect(vm.isExecuting == false)
        #expect(vm.isVisible == false)
        #expect(vm.lastQueryDuration == nil)
    }

    @Test func stateMutation() {
        let vm = EditorViewModel()
        vm.sqlText = "SELECT 1"
        vm.isExecuting = true
        vm.isVisible = true

        #expect(vm.sqlText == "SELECT 1")
        #expect(vm.isExecuting == true)
        #expect(vm.isVisible == true)
    }
}
