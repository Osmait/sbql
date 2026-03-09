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

        return foreignKeys.compactMap { fk in
            let fromId = "\(fk.fromSchema).\(fk.fromTable)"
            let toId = "\(fk.toSchema).\(fk.toTable)"

            guard let fromPos = positions[fromId],
                  let toPos = positions[toId],
                  let fromTable = tableById[fromId],
                  let toTable = tableById[toId] else { return nil }

            let fromColIndex = fromTable.columns.firstIndex(where: { $0.name == fk.fromCol }) ?? 0
            let toColIndex = toTable.columns.firstIndex(where: { $0.name == fk.toCol }) ?? 0

            let fromY = fromPos.y + DiagramLayout.headerHeight + CGFloat(fromColIndex) * DiagramLayout.rowHeight + DiagramLayout.rowHeight / 2
            let toY = toPos.y + DiagramLayout.headerHeight + CGFloat(toColIndex) * DiagramLayout.rowHeight + DiagramLayout.rowHeight / 2

            // Determine exit direction: connect from right edge if target is to the right, else left
            let fromExitsRight = toPos.x >= fromPos.x
            let toEntersLeft = toPos.x >= fromPos.x

            let fromX = fromExitsRight ? fromPos.x + DiagramLayout.nodeWidth : fromPos.x
            let toX = toEntersLeft ? toPos.x : toPos.x + DiagramLayout.nodeWidth

            return DiagramEdge(
                id: fk.constraintName,
                fromTableId: fromId,
                toTableId: toId,
                fromCol: fk.fromCol,
                toCol: fk.toCol,
                fromPoint: CGPoint(x: fromX, y: fromY),
                toPoint: CGPoint(x: toX, y: toY),
                isHovered: hoveredConstraint == fk.constraintName
            )
        }
    }

    /// Builds a bezier path for a single edge.
    static func bezierPath(for edge: DiagramEdge) -> Path {
        var path = Path()
        path.move(to: edge.fromPoint)

        let dx = abs(edge.toPoint.x - edge.fromPoint.x)
        let controlOffset = max(40, dx * 0.4)

        let dirFrom: CGFloat = edge.toPoint.x >= edge.fromPoint.x ? 1 : -1
        let dirTo: CGFloat = edge.toPoint.x >= edge.fromPoint.x ? -1 : 1

        path.addCurve(
            to: edge.toPoint,
            control1: CGPoint(x: edge.fromPoint.x + controlOffset * dirFrom, y: edge.fromPoint.y),
            control2: CGPoint(x: edge.toPoint.x + controlOffset * dirTo, y: edge.toPoint.y)
        )
        return path
    }

    /// Draw a one-tick mark at the source (one side of the relationship).
    static func sourceMarkerPath(for edge: DiagramEdge) -> Path {
        let p = edge.fromPoint
        let tickLen: CGFloat = 6
        var path = Path()
        path.move(to: CGPoint(x: p.x, y: p.y - tickLen))
        path.addLine(to: CGPoint(x: p.x, y: p.y + tickLen))
        return path
    }

    /// Draw a crow's foot (three-prong) at the target.
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
