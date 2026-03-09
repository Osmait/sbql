import Foundation

/// Data for the ER diagram view.
struct DiagramModel {
    let tables: [DiagramTable]
    let foreignKeys: [DiagramForeignKey]

    static let empty = DiagramModel(tables: [], foreignKeys: [])
}

struct DiagramTable: Identifiable, Hashable {
    var id: String {
        "\(schema).\(name)"
    }

    let schema: String
    let name: String
    let columns: [DiagramColumn]
}

struct DiagramColumn: Identifiable, Hashable {
    var id: String {
        "\(tableId).\(name)"
    }

    let tableId: String
    let name: String
    let dataType: String
    let isPk: Bool
    let isNullable: Bool
    let isFk: Bool
}

struct DiagramForeignKey: Identifiable, Hashable {
    var id: String {
        constraintName
    }

    let fromSchema: String
    let fromTable: String
    let fromCol: String
    let toSchema: String
    let toTable: String
    let toCol: String
    let constraintName: String
}

// MARK: - FFI Conversions

extension DiagramModel {
    init(ffi: FfiDiagramData) {
        let rawFKs = ffi.foreignKeys.map(DiagramForeignKey.init)

        // Build lookup set of columns that participate in foreign keys
        var fkColumnSet = Set<String>()
        for fk in rawFKs {
            fkColumnSet.insert("\(fk.fromSchema).\(fk.fromTable).\(fk.fromCol)")
            fkColumnSet.insert("\(fk.toSchema).\(fk.toTable).\(fk.toCol)")
        }

        tables = ffi.tables.map { ffiTable in
            let tableId = "\(ffiTable.schema).\(ffiTable.name)"
            return DiagramTable(
                schema: ffiTable.schema,
                name: ffiTable.name,
                columns: ffiTable.columns.map { ffiCol in
                    let qualifiedCol = "\(tableId).\(ffiCol.name)"
                    return DiagramColumn(
                        tableId: tableId,
                        name: ffiCol.name,
                        dataType: ffiCol.dataType,
                        isPk: ffiCol.isPk,
                        isNullable: ffiCol.isNullable,
                        isFk: fkColumnSet.contains(qualifiedCol)
                    )
                }
            )
        }
        foreignKeys = rawFKs
    }
}

extension DiagramForeignKey {
    init(ffi: FfiForeignKey) {
        fromSchema = ffi.fromSchema
        fromTable = ffi.fromTable
        fromCol = ffi.fromCol
        toSchema = ffi.toSchema
        toTable = ffi.toTable
        toCol = ffi.toCol
        constraintName = ffi.constraintName
    }
}
