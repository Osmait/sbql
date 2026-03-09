import Foundation

/// A table entry from schema introspection.
struct TableEntryModel: Identifiable, Hashable {
    var id: String {
        qualified
    }

    let schema: String
    let name: String

    var qualified: String {
        "\(schema).\(name)"
    }
}

extension TableEntryModel {
    init(ffi: FfiTableEntry) {
        schema = ffi.schema
        name = ffi.name
    }
}
