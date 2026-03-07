import Testing
import SwiftUI
@testable import sbql_macos

struct DiagramViewModelTests {

    // MARK: - Helpers

    private func makeTable(schema: String, name: String, columnCount: Int) -> DiagramTable {
        let tableId = "\(schema).\(name)"
        let columns = (0..<columnCount).map { i in
            DiagramColumn(
                tableId: tableId,
                name: "col\(i)",
                dataType: "text",
                isPk: i == 0,
                isNullable: i > 0,
                isFk: false
            )
        }
        return DiagramTable(schema: schema, name: name, columns: columns)
    }

    private func makeFk(
        from: (schema: String, table: String, col: String),
        to: (schema: String, table: String, col: String),
        constraint: String
    ) -> DiagramForeignKey {
        DiagramForeignKey(
            fromSchema: from.schema, fromTable: from.table, fromCol: from.col,
            toSchema: to.schema, toTable: to.table, toCol: to.col,
            constraintName: constraint
        )
    }

    // MARK: - computeInitialLayout

    @Test func computeInitialLayout_emptyDiagram() {
        let vm = DiagramViewModel()
        vm.diagramData = .empty
        vm.computeInitialLayout()
        #expect(vm.tablePositions.isEmpty)
    }

    @Test func computeInitialLayout_singleTable() {
        let vm = DiagramViewModel()
        vm.diagramData = DiagramModel(
            tables: [makeTable(schema: "public", name: "users", columnCount: 3)],
            foreignKeys: []
        )
        vm.computeInitialLayout()

        #expect(vm.tablePositions.count == 1)
        let pos = vm.tablePositions["public.users"]
        #expect(pos != nil)
        // Single unconnected table: startX=60, unconnectedStartY = startY(60) + spacingY(220) = 280
        #expect(pos!.x == 60)
        #expect(pos!.y == 280)
    }

    @Test func computeInitialLayout_unconnectedTables() {
        let vm = DiagramViewModel()
        vm.diagramData = DiagramModel(
            tables: [
                makeTable(schema: "public", name: "a", columnCount: 2),
                makeTable(schema: "public", name: "b", columnCount: 2),
                makeTable(schema: "public", name: "c", columnCount: 2),
            ],
            foreignKeys: []
        )
        vm.computeInitialLayout()

        #expect(vm.tablePositions.count == 3)
        // All three should be positioned (grid layout)
        for id in ["public.a", "public.b", "public.c"] {
            #expect(vm.tablePositions[id] != nil)
        }
    }

    @Test func computeInitialLayout_connectedTables() {
        let vm = DiagramViewModel()
        let fk = makeFk(
            from: (schema: "public", table: "orders", col: "user_id"),
            to: (schema: "public", table: "users", col: "id"),
            constraint: "fk_orders_users"
        )
        vm.diagramData = DiagramModel(
            tables: [
                makeTable(schema: "public", name: "users", columnCount: 3),
                makeTable(schema: "public", name: "orders", columnCount: 4),
            ],
            foreignKeys: [fk]
        )
        vm.computeInitialLayout()

        #expect(vm.tablePositions.count == 2)
        #expect(vm.tablePositions["public.users"] != nil)
        #expect(vm.tablePositions["public.orders"] != nil)
    }

    @Test func computeInitialLayout_mixedConnectedAndUnconnected() {
        let vm = DiagramViewModel()
        let fk = makeFk(
            from: (schema: "public", table: "orders", col: "user_id"),
            to: (schema: "public", table: "users", col: "id"),
            constraint: "fk_orders_users"
        )
        vm.diagramData = DiagramModel(
            tables: [
                makeTable(schema: "public", name: "users", columnCount: 3),
                makeTable(schema: "public", name: "orders", columnCount: 4),
                makeTable(schema: "public", name: "logs", columnCount: 2),
            ],
            foreignKeys: [fk]
        )
        vm.computeInitialLayout()

        #expect(vm.tablePositions.count == 3)
        // Connected tables placed first (lower Y), unconnected below
        let connectedMaxY = max(
            vm.tablePositions["public.users"]!.y,
            vm.tablePositions["public.orders"]!.y
        )
        let logsY = vm.tablePositions["public.logs"]!.y
        #expect(logsY > connectedMaxY)
    }

    // MARK: - moveTable

    @Test func moveTable_updatesPosition() {
        let vm = DiagramViewModel()
        vm.tablePositions["public.users"] = CGPoint(x: 100, y: 200)
        vm.moveTable(id: "public.users", by: CGSize(width: 50, height: -30))

        let pos = vm.tablePositions["public.users"]!
        #expect(pos.x == 150)
        #expect(pos.y == 170)
    }

    @Test func moveTable_missingId() {
        let vm = DiagramViewModel()
        vm.tablePositions["public.users"] = CGPoint(x: 100, y: 200)
        vm.moveTable(id: "public.unknown", by: CGSize(width: 50, height: -30))

        // No crash, existing positions unchanged
        #expect(vm.tablePositions.count == 1)
        #expect(vm.tablePositions["public.users"]!.x == 100)
    }

    // MARK: - Zoom

    @Test func zoomIn_incrementsScale() {
        let vm = DiagramViewModel()
        let before = vm.scale
        vm.zoomIn()
        #expect(vm.scale == before + 0.15)
    }

    @Test func zoomOut_decrementsScale() {
        let vm = DiagramViewModel()
        let before = vm.scale
        vm.zoomOut()
        #expect(vm.scale == before - 0.15)
    }

    @Test func zoomIn_clampsAtMax() {
        let vm = DiagramViewModel()
        vm.scale = 3.0
        vm.zoomIn()
        #expect(vm.scale <= 3.0)
    }

    @Test func zoomOut_clampsAtMin() {
        let vm = DiagramViewModel()
        vm.scale = 0.2
        vm.zoomOut()
        #expect(vm.scale >= 0.2)
    }

    @Test func zoomPercent_computation() {
        let vm = DiagramViewModel()
        vm.scale = 1.0
        #expect(vm.zoomPercent == 100)

        vm.scale = 0.5
        #expect(vm.zoomPercent == 50)

        vm.scale = 1.5
        #expect(vm.zoomPercent == 150)
    }

    // MARK: - computeContentBounds

    @Test func computeContentBounds_emptyPositions() {
        let vm = DiagramViewModel()
        let bounds = vm.computeContentBounds()
        #expect(bounds == .zero)
    }

    @Test func computeContentBounds_withPositions() {
        let vm = DiagramViewModel()
        let t1 = makeTable(schema: "public", name: "users", columnCount: 3)
        let t2 = makeTable(schema: "public", name: "orders", columnCount: 5)
        vm.diagramData = DiagramModel(tables: [t1, t2], foreignKeys: [])
        vm.tablePositions = [
            "public.users": CGPoint(x: 100, y: 100),
            "public.orders": CGPoint(x: 500, y: 300),
        ]

        let bounds = vm.computeContentBounds()
        #expect(bounds.minX == 100)
        #expect(bounds.minY == 100)
        // maxX = 500 + nodeWidth(240) = 740, width = 740 - 100 = 640
        #expect(bounds.width == 640)
        // maxY = 300 + headerHeight(32) + 5*rowHeight(22) = 300 + 142 = 442
        // height = 442 - 100 = 342
        let expectedHeight = 300 + DiagramLayout.headerHeight + 5 * DiagramLayout.rowHeight - 100
        #expect(bounds.height == expectedHeight)
    }

    // MARK: - fitToScreen

    @Test func fitToScreen_centersContent() {
        let vm = DiagramViewModel()
        let t = makeTable(schema: "public", name: "users", columnCount: 3)
        vm.diagramData = DiagramModel(tables: [t], foreignKeys: [])
        vm.tablePositions = ["public.users": CGPoint(x: 100, y: 100)]

        let viewport = CGSize(width: 1200, height: 800)
        vm.fitToScreen(viewportSize: viewport)

        // Scale should be clamped between 0.2 and 1.5
        #expect(vm.scale >= 0.2)
        #expect(vm.scale <= 1.5)
        // Offset should be set (non-zero for centering)
        #expect(vm.offset != .zero)
    }
}
