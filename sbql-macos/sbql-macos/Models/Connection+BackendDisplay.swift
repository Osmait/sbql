import SwiftUI

// MARK: - Backend display properties (single source of truth)

extension Connection.Backend {
    var displayLabel: String {
        switch self {
        case .postgres: "PostgreSQL"
        case .mysql: "MySQL"
        case .sqlite: "SQLite"
        case .redis: "Redis"
        case .dynamodb: "DynamoDB"
        case .mongodb: "MongoDB"
        case .sqlserver: "SQL Server"
        }
    }

    var abbreviation: String {
        switch self {
        case .postgres: "PG"
        case .mysql: "MY"
        case .sqlite: "SQ"
        case .redis: "RD"
        case .dynamodb: "DB"
        case .mongodb: "MG"
        case .sqlserver: "MS"
        }
    }

    var color: Color {
        switch self {
        case .postgres: Color(hex: 0x336791)
        case .mysql: Color(hex: 0x00758F)
        case .sqlite: Color(hex: 0x44A8D6)
        case .redis: Color(hex: 0xD82C20)
        case .dynamodb: Color(hex: 0x4053D6)
        case .mongodb: Color(hex: 0x47A248)
        case .sqlserver: Color(hex: 0xCC2927)
        }
    }
}
