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

    var isConnected: Bool = false

    enum Backend: String, CaseIterable, Hashable {
        case postgres
        case sqlite
    }

    enum SSLMode: String, CaseIterable, Hashable {
        case prefer
        case disable
        case require
        case verifyCa
        case verifyFull

        var displayName: String {
            switch self {
            case .prefer:     "Prefer"
            case .disable:    "Disable"
            case .require:    "Require"
            case .verifyCa:   "Verify CA"
            case .verifyFull: "Verify Full"
            }
        }
    }

    var displaySubtitle: String {
        switch backend {
        case .postgres:
            "\(user)@\(host):\(port)/\(database)"
        case .sqlite:
            filePath ?? "In-memory"
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
}

// MARK: - FFI Conversions

extension Connection {
    init(ffi: FfiConnectionConfig) {
        self.id       = ffi.id
        self.name     = ffi.name
        self.backend  = Backend(ffi: ffi.backend)
        self.host     = ffi.host
        self.port     = ffi.port
        self.user     = ffi.user
        self.database = ffi.database
        self.sslMode  = SSLMode(ffi: ffi.sslMode)
        self.filePath = ffi.filePath
    }

    var ffi: FfiConnectionConfig {
        FfiConnectionConfig(
            id:       id,
            name:     name,
            backend:  backend.ffi,
            host:     host,
            port:     port,
            user:     user,
            database: database,
            sslMode:  sslMode.ffi,
            filePath: filePath
        )
    }
}

extension Connection.Backend {
    init(ffi: FfiDbBackend) {
        switch ffi {
        case .postgres: self = .postgres
        case .sqlite:   self = .sqlite
        }
    }

    var ffi: FfiDbBackend {
        switch self {
        case .postgres: .postgres
        case .sqlite:   .sqlite
        }
    }
}

extension Connection.SSLMode {
    init(ffi: FfiSslMode) {
        switch ffi {
        case .prefer:     self = .prefer
        case .disable:    self = .disable
        case .require:    self = .require
        case .verifyCa:   self = .verifyCa
        case .verifyFull: self = .verifyFull
        }
    }

    var ffi: FfiSslMode {
        switch self {
        case .prefer:     .prefer
        case .disable:    .disable
        case .require:    .require
        case .verifyCa:   .verifyCa
        case .verifyFull: .verifyFull
        }
    }
}
