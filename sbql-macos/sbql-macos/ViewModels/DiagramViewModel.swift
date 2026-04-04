import SwiftUI

/// State for the ER diagram view.
@Observable
final class DiagramViewModel {
    var diagramData: DiagramModel = .empty
    var isLoading: Bool = false
    var scale: CGFloat = 1.0
    var offset: CGSize = .zero

    /// Per-table positions (enables drag)
    var tablePositions: [String: CGPoint] = [:]

    // Selection / hover state
    var selectedTableId: String?
    var hoveredTableId: String?
    var hoveredFkConstraint: String?

    // MARK: - Layout

    /// Hierarchical left-to-right layout (Sugiyama-inspired).
    /// Parent tables (referenced by FKs) appear on the left; children flow to the right.
    /// This makes FK lines flow consistently and reduces crossings.
    func computeInitialLayout() {
        let tables = diagramData.tables
        let fks = diagramData.foreignKeys
        guard !tables.isEmpty else { return }

        let tableById = Dictionary(uniqueKeysWithValues: tables.map { ($0.id, $0) })
        let allIds = Set(tables.map(\.id))

        // Build directed graph: child → parents, parent → children
        var childrenOf: [String: Set<String>] = [:] // parent → set of children
        var parentsOf: [String: Set<String>] = [:]  // child → set of parents
        for id in allIds {
            childrenOf[id] = []
            parentsOf[id] = []
        }
        for fk in fks {
            let childId = "\(fk.fromSchema).\(fk.fromTable)"
            let parentId = "\(fk.toSchema).\(fk.toTable)"
            guard childId != parentId,
                  allIds.contains(childId),
                  allIds.contains(parentId) else { continue }
            childrenOf[parentId, default: []].insert(childId)
            parentsOf[childId, default: []].insert(parentId)
        }

        // Identify connected vs unconnected tables
        let connectedIds = allIds.filter {
            !(childrenOf[$0] ?? []).isEmpty || !(parentsOf[$0] ?? []).isEmpty
        }
        let unconnectedIds = tables.map(\.id).filter { !connectedIds.contains($0) }

        // --- Layer assignment via longest-path from roots ---
        // Roots = tables that have no parents (they are only referenced, never reference others)
        var layers: [String: Int] = [:]
        let roots = connectedIds.filter { (parentsOf[$0] ?? []).isEmpty }

        // If cycles exist, there may be no pure roots — pick the most-referenced table
        var startNodes = roots
        if startNodes.isEmpty, !connectedIds.isEmpty {
            let mostReferenced = connectedIds.max {
                (childrenOf[$0]?.count ?? 0) < (childrenOf[$1]?.count ?? 0)
            }
            if let node = mostReferenced { startNodes = [node] }
        }

        // BFS layer assignment (longest path = max layer)
        var assigned = Set<String>()
        var queue: [(String, Int)] = startNodes.map { ($0, 0) }
        for node in startNodes {
            layers[node] = 0
            assigned.insert(node)
        }

        while !queue.isEmpty {
            let (current, layer) = queue.removeFirst()
            for child in childrenOf[current] ?? [] {
                let newLayer = layer + 1
                if newLayer > (layers[child] ?? -1) {
                    layers[child] = newLayer
                }
                if !assigned.contains(child) {
                    assigned.insert(child)
                    queue.append((child, newLayer))
                } else {
                    // Re-enqueue to propagate deeper layers
                    queue.append((child, newLayer))
                }
            }
        }

        // Assign remaining connected tables (from cycles) to layer 0
        for id in connectedIds where layers[id] == nil {
            layers[id] = 0
        }

        // --- Group by layer ---
        var layerGroups: [Int: [String]] = [:]
        for (tableId, layer) in layers {
            layerGroups[layer, default: []].append(tableId)
        }
        let maxLayer = layerGroups.keys.max() ?? 0

        // --- Barycenter ordering to reduce crossings ---
        // Initial: sort by name within each layer
        for layer in layerGroups.keys {
            layerGroups[layer]?.sort()
        }

        // Refine ordering: 3 forward passes using barycenter heuristic
        for _ in 0 ..< 3 {
            for layer in 1 ... maxLayer {
                guard let nodes = layerGroups[layer] else { continue }
                let prevNodes = layerGroups[layer - 1] ?? []
                let prevPositions = Dictionary(uniqueKeysWithValues: prevNodes.enumerated().map { ($1, $0) })

                layerGroups[layer] = nodes.sorted { a, b in
                    let aParents = (parentsOf[a] ?? []).compactMap { prevPositions[$0] }
                    let bParents = (parentsOf[b] ?? []).compactMap { prevPositions[$0] }
                    let aCenter = aParents.isEmpty
                        ? Double(nodes.count) / 2.0
                        : Double(aParents.reduce(0, +)) / Double(aParents.count)
                    let bCenter = bParents.isEmpty
                        ? Double(nodes.count) / 2.0
                        : Double(bParents.reduce(0, +)) / Double(bParents.count)
                    return aCenter < bCenter
                }
            }
        }

        // --- Position nodes: layers left→right, split tall layers into sub-columns ---
        let startX: CGFloat = 60
        let startY: CGFloat = 60
        let subColGap: CGFloat = DiagramLayout.nodeWidth + 40  // gap between sub-columns within a layer
        let layerExtraGap: CGFloat = 60                         // extra gap between different layers
        let nodeGap: CGFloat = 24
        let maxColumnHeight: CGFloat = 900 // max height before wrapping into sub-column

        var positions: [String: CGPoint] = [:]

        // Place connected tables layer by layer
        var currentX: CGFloat = startX

        for layer in 0 ... maxLayer {
            guard let nodes = layerGroups[layer] else { continue }

            // Split nodes into sub-columns that fit within maxColumnHeight
            var subColumns: [[String]] = [[]]
            var currentColHeight: CGFloat = 0

            for tableId in nodes {
                let h = nodeHeight(for: tableId)
                if !subColumns[subColumns.count - 1].isEmpty,
                   currentColHeight + h + nodeGap > maxColumnHeight
                {
                    subColumns.append([])
                    currentColHeight = 0
                }
                subColumns[subColumns.count - 1].append(tableId)
                currentColHeight += h + nodeGap
            }

            // Position each sub-column
            for (subColIdx, subCol) in subColumns.enumerated() {
                let x = currentX + CGFloat(subColIdx) * subColGap
                var y = startY

                for tableId in subCol {
                    positions[tableId] = CGPoint(x: x, y: y)
                    y += nodeHeight(for: tableId) + nodeGap
                }
            }

            // Advance X past all sub-columns of this layer
            currentX += CGFloat(subColumns.count) * subColGap + layerExtraGap
        }

        // --- Vertical centering: align shorter layers to the vertical center ---
        var layerMaxBottom: CGFloat = 0
        for (_, pos) in positions {
            // We can't easily get tableId here, so find the global max bottom
            layerMaxBottom = max(layerMaxBottom, pos.y)
        }
        // Compute actual max bottom with heights
        var globalBottom: CGFloat = startY
        for (tableId, pos) in positions {
            globalBottom = max(globalBottom, pos.y + nodeHeight(for: tableId))
        }
        let globalHeight = globalBottom - startY

        // Group positions by their X coordinate to identify columns
        var columnNodes: [CGFloat: [String]] = [:]
        for (tableId, pos) in positions {
            let roundedX = (pos.x / 10).rounded() * 10 // group by approximate X
            columnNodes[roundedX, default: []].append(tableId)
        }
        for (colX, nodes) in columnNodes {
            let colBottom = nodes.map { positions[$0]!.y + nodeHeight(for: $0) }.max() ?? startY
            let colTop = nodes.map { positions[$0]!.y }.min() ?? startY
            let colHeight = colBottom - colTop
            let offsetY = (globalHeight - colHeight) / 2
            guard offsetY > 1 else { continue }
            for tableId in nodes {
                positions[tableId]!.y += offsetY
            }
        }

        // --- Place unconnected tables below ---
        var connectedBottom: CGFloat = startY
        for (tableId, pos) in positions {
            connectedBottom = max(connectedBottom, pos.y + nodeHeight(for: tableId))
        }
        let unconnectedStartY = connectedBottom + 80
        let unconnectedCols = max(1, Int(sqrt(Double(unconnectedIds.count)).rounded(.up)))
        let unconnectedSpacingX = DiagramLayout.nodeWidth + 40

        var unconnectedRowMaxH: [Int: CGFloat] = [:]
        for (idx, tableId) in unconnectedIds.enumerated() {
            let row = idx / unconnectedCols
            unconnectedRowMaxH[row] = max(unconnectedRowMaxH[row] ?? 0, nodeHeight(for: tableId))
        }

        for (idx, tableId) in unconnectedIds.enumerated() {
            let col = idx % unconnectedCols
            let row = idx / unconnectedCols
            var yOffset: CGFloat = 0
            for r in 0 ..< row {
                yOffset += (unconnectedRowMaxH[r] ?? 0) + nodeGap
            }
            positions[tableId] = CGPoint(
                x: startX + CGFloat(col) * unconnectedSpacingX,
                y: unconnectedStartY + yOffset
            )
        }

        tablePositions = positions
    }

    private func nodeHeight(for tableId: String) -> CGFloat {
        guard let table = diagramData.tables.first(where: { $0.id == tableId }) else {
            return DiagramLayout.headerHeight
        }
        return DiagramLayout.headerHeight + CGFloat(table.columns.count) * DiagramLayout.rowHeight
    }

    // MARK: - Drag

    func moveTable(id: String, by delta: CGSize) {
        guard var pos = tablePositions[id] else { return }
        pos.x += delta.width
        pos.y += delta.height
        tablePositions[id] = pos
    }

    // MARK: - Zoom

    func zoomIn() {
        scale = min(3.0, scale + 0.15)
    }

    func zoomOut() {
        scale = max(0.2, scale - 0.15)
    }

    func fitToScreen(viewportSize: CGSize) {
        let bounds = computeContentBounds()
        guard bounds.width > 0, bounds.height > 0, viewportSize.width > 0, viewportSize.height > 0 else { return }

        // Account for the full extent of content including starting offset
        let contentWidth = bounds.maxX + 60
        let contentHeight = bounds.maxY + 60
        let scaleX = viewportSize.width / contentWidth
        let scaleY = viewportSize.height / contentHeight
        scale = max(0.2, min(1.5, min(scaleX, scaleY)))
    }

    var zoomPercent: Int {
        Int((scale * 100).rounded())
    }

    func computeContentBounds() -> CGRect {
        guard !tablePositions.isEmpty else { return .zero }
        var minX = CGFloat.infinity
        var minY = CGFloat.infinity
        var maxX = -CGFloat.infinity
        var maxY = -CGFloat.infinity

        for (tableId, pos) in tablePositions {
            let h = nodeHeight(for: tableId)
            minX = min(minX, pos.x)
            minY = min(minY, pos.y)
            maxX = max(maxX, pos.x + DiagramLayout.nodeWidth)
            maxY = max(maxY, pos.y + h)
        }
        return CGRect(x: minX, y: minY, width: maxX - minX, height: maxY - minY)
    }
}
