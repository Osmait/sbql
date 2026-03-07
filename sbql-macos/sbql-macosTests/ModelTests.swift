import Testing
@testable import sbql_macos

struct ModelTests {

    // MARK: - Connection

    @Test func connectionNewPostgresDefaults() {
        let conn = Connection.newPostgres()
        #expect(conn.backend == .postgres)
        #expect(conn.host == "localhost")
        #expect(conn.port == 5432)
        #expect(conn.user == "postgres")
        #expect(conn.database == "postgres")
        #expect(conn.sslMode == .prefer)
        #expect(conn.isConnected == false)
        #expect(!conn.id.isEmpty)
    }

    @Test func connectionNewSqliteDefaults() {
        let conn = Connection.newSqlite()
        #expect(conn.backend == .sqlite)
        #expect(conn.host == "")
        #expect(conn.port == 0)
        #expect(conn.filePath == "")
        #expect(conn.isConnected == false)
    }

    @Test func connectionDisplaySubtitlePostgres() {
        let conn = Connection(
            id: "1",
            name: "Test",
            backend: .postgres,
            host: "myhost",
            port: 5432,
            user: "admin",
            database: "mydb",
            sslMode: .prefer
        )
        #expect(conn.displaySubtitle == "admin@myhost:5432/mydb")
    }

    @Test func connectionDisplaySubtitleSqliteWithPath() {
        let conn = Connection(
            id: "1",
            name: "SQLite",
            backend: .sqlite,
            host: "",
            port: 0,
            user: "",
            database: "",
            sslMode: .prefer,
            filePath: "/tmp/test.db"
        )
        #expect(conn.displaySubtitle == "/tmp/test.db")
    }

    @Test func connectionDisplaySubtitleSqliteInMemory() {
        let conn = Connection(
            id: "1",
            name: "SQLite",
            backend: .sqlite,
            host: "",
            port: 0,
            user: "",
            database: "",
            sslMode: .prefer,
            filePath: nil
        )
        #expect(conn.displaySubtitle == "In-memory")
    }

    // MARK: - QueryResultData

    @Test func queryResultDataEmptyProperties() {
        let empty = QueryResultData.empty
        #expect(empty.isEmpty == true)
        #expect(empty.rowCount == 0)
        #expect(empty.columnCount == 0)
        #expect(empty.page == 0)
        #expect(empty.hasNextPage == false)
    }

    // MARK: - QueryTab

    @Test func queryTabDisplayNameWithTableName() {
        let tab = QueryTab(
            id: "public.users",
            schema: "public",
            tableName: "users",
            sqlText: "SELECT * FROM users",
            result: .empty,
            sortedColumn: nil,
            sortDirection: .ascending,
            filterText: "",
            isFilterBarVisible: false,
            primaryKeys: [],
            dirtyCells: [:],
            pendingDeletions: []
        )
        #expect(tab.displayName == "users")
    }

    @Test func queryTabDisplayNameWithoutTableName() {
        let tab = QueryTab(
            id: "query-1",
            schema: nil,
            tableName: nil,
            sqlText: "SELECT 1",
            result: .empty,
            sortedColumn: nil,
            sortDirection: .ascending,
            filterText: "",
            isFilterBarVisible: false,
            primaryKeys: [],
            dirtyCells: [:],
            pendingDeletions: []
        )
        #expect(tab.displayName == "Query")
    }

    // MARK: - Connection FFI Conversions

    @Test func connectionFromFfi_allFields() {
        let ffi = FfiConnectionConfig(
            id: "abc-123",
            name: "My DB",
            backend: .postgres,
            host: "db.example.com",
            port: 5433,
            user: "admin",
            database: "production",
            sslMode: .require,
            filePath: nil
        )
        let conn = Connection(ffi: ffi)
        #expect(conn.id == "abc-123")
        #expect(conn.name == "My DB")
        #expect(conn.backend == .postgres)
        #expect(conn.host == "db.example.com")
        #expect(conn.port == 5433)
        #expect(conn.user == "admin")
        #expect(conn.database == "production")
        #expect(conn.sslMode == .require)
        #expect(conn.filePath == nil)
    }

    @Test func connectionToFfi_roundTrip() {
        let conn = Connection(
            id: "rt-1",
            name: "Round Trip",
            backend: .sqlite,
            host: "",
            port: 0,
            user: "",
            database: "",
            sslMode: .disable,
            filePath: "/tmp/test.db"
        )
        let ffi = conn.ffi
        #expect(ffi.id == "rt-1")
        #expect(ffi.name == "Round Trip")
        #expect(ffi.backend == .sqlite)
        #expect(ffi.host == "")
        #expect(ffi.port == 0)
        #expect(ffi.sslMode == .disable)
        #expect(ffi.filePath == "/tmp/test.db")
    }

    @Test func backendFfiRoundTrip() {
        for backend in Connection.Backend.allCases {
            let roundTripped = Connection.Backend(ffi: backend.ffi)
            #expect(roundTripped == backend)
        }
    }

    @Test func sslModeFfiRoundTrip_allVariants() {
        for mode in Connection.SSLMode.allCases {
            let roundTripped = Connection.SSLMode(ffi: mode.ffi)
            #expect(roundTripped == mode)
        }
    }

    // MARK: - QueryResultData FFI

    @Test func queryResultDataFromFfi() {
        let ffi = FfiQueryResult(
            columns: ["id", "name", "email"],
            rows: [["1", "Alice", "a@b.com"], ["2", "Bob", "b@c.com"]],
            page: 3,
            hasNextPage: true
        )
        let result = QueryResultData(ffi: ffi)
        #expect(result.columns == ["id", "name", "email"])
        #expect(result.rows.count == 2)
        #expect(result.rows[0] == ["1", "Alice", "a@b.com"])
        #expect(result.page == 3)
        #expect(result.hasNextPage == true)
    }

    // MARK: - TableEntryModel FFI

    @Test func tableEntryModelFromFfi() {
        let ffi = FfiTableEntry(schema: "public", name: "users")
        let entry = TableEntryModel(ffi: ffi)
        #expect(entry.schema == "public")
        #expect(entry.name == "users")
    }

    @Test func tableEntryModelQualified() {
        let entry = TableEntryModel(schema: "myschema", name: "accounts")
        #expect(entry.qualified == "myschema.accounts")
        #expect(entry.id == "myschema.accounts")
    }

    // MARK: - DiagramModel FFI

    @Test func diagramModelFromFfi_marksFkColumns() {
        let ffiData = FfiDiagramData(
            tables: [
                FfiTableSchema(
                    schema: "public", name: "users",
                    columns: [
                        FfiColumnInfo(name: "id", dataType: "int4", isPk: true, isNullable: false),
                        FfiColumnInfo(name: "email", dataType: "text", isPk: false, isNullable: true),
                    ]
                ),
                FfiTableSchema(
                    schema: "public", name: "orders",
                    columns: [
                        FfiColumnInfo(name: "id", dataType: "int4", isPk: true, isNullable: false),
                        FfiColumnInfo(name: "user_id", dataType: "int4", isPk: false, isNullable: false),
                    ]
                ),
            ],
            foreignKeys: [
                FfiForeignKey(
                    fromSchema: "public", fromTable: "orders", fromCol: "user_id",
                    toSchema: "public", toTable: "users", toCol: "id",
                    constraintName: "fk_orders_user"
                ),
            ]
        )
        let model = DiagramModel(ffi: ffiData)

        // orders.user_id should be FK
        let orders = model.tables.first { $0.name == "orders" }!
        let userIdCol = orders.columns.first { $0.name == "user_id" }!
        #expect(userIdCol.isFk == true)

        // users.id should be FK (referenced side)
        let users = model.tables.first { $0.name == "users" }!
        let idCol = users.columns.first { $0.name == "id" }!
        #expect(idCol.isFk == true)
    }

    @Test func diagramModelFromFfi_nonFkColumnsUnmarked() {
        let ffiData = FfiDiagramData(
            tables: [
                FfiTableSchema(
                    schema: "public", name: "users",
                    columns: [
                        FfiColumnInfo(name: "id", dataType: "int4", isPk: true, isNullable: false),
                        FfiColumnInfo(name: "email", dataType: "text", isPk: false, isNullable: true),
                    ]
                ),
            ],
            foreignKeys: []
        )
        let model = DiagramModel(ffi: ffiData)
        let users = model.tables.first { $0.name == "users" }!
        for col in users.columns {
            #expect(col.isFk == false)
        }
    }

    @Test func diagramForeignKeyFromFfi() {
        let ffi = FfiForeignKey(
            fromSchema: "public", fromTable: "orders", fromCol: "user_id",
            toSchema: "public", toTable: "users", toCol: "id",
            constraintName: "fk_orders_user"
        )
        let fk = DiagramForeignKey(ffi: ffi)
        #expect(fk.fromSchema == "public")
        #expect(fk.fromTable == "orders")
        #expect(fk.fromCol == "user_id")
        #expect(fk.toSchema == "public")
        #expect(fk.toTable == "users")
        #expect(fk.toCol == "id")
        #expect(fk.constraintName == "fk_orders_user")
    }
}
