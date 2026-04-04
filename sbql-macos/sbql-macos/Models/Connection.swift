import Foundation

/// Swift domain model for a saved database connection.
struct Connection: Identifiable, Hashable {
    let id: String
    var name: String
    var backend: Backend
    var host: String
    var port: UInt16
    var user: String
    var database: String
    var sslMode: SSLMode
    var filePath: String?
    var requiresBiometric: Bool = false

    var isConnected: Bool = false

    enum Backend: String, CaseIterable, Hashable {
        case postgres
        case sqlite
        case redis
        case mysql
        case dynamodb
        case mongodb
    }

    enum SSLMode: String, CaseIterable, Hashable {
        case prefer
        case disable
        case require
        case verifyCa
        case verifyFull

        var displayName: String {
            switch self {
            case .prefer: "Prefer"
            case .disable: "Disable"
            case .require: "Require"
            case .verifyCa: "Verify CA"
            case .verifyFull: "Verify Full"
            }
        }
    }

    var displaySubtitle: String {
        switch backend {
        case .postgres, .mysql:
            "\(user)@\(host):\(port)/\(database)"
        case .sqlite:
            filePath ?? "In-memory"
        case .redis:
            "\(host):\(port)/\(database)"
        case .dynamodb:
            "\(host):\(port) (\(database))"
        case .mongodb:
            "\(host):\(port)/\(database)"
        }
    }

    static func newPostgres() -> Connection {
        Connection(
            id: UUID().uuidString.lowercased(),
            name: "",
            backend: .postgres,
            host: "localhost",
            port: 5432,
            user: "postgres",
            database: "postgres",
            sslMode: .prefer
        )
    }

    static func newMysql() -> Connection {
        Connection(
            id: UUID().uuidString.lowercased(),
            name: "",
            backend: .mysql,
            host: "localhost",
            port: 3306,
            user: "root",
            database: "",
            sslMode: .prefer
        )
    }

    static func newSqlite() -> Connection {
        Connection(
            id: UUID().uuidString.lowercased(),
            name: "",
            backend: .sqlite,
            host: "",
            port: 0,
            user: "",
            database: "",
            sslMode: .prefer,
            filePath: ""
        )
    }

    static func newMongodb() -> Connection {
        Connection(
            id: UUID().uuidString.lowercased(),
            name: "",
            backend: .mongodb,
            host: "localhost",
            port: 27017,
            user: "",
            database: "",
            sslMode: .prefer
        )
    }

    static func newDynamodb() -> Connection {
        Connection(
            id: UUID().uuidString.lowercased(),
            name: "",
            backend: .dynamodb,
            host: "localhost",
            port: 8000,
            user: "",
            database: "us-east-1",
            sslMode: .prefer
        )
    }

}

// MARK: - FFI Conversions

extension Connection {
    init(ffi: FfiConnectionConfig) {
        id = ffi.id
        name = ffi.name
        backend = Backend(ffi: ffi.backend)
        host = ffi.host
        port = ffi.port
        user = ffi.user
        database = ffi.database
        sslMode = SSLMode(ffi: ffi.sslMode)
        filePath = ffi.filePath
        // Biometric flag persisted in UserDefaults (not in Rust FFI)
        requiresBiometric = UserDefaults.standard.bool(forKey: "biometric_\(ffi.id)")
    }

    var ffi: FfiConnectionConfig {
        FfiConnectionConfig(
            id: id,
            name: name,
            backend: backend.ffi,
            host: host,
            port: port,
            user: user,
            database: database,
            sslMode: sslMode.ffi,
            filePath: filePath
        )
    }
}

extension Connection.Backend {
    init(ffi: FfiDbBackend) {
        switch ffi {
        case .postgres: self = .postgres
        case .sqlite: self = .sqlite
        case .redis: self = .redis
        case .mysql: self = .mysql
        case .dynamoDb: self = .dynamodb
        case .mongoDb: self = .mongodb
        }
    }

    var ffi: FfiDbBackend {
        switch self {
        case .postgres: .postgres
        case .sqlite: .sqlite
        case .redis: .redis
        case .mysql: .mysql
        case .dynamodb: .dynamoDb
        case .mongodb: .mongoDb
        }
    }
}

extension Connection.SSLMode {
    init(ffi: FfiSslMode) {
        switch ffi {
        case .prefer: self = .prefer
        case .disable: self = .disable
        case .require: self = .require
        case .verifyCa: self = .verifyCa
        case .verifyFull: self = .verifyFull
        }
    }

    var ffi: FfiSslMode {
        switch self {
        case .prefer: .prefer
        case .disable: .disable
        case .require: .require
        case .verifyCa: .verifyCa
        case .verifyFull: .verifyFull
        }
    }
}
