import Foundation

/// Thread-safe service wrapping the Rust `SbqlEngine` via UniFFI.
/// All FFI calls go through this actor; callers get Swift domain types back.
actor SbqlService {
    static let shared = SbqlService()

    private let engine: SbqlEngine

    private init() {
        engine = SbqlEngine()
    }

    // MARK: - Connections

    nonisolated func getConnections() -> [Connection] {
        engine.getConnections().map(Connection.init)
    }

    func saveConnection(_ conn: Connection, password: String?) async throws -> [Connection] {
        let list = try await engine.saveConnection(config: conn.ffi, password: password)
        return list.map(Connection.init)
    }

    func deleteConnection(id: String) async throws -> [Connection] {
        let list = try await engine.deleteConnection(id: id)
        return list.map(Connection.init)
    }

    func connect(id: String) async throws {
        try await engine.connect(id: id)
    }

    func disconnect(id: String) async throws {
        try await engine.disconnect(id: id)
    }

    // MARK: - Schema

    func listTables() async throws -> [TableEntryModel] {
        let list = try await engine.listTables()
        return list.map(TableEntryModel.init)
    }

    func getPrimaryKeys(schema: String, table: String) async throws -> [String] {
        try await engine.getPrimaryKeys(schema: schema, table: table)
    }

    func loadDiagram() async throws -> DiagramModel {
        let data = try await engine.loadDiagram()
        return DiagramModel(ffi: data)
    }

    // MARK: - Query

    func executeQuery(sql: String) async throws -> QueryResultData {
        let result = try await engine.executeQuery(sql: sql)
        return QueryResultData(ffi: result)
    }

    func fetchPage(_ page: UInt32) async throws -> QueryResultData {
        let result = try await engine.fetchPage(page: page)
        return QueryResultData(ffi: result)
    }

    // MARK: - Sort / Filter

    func applyOrder(column: String, direction: FfiSortDirection) async throws -> QueryResultData {
        let result = try await engine.applyOrder(column: column, direction: direction)
        return QueryResultData(ffi: result)
    }

    func clearOrder() async throws -> QueryResultData {
        let result = try await engine.clearOrder()
        return QueryResultData(ffi: result)
    }

    func applyFilter(query: String) async throws -> QueryResultData {
        let result = try await engine.applyFilter(query: query)
        return QueryResultData(ffi: result)
    }

    func clearFilter() async throws -> QueryResultData {
        let result = try await engine.clearFilter()
        return QueryResultData(ffi: result)
    }

    func suggestFilterValues(
        column: String,
        prefix: String,
        limit: UInt32,
        token: UInt64
    ) async throws -> FfiFilterSuggestions {
        try await engine.suggestFilterValues(
            column: column,
            prefix: prefix,
            limit: limit,
            token: token
        )
    }

    // MARK: - Mutations

    func updateCell(
        schema: String,
        table: String,
        pkCol: String,
        pkVal: String,
        targetCol: String,
        newVal: String
    ) async throws {
        try await engine.updateCell(
            schema: schema,
            table: table,
            pkCol: pkCol,
            pkVal: pkVal,
            targetCol: targetCol,
            newVal: newVal
        )
    }

    func deleteRow(
        schema: String,
        table: String,
        pkCol: String,
        pkVal: String
    ) async throws {
        try await engine.deleteRow(
            schema: schema,
            table: table,
            pkCol: pkCol,
            pkVal: pkVal
        )
    }
}
