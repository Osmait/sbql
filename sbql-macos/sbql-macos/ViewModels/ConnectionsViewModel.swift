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
    var connectionFilter: String = ""

    private var service: SbqlService { SbqlService.shared }

    var activeConnection: Connection? {
        connections.first { $0.isConnected }
    }

    /// Connections filtered by search query.
    var filteredConnections: [Connection] {
        guard !connectionFilter.isEmpty else { return connections }
        return connections.filter {
            $0.name.localizedCaseInsensitiveContains(connectionFilter) ||
            $0.host.localizedCaseInsensitiveContains(connectionFilter) ||
            $0.database.localizedCaseInsensitiveContains(connectionFilter)
        }
    }

    /// Connections grouped by backend, filtered by search.
    var groupedConnections: [(backend: Connection.Backend, connections: [Connection])] {
        let filtered = filteredConnections
        let order: [Connection.Backend] = [.postgres, .mysql, .sqlserver, .sqlite, .mongodb, .redis, .dynamodb]
        return order.compactMap { backend in
            let group = filtered.filter { $0.backend == backend }
            return group.isEmpty ? nil : (backend: backend, connections: group)
        }
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

    func saveConnection(_ conn: Connection, password: String?, sshPassword: String? = nil) async throws {
        let list = try await service.saveConnection(conn, password: password, sshPassword: sshPassword)
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
