import SwiftUI

/// A precomputed FK edge between two column rows.
struct DiagramEdge: Identifiable {
    let id: String // constraint name
    let fromTableId: String
    let toTableId: String
    let fromCol: String
    let toCol: String
    let fromPoint: CGPoint
    let toPoint: CGPoint
    let isHovered: Bool
    let colorIndex: Int
    /// Offset applied to the vertical segment to prevent overlapping parallel routes.
    let channelOffset: CGFloat
}

/// Computes drawable edges anchored to specific column rows at left/right node edges.
enum DiagramEdgeGeometry {
    static func edges(
        for foreignKeys: [DiagramForeignKey],
        tables: [DiagramTable],
        positions: [String: CGPoint],
        hoveredConstraint: String?
    ) -> [DiagramEdge] {
        let tableById = Dictionary(uniqueKeysWithValues: tables.map { ($0.id, $0) })

        // First pass: build raw edges to compute channel offsets
        struct RawEdge {
            let fk: DiagramForeignKey
            let fromPoint: CGPoint
            let toPoint: CGPoint
            let fromId: String
            let toId: String
            let index: Int
        }

        var rawEdges: [RawEdge] = []
        for (index, fk) in foreignKeys.enumerated() {
            let fromId = "\(fk.fromSchema).\(fk.fromTable)"
            let toId = "\(fk.toSchema).\(fk.toTable)"

            guard let fromPos = positions[fromId],
                  let toPos = positions[toId],
                  let fromTable = tableById[fromId],
                  let toTable = tableById[toId] else { continue }

            let fromColIndex = fromTable.columns.firstIndex(where: { $0.name == fk.fromCol }) ?? 0
            let toColIndex = toTable.columns.firstIndex(where: { $0.name == fk.toCol }) ?? 0

            let fromY = fromPos.y + DiagramLayout.headerHeight + CGFloat(fromColIndex) * DiagramLayout.rowHeight + DiagramLayout.rowHeight / 2
            let toY = toPos.y + DiagramLayout.headerHeight + CGFloat(toColIndex) * DiagramLayout.rowHeight + DiagramLayout.rowHeight / 2

            let fromExitsRight = toPos.x >= fromPos.x
            let fromX = fromExitsRight ? fromPos.x + DiagramLayout.nodeWidth : fromPos.x
            let toX = fromExitsRight ? toPos.x : toPos.x + DiagramLayout.nodeWidth

            rawEdges.append(RawEdge(
                fk: fk,
                fromPoint: CGPoint(x: fromX, y: fromY),
                toPoint: CGPoint(x: toX, y: toY),
                fromId: fromId,
                toId: toId,
                index: index
            ))
        }

        // Pre-compute pair counts in one pass to avoid O(N^2) nested filter.
        var pairCounts: [String: Int] = [:]
        for raw in rawEdges {
            let key = [raw.fromId, raw.toId].sorted().joined(separator: "|")
            pairCounts[key, default: 0] += 1
        }

        var tablePairIndex: [String: Int] = [:]

        return rawEdges.map { raw in
            let pairKey = [raw.fromId, raw.toId].sorted().joined(separator: "|")
            let idx = tablePairIndex[pairKey] ?? 0
            tablePairIndex[pairKey] = idx + 1
            let total = pairCounts[pairKey] ?? 1

            // Spread channels: center them around 0
            let spacing: CGFloat = 8
            let offset = CGFloat(idx) * spacing - CGFloat(total - 1) * spacing / 2

            return DiagramEdge(
                id: raw.fk.constraintName,
                fromTableId: raw.fromId,
                toTableId: raw.toId,
                fromCol: raw.fk.fromCol,
                toCol: raw.fk.toCol,
                fromPoint: raw.fromPoint,
                toPoint: raw.toPoint,
                isHovered: hoveredConstraint == raw.fk.constraintName,
                colorIndex: raw.index % SbqlTheme.Colors.fkLinePalette.count,
                channelOffset: offset
            )
        }
    }

    // MARK: - Orthogonal Path

    /// Builds an orthogonal (right-angle) path: horizontal → vertical → horizontal.
    static func orthogonalPath(for edge: DiagramEdge) -> Path {
        let from = edge.fromPoint
        let to = edge.toPoint
        let offset = edge.channelOffset

        var path = Path()
        path.move(to: from)

        // Midpoint X for the vertical segment
        let midX = (from.x + to.x) / 2 + offset

        // Horizontal from source to midX
        path.addLine(to: CGPoint(x: midX, y: from.y))
        // Vertical from source Y to target Y
        path.addLine(to: CGPoint(x: midX, y: to.y))
        // Horizontal from midX to target
        path.addLine(to: to)

        return path
    }

    /// Builds a rounded orthogonal path with small corner radii at the two turns.
    static func roundedOrthogonalPath(for edge: DiagramEdge) -> Path {
        let from = edge.fromPoint
        let to = edge.toPoint
        let offset = edge.channelOffset
        let r: CGFloat = 6 // corner radius

        var path = Path()
        path.move(to: from)

        let midX = (from.x + to.x) / 2 + offset
        let dy = to.y - from.y

        // If nearly horizontal, just draw a straight line
        if abs(dy) < r * 2 {
            path.addLine(to: to)
            return path
        }

        // If nearly vertical (same X), draw straight
        if abs(midX - from.x) < r * 2 || abs(midX - to.x) < r * 2 {
            path.addLine(to: CGPoint(x: midX, y: from.y))
            path.addLine(to: CGPoint(x: midX, y: to.y))
            path.addLine(to: to)
            return path
        }

        let signX1: CGFloat = midX > from.x ? 1 : -1
        let signY: CGFloat = dy > 0 ? 1 : -1
        let signX2: CGFloat = to.x > midX ? 1 : -1

        // Horizontal segment from source, stop before first corner
        path.addLine(to: CGPoint(x: midX - r * signX1, y: from.y))

        // First rounded corner (horizontal → vertical)
        path.addQuadCurve(
            to: CGPoint(x: midX, y: from.y + r * signY),
            control: CGPoint(x: midX, y: from.y)
        )

        // Vertical segment, stop before second corner
        path.addLine(to: CGPoint(x: midX, y: to.y - r * signY))

        // Second rounded corner (vertical → horizontal)
        path.addQuadCurve(
            to: CGPoint(x: midX + r * signX2, y: to.y),
            control: CGPoint(x: midX, y: to.y)
        )

        // Horizontal segment to target
        path.addLine(to: to)

        return path
    }

    // MARK: - Markers

    /// Draw a one-tick mark at the source (one side of the relationship).
    static func sourceMarkerPath(for edge: DiagramEdge) -> Path {
        let p = edge.fromPoint
        let tickLen: CGFloat = 6
        var path = Path()
        path.move(to: CGPoint(x: p.x, y: p.y - tickLen))
        path.addLine(to: CGPoint(x: p.x, y: p.y + tickLen))
        return path
    }

    /// Draw a crow's foot (three-prong) at the target — horizontal direction into the node.
    static func crowsFootPath(for edge: DiagramEdge) -> Path {
        let p = edge.toPoint
        let len: CGFloat = 10
        let spread: CGFloat = 7
        let dir: CGFloat = edge.toPoint.x >= edge.fromPoint.x ? -1 : 1

        var path = Path()
        // Center prong
        path.move(to: p)
        path.addLine(to: CGPoint(x: p.x + len * dir, y: p.y))
        // Top prong
        path.move(to: p)
        path.addLine(to: CGPoint(x: p.x + len * dir, y: p.y - spread))
        // Bottom prong
        path.move(to: p)
        path.addLine(to: CGPoint(x: p.x + len * dir, y: p.y + spread))
        return path
    }
}
