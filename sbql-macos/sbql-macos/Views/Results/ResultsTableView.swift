import SwiftUI
import AppKit

/// High-performance NSTableView wrapper with virtualized rows and column sorting.
struct ResultsTableView: NSViewRepresentable {
    @Environment(AppViewModel.self) private var appVM

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.borderType = .noBorder
        scrollView.backgroundColor = NSColor(SbqlTheme.Colors.background)

        let tableView = NSTableView()
        tableView.style = .plain
        tableView.backgroundColor = NSColor(SbqlTheme.Colors.background)
        tableView.rowHeight = SbqlTheme.Size.rowHeight
        tableView.intercellSpacing = NSSize(width: 0, height: 1)
        tableView.gridColor = NSColor(SbqlTheme.Colors.borderSubtle)
        tableView.gridStyleMask = [.solidHorizontalGridLineMask]
        tableView.usesAlternatingRowBackgroundColors = false
        tableView.headerView = SbqlTableHeaderView()
        tableView.allowsColumnResizing = true
        tableView.columnAutoresizingStyle = .noColumnAutoresizing
        tableView.allowsMultipleSelection = false

        tableView.delegate = context.coordinator
        tableView.dataSource = context.coordinator
        tableView.doubleAction = #selector(Coordinator.handleDoubleClick(_:))
        tableView.target = context.coordinator
        context.coordinator.tableView = tableView

        // Context menu for row actions (delete)
        let menu = NSMenu()
        menu.delegate = context.coordinator
        tableView.menu = menu

        scrollView.documentView = tableView

        // Set up columns from current result
        context.coordinator.rebuildColumns()

        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        context.coordinator.appVM = appVM
        context.coordinator.rebuildColumns()
        context.coordinator.tableView?.reloadData()
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(appVM: appVM)
    }

    class Coordinator: NSObject, NSTableViewDelegate, NSTableViewDataSource, NSMenuDelegate {
        var appVM: AppViewModel
        weak var tableView: NSTableView?

        init(appVM: AppViewModel) {
            self.appVM = appVM
        }

        func rebuildColumns() {
            guard let tableView else { return }
            let result = appVM.results.currentResult

            // Remove old columns
            for col in tableView.tableColumns.reversed() {
                tableView.removeTableColumn(col)
            }

            // Add new columns
            for (idx, colName) in result.columns.enumerated() {
                let col = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("col_\(idx)"))
                col.title = colName
                col.minWidth = 60
                col.width = max(100, CGFloat(colName.count * 10 + 20))
                col.headerCell = SbqlHeaderCell(colName, coordinator: self, colIndex: idx)
                tableView.addTableColumn(col)
            }
        }

        // MARK: - NSTableViewDataSource

        func numberOfRows(in tableView: NSTableView) -> Int {
            appVM.results.currentResult.rows.count
        }

        // MARK: - NSTableViewDelegate

        func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
            guard let tableColumn else { return nil }
            let result = appVM.results.currentResult
            guard let colIdx = tableView.tableColumns.firstIndex(of: tableColumn),
                  row < result.rows.count,
                  colIdx < result.rows[row].count else { return nil }

            let id = NSUserInterfaceItemIdentifier("Cell")
            let cellView: NSTextField
            if let reused = tableView.makeView(withIdentifier: id, owner: nil) as? NSTextField {
                cellView = reused
            } else {
                cellView = NSTextField(labelWithString: "")
                cellView.identifier = id
                cellView.font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
                cellView.lineBreakMode = .byTruncatingTail
                cellView.cell?.truncatesLastVisibleLine = true
            }

            let key = CellKey(row: row, col: colIdx)
            let isMarkedForDeletion = appVM.results.pendingDeletions.contains(row)
            let isDirty = appVM.results.dirtyCells[key] != nil
            let value = isDirty ? appVM.results.dirtyCells[key]! : result.rows[row][colIdx]

            cellView.toolTip = value

            if isMarkedForDeletion {
                // Strikethrough + red for rows pending deletion
                let attrs: [NSAttributedString.Key: Any] = [
                    .strikethroughStyle: NSUnderlineStyle.single.rawValue,
                    .foregroundColor: NSColor(SbqlTheme.Colors.danger),
                    .font: NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
                ]
                cellView.attributedStringValue = NSAttributedString(string: value, attributes: attrs)
                cellView.drawsBackground = true
                cellView.backgroundColor = NSColor(SbqlTheme.Colors.danger).withAlphaComponent(0.10)
            } else if isDirty {
                cellView.attributedStringValue = NSAttributedString(string: value)
                cellView.stringValue = value
                cellView.textColor = NSColor(SbqlTheme.Colors.warning)
                cellView.drawsBackground = true
                cellView.backgroundColor = NSColor(SbqlTheme.Colors.warning).withAlphaComponent(0.15)
            } else {
                cellView.attributedStringValue = NSAttributedString(string: value)
                cellView.stringValue = value
                cellView.textColor = NSColor(SbqlTheme.Colors.textPrimary)
                cellView.drawsBackground = false
                cellView.backgroundColor = .clear
            }

            return cellView
        }

        func tableView(_ tableView: NSTableView, rowViewForRow row: Int) -> NSTableRowView? {
            let rowView = SbqlTableRowView()
            return rowView
        }

        // MARK: - Double-click editing

        @objc func handleDoubleClick(_ sender: NSTableView) {
            let row = sender.clickedRow
            let col = sender.clickedColumn
            guard row >= 0, col >= 0 else { return }

            let result = appVM.results.currentResult
            guard row < result.rows.count, col < result.columns.count else { return }

            let pks = appVM.results.primaryKeys
            guard appVM.results.activeSchema != nil,
                  appVM.results.activeTable != nil,
                  !pks.isEmpty else {
                appVM.showToast("Cannot edit: no primary key info")
                return
            }

            guard let pkCol = pks.first,
                  result.columns.contains(pkCol) else {
                appVM.showToast("Cannot edit: PK column not in result")
                return
            }

            let targetCol = result.columns[col]
            let currentVal: String
            if let dirtyVal = appVM.results.dirtyCells[CellKey(row: row, col: col)] {
                currentVal = dirtyVal
            } else {
                currentVal = result.rows[row][col]
            }

            // Get the cell rect for popover positioning
            let cellRect = sender.frameOfCell(atColumn: col, row: row)

            // Create the popover
            let popover = NSPopover()
            popover.behavior = .transient
            popover.contentSize = NSSize(width: 320, height: 160)

            let editorView = CellEditor(column: targetCol, currentValue: currentVal) { [weak self] newVal in
                guard let self else { return }
                popover.close()
                let key = CellKey(row: row, col: col)
                self.appVM.results.dirtyCells[key] = newVal
                self.tableView?.reloadData(forRowIndexes: IndexSet(integer: row),
                                           columnIndexes: IndexSet(integer: col))
            }

            let hostingController = NSHostingController(rootView:
                editorView
                    .environment(\.colorScheme, .dark)
            )
            popover.contentViewController = hostingController
            popover.show(relativeTo: cellRect, of: sender, preferredEdge: .maxY)
        }

        // MARK: - Context menu (NSMenuDelegate)

        func menuNeedsUpdate(_ menu: NSMenu) {
            menu.removeAllItems()
            guard let tableView else { return }

            let row = tableView.clickedRow
            guard row >= 0 else { return }

            let pks = appVM.results.primaryKeys
            guard !pks.isEmpty else { return }

            let isMarked = appVM.results.pendingDeletions.contains(row)
            let title = isMarked ? "Undo Delete" : "Delete Row"
            let item = NSMenuItem(title: title, action: #selector(toggleDeleteRow(_:)), keyEquivalent: "")
            item.target = self
            item.tag = row
            menu.addItem(item)
        }

        @objc func toggleDeleteRow(_ sender: NSMenuItem) {
            let row = sender.tag
            if appVM.results.pendingDeletions.contains(row) {
                appVM.results.pendingDeletions.remove(row)
            } else {
                appVM.results.pendingDeletions.insert(row)
            }
            tableView?.reloadData(forRowIndexes: IndexSet(integer: row),
                                  columnIndexes: IndexSet(0..<(tableView?.numberOfColumns ?? 0)))
        }

        // MARK: - Sort

        func sortByColumn(_ colIndex: Int) {
            let result = appVM.results.currentResult
            guard colIndex < result.columns.count else { return }
            let colName = result.columns[colIndex]

            let direction: FfiSortDirection
            if appVM.results.sortedColumn == colName && appVM.results.sortDirection == .ascending {
                direction = .descending
            } else {
                direction = .ascending
            }

            appVM.results.sortedColumn = colName
            appVM.results.sortDirection = direction

            Task { @MainActor in
                await appVM.applyOrder(column: colName, direction: direction)
            }
        }
    }
}

// MARK: - Custom Row View

private class SbqlTableRowView: NSTableRowView {
    override func drawSelection(in dirtyRect: NSRect) {
        NSColor(SbqlTheme.Colors.selection).setFill()
        dirtyRect.fill()
    }

    override var backgroundColor: NSColor {
        get { NSColor(SbqlTheme.Colors.background) }
        set { _ = newValue }
    }
}

// MARK: - Custom Header View

private class SbqlTableHeaderView: NSTableHeaderView {
    override func mouseDown(with event: NSEvent) {
        let location = convert(event.locationInWindow, from: nil)
        let col = column(at: location)
        guard col >= 0, let tableView else {
            super.mouseDown(with: event)
            return
        }

        // If the click is near the right edge of the column, let super handle resize
        let colRect = headerRect(ofColumn: col)
        let resizeMargin: CGFloat = 4
        if location.x >= colRect.maxX - resizeMargin {
            super.mouseDown(with: event)
            return
        }
        // Also check the left edge (resize handle of previous column)
        if location.x <= colRect.minX + resizeMargin {
            super.mouseDown(with: event)
            return
        }

        guard let cell = tableView.tableColumns[col].headerCell as? SbqlHeaderCell else {
            super.mouseDown(with: event)
            return
        }
        cell.coordinator?.sortByColumn(cell.colIndex)
    }

    override func draw(_ dirtyRect: NSRect) {
        // Custom background
        NSColor(SbqlTheme.Colors.surface).setFill()
        bounds.fill()

        // Draw each column's header cell manually (since we skip super)
        if let tableView {
            for idx in 0..<tableView.tableColumns.count {
                let cellRect = headerRect(ofColumn: idx)
                if dirtyRect.intersects(cellRect) {
                    tableView.tableColumns[idx].headerCell.draw(withFrame: cellRect, in: self)
                }
            }
        }

        // Bottom border
        let borderRect = NSRect(x: 0, y: bounds.height - 1, width: bounds.width, height: 1)
        NSColor(SbqlTheme.Colors.border).setFill()
        borderRect.fill()
    }
}

// MARK: - Custom Header Cell (clickable for sort)

private class SbqlHeaderCell: NSTableHeaderCell {
    weak var coordinator: ResultsTableView.Coordinator?
    var colIndex: Int = 0

    convenience init(_ title: String, coordinator: ResultsTableView.Coordinator, colIndex: Int) {
        self.init(textCell: title)
        self.coordinator = coordinator
        self.colIndex = colIndex
        self.font = NSFont.systemFont(ofSize: 11, weight: .semibold)
        self.textColor = NSColor(SbqlTheme.Colors.textSecondary)
    }

    override func trackMouse(with event: NSEvent, in cellFrame: NSRect, of controlView: NSView, untilMouseUp flag: Bool) -> Bool {
        coordinator?.sortByColumn(colIndex)
        return true
    }

    override func draw(withFrame cellFrame: NSRect, in controlView: NSView) {
        // Background
        NSColor(SbqlTheme.Colors.surface).setFill()
        cellFrame.fill()

        // Text
        let titleStr = stringValue
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 11, weight: .semibold),
            .foregroundColor: NSColor(SbqlTheme.Colors.textSecondary)
        ]
        let size = titleStr.size(withAttributes: attrs)
        let textRect = NSRect(
            x: cellFrame.origin.x + 6,
            y: cellFrame.origin.y + (cellFrame.height - size.height) / 2,
            width: cellFrame.width - 20,
            height: size.height
        )
        titleStr.draw(in: textRect, withAttributes: attrs)

        // Sort indicator
        if let coordinator, let appVM = coordinator.appVM as AppViewModel?,
           appVM.results.sortedColumn == titleStr {
            let arrow = appVM.results.sortDirection == .ascending ? "\u{25B2}" : "\u{25BC}"
            let arrowAttrs: [NSAttributedString.Key: Any] = [
                .font: NSFont.systemFont(ofSize: 8),
                .foregroundColor: NSColor(SbqlTheme.Colors.accent)
            ]
            let arrowSize = arrow.size(withAttributes: arrowAttrs)
            let arrowRect = NSRect(
                x: cellFrame.maxX - arrowSize.width - 6,
                y: cellFrame.origin.y + (cellFrame.height - arrowSize.height) / 2,
                width: arrowSize.width,
                height: arrowSize.height
            )
            arrow.draw(in: arrowRect, withAttributes: arrowAttrs)
        }

        // Right border
        let borderRect = NSRect(x: cellFrame.maxX - 1, y: cellFrame.minY, width: 1, height: cellFrame.height)
        NSColor(SbqlTheme.Colors.borderSubtle).setFill()
        borderRect.fill()
    }
}
