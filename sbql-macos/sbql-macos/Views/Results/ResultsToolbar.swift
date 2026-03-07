import SwiftUI

struct ResultsToolbar: View {
    @Environment(AppViewModel.self) private var appVM

    var body: some View {
        HStack(spacing: SbqlTheme.Spacing.sm) {
            // Commit / Discard pending edits
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
                .buttonStyle(.plain)

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
                .buttonStyle(.plain)

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
                        : SbqlTheme.Colors.textSecondary
                )
            }
            .buttonStyle(.plain)
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
                    .foregroundStyle(SbqlTheme.Colors.textSecondary)
                }
                .buttonStyle(.plain)
            }

            // Editor toggle
            Button {
                withAnimation(SbqlTheme.Animations.quick) {
                    appVM.editor.isVisible.toggle()
                }
            } label: {
                HStack(spacing: SbqlTheme.Spacing.xs) {
                    Image(systemName: "chevron.left.forwardslash.chevron.right")
                        .font(.system(size: 11))
                    Text("Editor")
                        .font(SbqlTheme.Typography.captionBold)
                }
                .foregroundStyle(
                    appVM.editor.isVisible
                        ? SbqlTheme.Colors.accent
                        : SbqlTheme.Colors.textSecondary
                )
            }
            .buttonStyle(.plain)
            .keyboardShortcut("e", modifiers: .command)

            Spacer()

            // Row count
            Text("\(appVM.results.currentResult.rowCount) rows")
                .font(SbqlTheme.Typography.caption)
                .foregroundStyle(SbqlTheme.Colors.textTertiary)

            // Pagination
            HStack(spacing: SbqlTheme.Spacing.xs) {
                Button {
                    let page = appVM.results.currentResult.page
                    if page > 0 {
                        Task { await appVM.fetchPage(page - 1) }
                    }
                } label: {
                    Image(systemName: "chevron.left")
                        .font(.system(size: 10))
                }
                .buttonStyle(.plain)
                .disabled(appVM.results.currentResult.page == 0)

                Text(appVM.results.pageDisplay)
                    .font(SbqlTheme.Typography.caption)
                    .foregroundStyle(SbqlTheme.Colors.textSecondary)

                Button {
                    let page = appVM.results.currentResult.page
                    Task { await appVM.fetchPage(page + 1) }
                } label: {
                    Image(systemName: "chevron.right")
                        .font(.system(size: 10))
                }
                .buttonStyle(.plain)
                .disabled(!appVM.results.currentResult.hasNextPage)
            }
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.vertical, SbqlTheme.Spacing.sm)
        .background(SbqlTheme.Colors.surface)
        .overlay(alignment: .bottom) {
            SbqlTheme.Colors.border.frame(height: 1)
        }
    }
}
