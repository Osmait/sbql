import AppKit
import SwiftUI

// MARK: - Data types

enum CompletionKind {
    case table, column, keyword

    var icon: String {
        switch self {
        case .table: "tablecells"
        case .column: "character.textbox"
        case .keyword: "textformat"
        }
    }

    var color: Color {
        switch self {
        case .table: SbqlTheme.Colors.accent
        case .column: SbqlTheme.Colors.warning
        case .keyword: SbqlTheme.Colors.textTertiary
        }
    }
}

struct CompletionItem: Identifiable, Equatable {
    let id = UUID()
    let text: String
    let detail: String
    let kind: CompletionKind

    static func == (lhs: CompletionItem, rhs: CompletionItem) -> Bool {
        lhs.id == rhs.id
    }
}

// MARK: - Provider

enum SQLCompletionProvider {
    private static let sqlKeywords = [
        "SELECT", "FROM", "WHERE", "INSERT", "INTO", "VALUES", "UPDATE", "SET",
        "DELETE", "CREATE", "TABLE", "ALTER", "DROP", "INDEX", "JOIN", "INNER",
        "LEFT", "RIGHT", "OUTER", "CROSS", "ON", "AND", "OR", "NOT", "IN",
        "EXISTS", "BETWEEN", "LIKE", "IS", "NULL", "AS", "ORDER", "BY", "ASC",
        "DESC", "GROUP", "HAVING", "LIMIT", "OFFSET", "UNION", "ALL", "DISTINCT",
        "CASE", "WHEN", "THEN", "ELSE", "END", "COUNT", "SUM", "AVG", "MIN",
        "MAX", "CAST", "COALESCE", "PRIMARY", "KEY", "FOREIGN", "REFERENCES",
        "CONSTRAINT", "DEFAULT", "CHECK", "UNIQUE", "TRUNCATE", "BEGIN",
        "COMMIT", "ROLLBACK", "GRANT", "REVOKE", "WITH", "RECURSIVE", "EXPLAIN",
        "ANALYZE", "VACUUM", "PRAGMA",
    ]

    static func completions(prefix: String, tables: [DiagramTable]) -> [CompletionItem] {
        let upper = prefix.uppercased()
        let lower = prefix.lowercased()

        // Table matches
        let tableItems = tables
            .filter { $0.name.lowercased().hasPrefix(lower) }
            .map { CompletionItem(text: $0.name, detail: $0.schema, kind: .table) }

        // Column matches (deduplicated by name)
        var seenColumns = Set<String>()
        var columnItems = [CompletionItem]()
        for table in tables {
            for col in table.columns where col.name.lowercased().hasPrefix(lower) {
                if seenColumns.insert(col.name).inserted {
                    columnItems.append(
                        CompletionItem(text: col.name, detail: "\(table.name) · \(col.dataType)", kind: .column)
                    )
                }
            }
        }

        // Keyword matches
        let keywordItems = sqlKeywords
            .filter { $0.hasPrefix(upper) }
            .map { CompletionItem(text: $0, detail: "keyword", kind: .keyword) }

        // Sort: tables → columns → keywords, shorter first within each group
        let sorted = (tableItems.sorted { $0.text.count < $1.text.count })
            + (columnItems.sorted { $0.text.count < $1.text.count })
            + (keywordItems.sorted { $0.text.count < $1.text.count })

        return Array(sorted.prefix(10))
    }
}

// MARK: - SwiftUI list

private struct CompletionListView: View {
    let items: [CompletionItem]
    let selectedIndex: Int
    let onAccept: (CompletionItem) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                HStack(spacing: SbqlTheme.Spacing.sm) {
                    Image(systemName: item.kind.icon)
                        .font(.system(size: 11))
                        .foregroundStyle(item.kind.color)
                        .frame(width: 16, alignment: .center)

                    Text(item.text)
                        .font(.system(size: 12, weight: .medium, design: .monospaced))
                        .foregroundStyle(SbqlTheme.Colors.textPrimary)

                    Spacer()

                    Text(item.detail)
                        .font(.system(size: 10))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                }
                .padding(.horizontal, SbqlTheme.Spacing.sm)
                .padding(.vertical, SbqlTheme.Spacing.xs)
                .background(
                    index == selectedIndex
                        ? SbqlTheme.Colors.selection
                        : Color.clear
                )
                .contentShape(Rectangle())
                .onTapGesture { onAccept(item) }
            }
        }
        .padding(.vertical, SbqlTheme.Spacing.xs)
        .background(SbqlTheme.Colors.surfaceElevated)
        .overlay(
            RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium)
                .stroke(SbqlTheme.Colors.border, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
    }
}

// MARK: - NSPanel wrapper

final class CompletionPanel {
    private let panel: NSPanel
    private let hostingView: NSHostingView<AnyView>

    private(set) var items: [CompletionItem] = []
    private(set) var selectedIndex: Int = 0
    var onAccept: ((CompletionItem) -> Void)?

    var isVisible: Bool {
        panel.isVisible
    }

    init() {
        panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 320, height: 10),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: true
        )
        panel.level = .popUpMenu
        panel.isFloatingPanel = true
        panel.hasShadow = true
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hidesOnDeactivate = true

        hostingView = NSHostingView(rootView: AnyView(EmptyView()))
        panel.contentView = hostingView
    }

    func show(items: [CompletionItem], at screenPoint: NSPoint) {
        guard !items.isEmpty else { dismiss(); return }
        self.items = items
        selectedIndex = 0
        rebuildView()

        // Size the panel to fit content
        let height = min(CGFloat(items.count) * 26 + 8, 280)
        let frame = NSRect(x: screenPoint.x, y: screenPoint.y - height, width: 320, height: height)
        panel.setFrame(frame, display: true)
        panel.orderFront(nil)
    }

    func dismiss() {
        panel.orderOut(nil)
        items = []
        selectedIndex = 0
    }

    func moveUp() {
        guard !items.isEmpty else { return }
        selectedIndex = (selectedIndex - 1 + items.count) % items.count
        rebuildView()
    }

    func moveDown() {
        guard !items.isEmpty else { return }
        selectedIndex = (selectedIndex + 1) % items.count
        rebuildView()
    }

    func acceptSelected() {
        guard !items.isEmpty else { return }
        onAccept?(items[selectedIndex])
    }

    private func rebuildView() {
        let idx = selectedIndex
        let currentItems = items
        let accept = onAccept
        hostingView.rootView = AnyView(
            CompletionListView(items: currentItems, selectedIndex: idx) { item in
                accept?(item)
            }
        )
    }
}
