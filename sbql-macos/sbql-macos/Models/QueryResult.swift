import Foundation

/// Swift domain model for a paginated query result.
struct QueryResultData: Equatable {
    let columns: [String]
    let rows: [[String]]
    let page: UInt32
    let hasNextPage: Bool

    var isEmpty: Bool {
        rows.isEmpty
    }

    var rowCount: Int {
        rows.count
    }

    var columnCount: Int {
        columns.count
    }

    static let empty = QueryResultData(columns: [], rows: [], page: 0, hasNextPage: false)
}

extension QueryResultData {
    init(ffi: FfiQueryResult) {
        columns = ffi.columns
        rows = ffi.rows
        page = ffi.page
        hasNextPage = ffi.hasNextPage
    }
}
