import SwiftUI

/// A single table node in the ER diagram with selection, hover, and drag support.
struct DiagramTableNode: View {
    let table: DiagramTable
    let isSelected: Bool
    let isHovered: Bool
    let hoveredFkConstraint: String?
    let fksForTable: [DiagramForeignKey]

    var onSelect: () -> Void = {}
    var onHoverChange: (Bool) -> Void = { _ in }
    var onDragChanged: (CGSize) -> Void = { _ in }
    var onDragEnded: () -> Void = {}

    /// Set of column names highlighted by the currently hovered FK constraint.
    private var highlightedCols: Set<String> {
        guard let constraint = hoveredFkConstraint else { return [] }
        var cols = Set<String>()
        for fk in fksForTable where fk.constraintName == constraint {
            cols.insert(fk.fromCol)
            cols.insert(fk.toCol)
        }
        return cols
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            columnList
        }
        .frame(width: DiagramLayout.nodeWidth)
        .background(SbqlTheme.Colors.surface)
        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
        .overlay(borderOverlay)
        .shadow(
            color: shadowColor,
            radius: shadowRadius,
            y: 2
        )
        .scaleEffect(isHovered ? 1.015 : 1.0)
        .animation(.easeOut(duration: 0.15), value: isHovered)
        .animation(.easeOut(duration: 0.15), value: isSelected)
        .onTapGesture { onSelect() }
        .onHover { hovering in onHoverChange(hovering) }
        .gesture(
            DragGesture()
                .onChanged { value in
                    onDragChanged(value.translation)
                }
                .onEnded { _ in onDragEnded() }
        )
    }

    // MARK: - Header

    private var header: some View {
        HStack(spacing: 0) {
            // Accent left stripe
            SbqlTheme.Colors.accent
                .frame(width: 3)

            HStack(spacing: SbqlTheme.Spacing.xs) {
                Image(systemName: "tablecells")
                    .font(.system(size: 10))
                    .foregroundStyle(SbqlTheme.Colors.accent)

                VStack(alignment: .leading, spacing: 0) {
                    Text(table.name)
                        .font(SbqlTheme.Typography.captionBold)
                        .foregroundStyle(SbqlTheme.Colors.textPrimary)
                        .lineLimit(1)

                    Text(table.schema)
                        .font(.system(size: 9))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                        .lineLimit(1)
                }

                Spacer()
            }
            .padding(.horizontal, SbqlTheme.Spacing.sm)
        }
        .frame(height: DiagramLayout.headerHeight)
        .background(
            LinearGradient(
                colors: [
                    SbqlTheme.Colors.accent.opacity(0.18),
                    SbqlTheme.Colors.accent.opacity(0.06)
                ],
                startPoint: .leading,
                endPoint: .trailing
            )
        )
    }

    // MARK: - Columns

    private var columnList: some View {
        VStack(spacing: 0) {
            ForEach(table.columns) { col in
                columnRow(col)
            }
        }
    }

    private func columnRow(_ col: DiagramColumn) -> some View {
        let isHighlighted = highlightedCols.contains(col.name)

        return HStack(spacing: SbqlTheme.Spacing.xs) {
            if col.isPk {
                Image(systemName: "key.fill")
                    .font(.system(size: 8))
                    .foregroundStyle(SbqlTheme.Colors.warning)
            } else if col.isFk {
                Image(systemName: "link")
                    .font(.system(size: 8))
                    .foregroundStyle(isHighlighted ? SbqlTheme.Colors.accent : SbqlTheme.Colors.textTertiary)
            }

            Text(col.name)
                .font(SbqlTheme.Typography.codeSmall)
                .foregroundStyle(
                    isHighlighted ? SbqlTheme.Colors.accent : SbqlTheme.Colors.textPrimary
                )
                .lineLimit(1)

            Spacer()

            Text(col.dataType)
                .font(.system(size: 9, design: .monospaced))
                .foregroundStyle(SbqlTheme.Colors.textTertiary)
                .lineLimit(1)

            if col.isNullable {
                Text("?")
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.sm)
        .frame(height: DiagramLayout.rowHeight)
        .background(isHighlighted ? SbqlTheme.Colors.accent.opacity(0.08) : Color.clear)
    }

    // MARK: - Visual Effects

    private var borderOverlay: some View {
        RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium)
            .stroke(
                isSelected ? SbqlTheme.Colors.accent :
                    isHovered ? SbqlTheme.Colors.accent.opacity(0.4) :
                    SbqlTheme.Colors.border,
                lineWidth: isSelected ? 1.5 : 1
            )
    }

    private var shadowColor: Color {
        if isSelected {
            return SbqlTheme.Colors.accent.opacity(0.3)
        } else if isHovered {
            return Color.black.opacity(0.35)
        }
        return Color.black.opacity(0.2)
    }

    private var shadowRadius: CGFloat {
        if isSelected { return 8 }
        if isHovered { return 6 }
        return 4
    }
}
