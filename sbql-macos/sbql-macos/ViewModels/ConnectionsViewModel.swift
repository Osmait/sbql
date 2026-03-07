import Foundation

/// Manages the list of saved connections and sidebar state.
@Observable
final class ConnectionsViewModel {
    var connections: [Connection] = []
    var selectedConnectionId: String?
    var tables: [TableEntryModel] = []
    var tableFilter: String = ""
    var selectedTable: TableEntryModel?
    var isShowingConnectionForm = false
    var editingConnection: Connection?

    private let service = SbqlService.shared

    var activeConnection: Connection? {
        connections.first { $0.isConnected }
    }

    var activeConnectionName: String {
        activeConnection?.name ?? "sbql"
    }

    var filteredTables: [TableEntryModel] {
        guard !tableFilter.isEmpty else { return tables }
        return tables.filter { $0.name.localizedCaseInsensitiveContains(tableFilter) }
    }

    func loadFromDisk() {
        connections = service.getConnections()
    }

    func saveConnection(_ conn: Connection, password: String?) async throws {
        let list = try await service.saveConnection(conn, password: password)
        connections = list
    }

    func deleteConnection(id: String) async throws {
        let list = try await service.deleteConnection(id: id)
        connections = list
        if selectedConnectionId == id {
            selectedConnectionId = nil
        }
    }

    func markConnected(id: String) {
        for i in connections.indices {
            connections[i].isConnected = (connections[i].id == id)
        }
        selectedConnectionId = id
    }

    func markDisconnected(id: String) {
        for i in connections.indices where connections[i].id == id {
            connections[i].isConnected = false
        }
        tableFilter = ""
    }
}
