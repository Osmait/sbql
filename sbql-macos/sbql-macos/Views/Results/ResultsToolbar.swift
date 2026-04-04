import SwiftUI

struct ResultsToolbar: View {
    @Environment(AppViewModel.self) private var appVM

    var body: some View {
        HStack(spacing: SbqlTheme.Spacing.sm) {
            // Commit / Discard pending edits (animated)
            if appVM.results.hasPendingEdits {
                Button {
                    Task { await appVM.commitEdits() }
                } label: {
                    HStack(spacing: SbqlTheme.Spacing.xs) {
                        Image(systemName: "checkmark.circle")
                            .font(.system(size: 11))
                        Text("Commit")
                            .font(SbqlTheme.Typography.captionBold)
                        if !appVM.results.pendingDeletions.isEmpty {
                            Text("(\(appVM.results.pendingDeletions.count) delete\(appVM.results.pendingDeletions.count == 1 ? "" : "s"))")
                                .font(SbqlTheme.Typography.caption)
                                .foregroundStyle(SbqlTheme.Colors.danger)
                        }
                    }
                    .foregroundStyle(SbqlTheme.Colors.accent)
                }
                .buttonStyle(.hover)

                Button {
                    appVM.discardEdits()
                } label: {
                    HStack(spacing: SbqlTheme.Spacing.xs) {
                        Image(systemName: "xmark.circle")
                            .font(.system(size: 11))
                        Text("Discard")
                            .font(SbqlTheme.Typography.caption)
                    }
                    .foregroundStyle(SbqlTheme.Colors.textSecondary)
                }
                .buttonStyle(.hover)

                Divider()
                    .frame(height: 14)
            }

            // Filter toggle
            Button {
                withAnimation(SbqlTheme.Animations.quick) {
                    appVM.results.isFilterBarVisible.toggle()
                }
            } label: {
                HStack(spacing: SbqlTheme.Spacing.xs) {
                    Image(systemName: "line.3.horizontal.decrease")
                        .font(.system(size: 11))
                    Text("Filter")
                        .font(SbqlTheme.Typography.captionBold)
                }
                .foregroundStyle(
                    appVM.results.isFilterBarVisible
                        ? SbqlTheme.Colors.accent
                        : SbqlTheme.Colors.accent.opacity(0.4)
                )
            }
            .buttonStyle(.hover)
            .keyboardShortcut("f", modifiers: .command)

            if appVM.results.sortedColumn != nil {
                Button {
                    Task { await appVM.clearOrder() }
                    appVM.results.sortedColumn = nil
                } label: {
                    HStack(spacing: SbqlTheme.Spacing.xs) {
                        Image(systemName: "xmark")
                            .font(.system(size: 9))
                        Text("Clear sort")
                            .font(SbqlTheme.Typography.caption)
                    }
                    .foregroundStyle(SbqlTheme.Colors.warning)
                }
                .buttonStyle(.hover)
            }

            // SQL Editor toggle
            Button {
                withAnimation(SbqlTheme.Animations.quick) {
                    appVM.editor.isVisible.toggle()
                }
            } label: {
                HStack(spacing: SbqlTheme.Spacing.xs) {
                    Text("SQL")
                        .font(.system(size: 9, weight: .heavy, design: .monospaced))
                    Image(systemName: appVM.editor.isVisible ? "chevron.up" : "chevron.down")
                        .font(.system(size: 8, weight: .bold))
                }
                .foregroundStyle(
                    appVM.editor.isVisible
                        ? SbqlTheme.Colors.accent
                        : SbqlTheme.Colors.accent.opacity(0.5)
                )
                .padding(.horizontal, SbqlTheme.Spacing.sm)
                .padding(.vertical, SbqlTheme.Spacing.xs)
                .background(SbqlTheme.Colors.accent.opacity(appVM.editor.isVisible ? 0.15 : 0.06))
                .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
            }
            .buttonStyle(.hover)
            .keyboardShortcut("e", modifiers: .command)
            .help(appVM.editor.isVisible ? "Hide SQL editor" : "Show SQL editor")

            Spacer()

            // Export menu
            if !appVM.results.currentResult.isEmpty {
                Menu {
                    Section("Current Page (\(appVM.results.currentResult.rowCount) rows)") {
                        ForEach(ExportFormat.allCases, id: \.self) { format in
                            Button {
                                ResultsExporter.export(
                                    format: format,
                                    columns: appVM.results.currentResult.columns,
                                    rows: appVM.results.currentResult.rows,
                                    tableName: appVM.results.activeTable ?? "export"
                                )
                            } label: {
                                Label(format.rawValue, systemImage: format.icon)
                            }
                        }
                    }
                    Section("All Results (streaming)") {
                        ForEach(ExportFormat.allCases, id: \.self) { format in
                            Button {
                                Task {
                                    await appVM.exportAll(
                                        format: format,
                                        tableName: appVM.results.activeTable ?? "export"
                                    )
                                }
                            } label: {
                                Label("\(format.rawValue) — all rows", systemImage: format.icon)
                            }
                            .disabled(appVM.isExporting)
                        }
                    }
                } label: {
                    HStack(spacing: SbqlTheme.Spacing.xs) {
                        if appVM.isExporting {
                            ProgressView()
                                .controlSize(.small)
                                .scaleEffect(0.7)
                        }
                        Image(systemName: "square.and.arrow.up")
                            .font(.system(size: 10))
                        Text(appVM.isExporting ? "Exporting…" : "Export")
                            .font(SbqlTheme.Typography.captionBold)
                    }
                    .foregroundStyle(SbqlTheme.Colors.accent.opacity(appVM.isExporting ? 1.0 : 0.6))
                    .animation(SbqlTheme.Animations.quick, value: appVM.isExporting)
                }
                .menuStyle(.borderlessButton)
                .fixedSize()
            }

            // Snapshot & Diff
            if !appVM.results.currentResult.isEmpty {
                Button {
                    appVM.results.takeSnapshot()
                    appVM.showToast("Snapshot taken")
                } label: {
                    Image(systemName: "camera")
                        .font(.system(size: 10))
                        .foregroundStyle(appVM.results.snapshot != nil ? SbqlTheme.Colors.accent : SbqlTheme.Colors.accent.opacity(0.4))
                }
                .buttonStyle(.hoverIcon)
                .help("Take snapshot for diff")

                if appVM.results.snapshot != nil {
                    Button {
                        if appVM.results.isDiffMode { appVM.results.clearDiff() }
                        else { appVM.results.computeDiff() }
                    } label: {
                        HStack(spacing: 2) {
                            Image(systemName: "arrow.left.arrow.right")
                                .font(.system(size: 10))
                            if appVM.results.isDiffMode, let diff = appVM.results.diffResult {
                                Text(diff.summary)
                                    .font(SbqlTheme.Typography.caption)
                            }
                        }
                        .foregroundStyle(appVM.results.isDiffMode ? SbqlTheme.Colors.warning : SbqlTheme.Colors.accent.opacity(0.5))
                    }
                    .buttonStyle(.hover)
                    .help(appVM.results.isDiffMode ? "Exit diff" : "Compare with snapshot")
                }
            }

            // Row count
            HStack(spacing: 2) {
                Text("\(appVM.results.currentResult.rowCount) rows")
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                if let total = appVM.results.currentResult.totalCount, total > 0 {
                    Text("/ \(total) total")
                        .font(SbqlTheme.Typography.caption)
                        .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.6))
                }
            }
            .padding(.horizontal, SbqlTheme.Spacing.sm)
            .padding(.vertical, SbqlTheme.Spacing.xxs)
            .background(SbqlTheme.Colors.surfaceElevated)
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))

            // Pagination
            HStack(spacing: SbqlTheme.Spacing.xs) {
                Button {
                    let page = appVM.results.currentResult.page
                    if page > 0 {
                        Task { await appVM.fetchPage(page - 1) }
                    }
                } label: {
                    Image(systemName: "chevron.left")
                        .font(.system(size: 12))
                        .foregroundStyle(
                            appVM.results.currentResult.page == 0
                                ? SbqlTheme.Colors.textTertiary
                                : SbqlTheme.Colors.accent
                        )
                        .padding(.horizontal, SbqlTheme.Spacing.xs)
                        .padding(.vertical, SbqlTheme.Spacing.xxs)
                        .background(SbqlTheme.Colors.surfaceElevated)
                        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
                }
                .buttonStyle(.hoverIcon)
                .disabled(appVM.results.currentResult.page == 0)

                Text(appVM.results.pageDisplay)
                    .font(SbqlTheme.Typography.captionBold)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)

                Button {
                    let page = appVM.results.currentResult.page
                    Task { await appVM.fetchPage(page + 1) }
                } label: {
                    Image(systemName: "chevron.right")
                        .font(.system(size: 12))
                        .foregroundStyle(
                            !appVM.results.currentResult.hasNextPage
                                ? SbqlTheme.Colors.textTertiary
                                : SbqlTheme.Colors.accent
                        )
                        .padding(.horizontal, SbqlTheme.Spacing.xs)
                        .padding(.vertical, SbqlTheme.Spacing.xxs)
                        .background(SbqlTheme.Colors.surfaceElevated)
                        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
                }
                .buttonStyle(.hoverIcon)
                .disabled(!appVM.results.currentResult.hasNextPage)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.vertical, SbqlTheme.Spacing.sm)
        .background(SbqlTheme.Colors.surface)
        .overlay(alignment: .bottom) {
            SbqlTheme.Colors.border.frame(height: 1)
        }
        .animation(SbqlTheme.Animations.gentle, value: appVM.results.hasPendingEdits)
        .animation(SbqlTheme.Animations.quick, value: appVM.results.sortedColumn)
    }

}
