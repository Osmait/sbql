import Testing
@testable import sbql_macos

struct ConnectionsViewModelTests {

    private func makeConnection(
        id: String = "test-id",
        name: String = "Test",
        isConnected: Bool = false
    ) -> Connection {
        Connection(
            id: id,
            name: name,
            backend: .postgres,
            host: "localhost",
            port: 5432,
            user: "user",
            database: "db",
            sslMode: .prefer,
            isConnected: isConnected
        )
    }

    @Test func activeConnectionNilWhenEmpty() {
        let vm = ConnectionsViewModel()
        #expect(vm.activeConnection == nil)
    }

    @Test func activeConnectionReturnsConnectedItem() {
        let vm = ConnectionsViewModel()
        vm.connections = [
            makeConnection(id: "a", name: "A", isConnected: false),
            makeConnection(id: "b", name: "B", isConnected: true),
        ]
        #expect(vm.activeConnection?.id == "b")
    }

    @Test func activeConnectionNameDefaultsToSbql() {
        let vm = ConnectionsViewModel()
        #expect(vm.activeConnectionName == "sbql")
    }

    @Test func activeConnectionNameReturnsConnectedName() {
        let vm = ConnectionsViewModel()
        vm.connections = [
            makeConnection(id: "a", name: "MyDB", isConnected: true),
        ]
        #expect(vm.activeConnectionName == "MyDB")
    }

    @Test func filteredTablesReturnsAllWhenFilterEmpty() {
        let vm = ConnectionsViewModel()
        vm.tables = [
            TableEntryModel(schema: "public", name: "users"),
            TableEntryModel(schema: "public", name: "orders"),
        ]
        vm.tableFilter = ""
        #expect(vm.filteredTables.count == 2)
    }

    @Test func filteredTablesCaseInsensitive() {
        let vm = ConnectionsViewModel()
        vm.tables = [
            TableEntryModel(schema: "public", name: "Users"),
            TableEntryModel(schema: "public", name: "orders"),
        ]
        vm.tableFilter = "user"
        #expect(vm.filteredTables.count == 1)
        #expect(vm.filteredTables.first?.name == "Users")
    }

    @Test func markConnectedSetsFlags() {
        let vm = ConnectionsViewModel()
        vm.connections = [
            makeConnection(id: "a", name: "A"),
            makeConnection(id: "b", name: "B"),
        ]
        vm.markConnected(id: "b")
        #expect(vm.connections[0].isConnected == false)
        #expect(vm.connections[1].isConnected == true)
        #expect(vm.selectedConnectionId == "b")
    }

    @Test func markDisconnectedClearsState() {
        let vm = ConnectionsViewModel()
        vm.connections = [
            makeConnection(id: "a", name: "A", isConnected: true),
        ]
        vm.tableFilter = "some filter"
        vm.markDisconnected(id: "a")
        #expect(vm.connections[0].isConnected == false)
        #expect(vm.tableFilter == "")
    }
}
