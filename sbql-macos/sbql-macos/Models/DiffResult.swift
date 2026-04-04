import Foundation

struct DiffResult {
    let addedRows: Set<Int>
    let removedRows: [[String]]
    let changedCells: [CellKey: (old: String, new: String)]

    var isEmpty: Bool { addedRows.isEmpty && removedRows.isEmpty && changedCells.isEmpty }

    var summary: String {
        var parts: [String] = []
        if !addedRows.isEmpty { parts.append("+\(addedRows.count) added") }
        if !removedRows.isEmpty { parts.append("-\(removedRows.count) removed") }
        if !changedCells.isEmpty { parts.append("~\(changedCells.count) changed") }
        return parts.isEmpty ? "No changes" : parts.joined(separator: ", ")
    }
}
