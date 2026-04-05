import AppKit
import SwiftUI

/// High-performance NSTableView wrapper with virtualized rows and column sorting.
struct ResultsTableView: NSViewRepresentable {
    /// Passed in so SwiftUI diffs it and calls updateNSView on theme change.
    var activeTheme: ThemeName
    @Environment(AppViewModel.self) private var appVM

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = false

        let tableView = NSTableView()
        tableView.style = .plain
        tableView.backgroundColor = NSColor(SbqlTheme.Colors.surface)
        tableView.rowHeight = SbqlTheme.Size.rowHeight
        tableView.intercellSpacing = NSSize(width: 16, height: 1)
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
        let _ = activeTheme // SwiftUI triggers updateNSView when this changes

        // Update theme colors on all layers
        let bg = NSColor(SbqlTheme.Colors.surface)
        scrollView.drawsBackground = false // let SwiftUI island background show through
        scrollView.contentView.drawsBackground = false

        // Cache theme colors once per update cycle
        context.coordinator.cachedTextPrimary = NSColor(SbqlTheme.Colors.textPrimary)
        context.coordinator.cachedDanger = NSColor(SbqlTheme.Colors.danger)
        context.coordinator.cachedWarning = NSColor(SbqlTheme.Colors.warning)
        context.coordinator.cachedSuccess = NSColor(SbqlTheme.Colors.success)

        if let tableView = context.coordinator.tableView {
            tableView.backgroundColor = bg
            tableView.gridColor = NSColor(SbqlTheme.Colors.borderSubtle)

            // Only rebuild columns when they actually change
            let currentColumns = appVM.results.currentResult.columns
            if currentColumns != context.coordinator.previousColumns {
                context.coordinator.rebuildColumns()
                context.coordinator.previousColumns = currentColumns
            }
            tableView.reloadData()
            tableView.headerView?.needsDisplay = true
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(appVM: appVM)
    }

    class Coordinator: NSObject, NSTableViewDelegate, NSTableViewDataSource, NSMenuDelegate, NSTextFieldDelegate {
        var appVM: AppViewModel
        weak var tableView: NSTableView?
        var previousColumns: [String] = []

        // Cached theme colors to avoid repeated SbqlTheme lookups per cell
        var cachedTextPrimary: NSColor = .white
        var cachedDanger: NSColor = .red
        var cachedWarning: NSColor = .yellow
        var cachedSuccess: NSColor = .green

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
            let headerFont = NSFont.systemFont(ofSize: 11, weight: .semibold)
            let cellFont = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
            let sampleCount = min(result.rows.count, 50)

            for (idx, colName) in result.columns.enumerated() {
                let col = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("col_\(idx)"))
                col.title = colName
                col.minWidth = 100

                // Measure header width (+ sort indicator space)
                let headerWidth = (colName as NSString).size(withAttributes: [.font: headerFont]).width + 26

                // Sample cell content to find max width
                var maxCellWidth: CGFloat = 0
                for rowIdx in 0 ..< sampleCount {
                    if idx < result.rows[rowIdx].count {
                        let cellWidth = (result.rows[rowIdx][idx] as NSString)
                            .size(withAttributes: [.font: cellFont]).width
                        maxCellWidth = max(maxCellWidth, cellWidth)
                    }
                }

                col.width = max(100, max(headerWidth, maxCellWidth) + 24)
                col.headerCell = SbqlHeaderCell(colName, coordinator: self, colIndex: idx)
                tableView.addTableColumn(col)
            }
        }

        // MARK: - NSTableViewDataSource

        func numberOfRows(in _: NSTableView) -> Int {
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
            let dirtyValue = appVM.results.dirtyCells[key]
            let isDirty = dirtyValue != nil
            let value = dirtyValue ?? result.rows[row][colIdx]

            cellView.toolTip = value

            if isMarkedForDeletion {
                // Strikethrough + red for rows pending deletion
                let attrs: [NSAttributedString.Key: Any] = [
                    .strikethroughStyle: NSUnderlineStyle.single.rawValue,
                    .foregroundColor: cachedDanger,
                    .font: NSFont.monospacedSystemFont(ofSize: 12, weight: .regular),
                ]
                cellView.attributedStringValue = NSAttributedString(string: value, attributes: attrs)
                cellView.drawsBackground = true
                cellView.backgroundColor = cachedDanger.withAlphaComponent(0.10)
            } else if isDirty {
                cellView.stringValue = value
                cellView.textColor = cachedWarning
                cellView.drawsBackground = true
                cellView.backgroundColor = cachedWarning.withAlphaComponent(0.15)
            } else {
                cellView.stringValue = value
                cellView.textColor = cachedTextPrimary
                cellView.drawsBackground = false
                cellView.backgroundColor = .clear
            }

            // Diff mode highlighting
            if appVM.results.isDiffMode, let diff = appVM.results.diffResult {
                if diff.addedRows.contains(row) {
                    cellView.textColor = cachedSuccess
                    cellView.drawsBackground = true
                    cellView.backgroundColor = cachedSuccess.withAlphaComponent(0.12)
                } else if let change = diff.changedCells[CellKey(row: row, col: colIdx)] {
                    cellView.textColor = cachedWarning
                    cellView.drawsBackground = true
                    cellView.backgroundColor = cachedWarning.withAlphaComponent(0.15)
                    cellView.toolTip = "Was: \(change.old)"
                }
            }

            return cellView
        }

        func tableView(_: NSTableView, rowViewForRow row: Int) -> NSTableRowView? {
            SbqlTableRowView(rowIndex: row)
        }

        // MARK: - Double-click inline editing

        @objc func handleDoubleClick(_ sender: NSTableView) {
            let row = sender.clickedRow
            let col = sender.clickedColumn
            guard row >= 0, col >= 0 else { return }

            let result = appVM.results.currentResult
            guard row < result.rows.count, col < result.columns.count else { return }

            let pks = appVM.results.primaryKeys
            guard appVM.results.activeSchema != nil,
                  appVM.results.activeTable != nil,
                  !pks.isEmpty
            else {
                appVM.showToast("Cannot edit: no primary key info")
                return
            }

            guard let pkCol = pks.first,
                  result.columns.contains(pkCol)
            else {
                appVM.showToast("Cannot edit: PK column not in result")
                return
            }

            let currentVal: String = if let dirtyVal = appVM.results.dirtyCells[CellKey(row: row, col: col)] {
                dirtyVal
            } else {
                result.rows[row][col]
            }

            // Get the cell view and make it editable inline
            guard let cellView = sender.view(atColumn: col, row: row, makeIfNecessary: false) as? NSTextField else { return }

            cellView.isEditable = true
            cellView.isSelectable = true
            cellView.isBordered = true
            cellView.bezelStyle = .roundedBezel
            cellView.drawsBackground = true
            cellView.backgroundColor = NSColor(SbqlTheme.Colors.surfaceElevated)
            cellView.textColor = NSColor(SbqlTheme.Colors.textPrimary)
            cellView.currentEditor()?.selectedRange = NSRange(location: 0, length: currentVal.count)
            cellView.delegate = self
            cellView.tag = row * 10000 + col // encode row+col in tag
            cellView.window?.makeFirstResponder(cellView)
        }

        // MARK: - NSTextFieldDelegate for inline editing

        func controlTextDidEndEditing(_ obj: Notification) {
            guard let textField = obj.object as? NSTextField else { return }
            let row = textField.tag / 10000
            let col = textField.tag % 10000

            let newVal = textField.stringValue
            let result = appVM.results.currentResult

            // Check if value actually changed
            let oldVal = appVM.results.dirtyCells[CellKey(row: row, col: col)]
                ?? (row < result.rows.count && col < result.rows[row].count ? result.rows[row][col] : "")

            if newVal != oldVal {
                appVM.results.dirtyCells[CellKey(row: row, col: col)] = newVal
            }

            // Reset cell to non-editable label style
            textField.isEditable = false
            textField.isSelectable = false
            textField.isBordered = false
            textField.bezelStyle = .squareBezel
            textField.drawsBackground = false

            // Reload the row to apply dirty styling
            tableView?.reloadData(forRowIndexes: IndexSet(integer: row),
                                  columnIndexes: IndexSet(integer: col))
        }

        // MARK: - Context menu (NSMenuDelegate)

        func menuNeedsUpdate(_ menu: NSMenu) {
            menu.removeAllItems()
            guard let tableView else { return }

            let row = tableView.clickedRow
            let col = tableView.clickedColumn
            guard row >= 0 else { return }

            let result = appVM.results.currentResult
            guard row < result.rows.count else { return }

            // Copy Cell Value
            if col >= 0, col < result.columns.count {
                let cellItem = NSMenuItem(title: "Copy Cell Value", action: #selector(copyCellValue(_:)), keyEquivalent: "")
                cellItem.target = self
                cellItem.tag = row
                cellItem.representedObject = col
                menu.addItem(cellItem)
            }

            // Copy Row as JSON
            let jsonItem = NSMenuItem(title: "Copy Row as JSON", action: #selector(copyRowAsJSON(_:)), keyEquivalent: "")
            jsonItem.target = self
            jsonItem.tag = row
            menu.addItem(jsonItem)

            // Copy Row as INSERT
            let insertItem = NSMenuItem(title: "Copy Row as INSERT", action: #selector(copyRowAsInsert(_:)), keyEquivalent: "")
            insertItem.target = self
            insertItem.tag = row
            menu.addItem(insertItem)

            // Delete row (requires PKs)
            let pks = appVM.results.primaryKeys
            if !pks.isEmpty {
                menu.addItem(NSMenuItem.separator())
                let isMarked = appVM.results.pendingDeletions.contains(row)
                let title = isMarked ? "Undo Delete" : "Delete Row"
                let item = NSMenuItem(title: title, action: #selector(toggleDeleteRow(_:)), keyEquivalent: "")
                item.target = self
                item.tag = row
                menu.addItem(item)
            }
        }

        @objc func copyCellValue(_ sender: NSMenuItem) {
            let row = sender.tag
            guard let col = sender.representedObject as? Int else { return }
            let result = appVM.results.currentResult
            guard row < result.rows.count, col < result.rows[row].count else { return }
            let value = appVM.results.dirtyCells[CellKey(row: row, col: col)] ?? result.rows[row][col]
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(value, forType: .string)
        }

        @objc func copyRowAsJSON(_ sender: NSMenuItem) {
            let row = sender.tag
            let result = appVM.results.currentResult
            guard row < result.rows.count else { return }
            var obj: [String: Any] = [:]
            for (i, col) in result.columns.enumerated() {
                let val = i < result.rows[row].count ? result.rows[row][i] : ""
                if val.isEmpty { obj[col] = NSNull() }
                else if let n = Int(val) { obj[col] = n }
                else if let d = Double(val), val.contains(".") { obj[col] = d }
                else if val == "true" { obj[col] = true }
                else if val == "false" { obj[col] = false }
                else { obj[col] = val }
            }
            if let data = try? JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys]),
               let json = String(data: data, encoding: .utf8) {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(json, forType: .string)
            }
        }

        @objc func copyRowAsInsert(_ sender: NSMenuItem) {
            let row = sender.tag
            let result = appVM.results.currentResult
            guard row < result.rows.count else { return }
            let tableName = appVM.results.activeTable ?? "table"
            let cols = result.columns.map { "\"\($0)\"" }.joined(separator: ", ")
            let vals = result.rows[row].map { val -> String in
                if val.isEmpty { return "NULL" }
                if Double(val) != nil { return val }
                if val == "true" || val == "false" { return val.uppercased() }
                return "'\(val.replacingOccurrences(of: "'", with: "''"))'"
            }.joined(separator: ", ")
            let sql = "INSERT INTO \"\(tableName)\" (\(cols)) VALUES (\(vals));"
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(sql, forType: .string)
        }

        @objc func toggleDeleteRow(_ sender: NSMenuItem) {
            let row = sender.tag
            if appVM.results.pendingDeletions.contains(row) {
                appVM.results.pendingDeletions.remove(row)
            } else {
                appVM.results.pendingDeletions.insert(row)
            }
            tableView?.reloadData(forRowIndexes: IndexSet(integer: row),
                                  columnIndexes: IndexSet(0 ..< (tableView?.numberOfColumns ?? 0)))
        }

        // MARK: - Sort

        func sortByColumn(_ colIndex: Int) {
            let result = appVM.results.currentResult
            guard colIndex < result.columns.count else { return }
            let colName = result.columns[colIndex]

            let direction: FfiSortDirection = if appVM.results.sortedColumn == colName, appVM.results.sortDirection == .ascending {
                .descending
            } else {
                .ascending
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
    private static var evenColor: NSColor { NSColor(SbqlTheme.Colors.surface) }
    private static var oddColor: NSColor {
        // Slightly different from surface for alternating rows
        let base = NSColor(SbqlTheme.Colors.surface)
        let elevated = NSColor(SbqlTheme.Colors.surfaceElevated)
        return base.blended(withFraction: 0.3, of: elevated) ?? elevated
    }

    private let rowIndex: Int

    init(rowIndex: Int) {
        self.rowIndex = rowIndex
        super.init(frame: .zero)
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func drawSelection(in dirtyRect: NSRect) {
        NSColor(SbqlTheme.Colors.selection).setFill()
        dirtyRect.fill()
    }

    override func drawBackground(in dirtyRect: NSRect) {
        let color = rowIndex % 2 == 0 ? Self.evenColor : Self.oddColor
        color.setFill()
        dirtyRect.fill()
    }

    override var backgroundColor: NSColor {
        get { rowIndex % 2 == 0 ? Self.evenColor : Self.oddColor }
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
            for idx in 0 ..< tableView.tableColumns.count {
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
        font = NSFont.systemFont(ofSize: 11, weight: .semibold)
        textColor = NSColor(SbqlTheme.Colors.textSecondary)
    }

    override func trackMouse(with _: NSEvent, in _: NSRect, of _: NSView, untilMouseUp _: Bool) -> Bool {
        coordinator?.sortByColumn(colIndex)
        return true
    }

    override func draw(withFrame cellFrame: NSRect, in _: NSView) {
        // Background
        NSColor(SbqlTheme.Colors.surface).setFill()
        cellFrame.fill()

        // Text
        let titleStr = stringValue
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 11, weight: .semibold),
            .foregroundColor: NSColor(SbqlTheme.Colors.textSecondary),
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
           appVM.results.sortedColumn == titleStr
        {
            let arrow = appVM.results.sortDirection == .ascending ? "\u{25B2}" : "\u{25BC}"
            let arrowAttrs: [NSAttributedString.Key: Any] = [
                .font: NSFont.systemFont(ofSize: 8),
                .foregroundColor: NSColor(SbqlTheme.Colors.accent),
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
