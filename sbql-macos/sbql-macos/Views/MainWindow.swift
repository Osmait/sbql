import SwiftUI

/// Top-level layout: custom titlebar + sidebar + main content area.
struct MainWindow: View {
    @Environment(AppViewModel.self) private var appVM

    var body: some View {
        ZStack {
            SbqlTheme.Colors.background.ignoresSafeArea()

            HSplitView {
                SidebarView()
                    .frame(minWidth: 180, idealWidth: SbqlTheme.Size.sidebarWidth, maxWidth: 300)

                mainContent
            }
            .ignoresSafeArea()

            // Toast overlay
            if let message = appVM.toastMessage {
                VStack {
                    Spacer()
                    ToastNotification(message: message, isError: appVM.toastIsError)
                        .padding(.bottom, SbqlTheme.Spacing.xl)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                }
                .animation(SbqlTheme.Animations.spring, value: appVM.toastMessage)
            }
        }
        .frame(minWidth: 800, minHeight: 500)
        .background(WindowAccessor { window in
            window.titlebarAppearsTransparent = true
            window.titleVisibility = .hidden
            window.styleMask.insert(.fullSizeContentView)
        })
        .onAppear { appVM.onAppear() }
    }

    private var mainContent: some View {
        VStack(spacing: 0) {
            unifiedHeader

            switch appVM.activeTab {
            case .query:
                queryContent
            case .diagram:
                DiagramView()
            }
        }
        .background(SbqlTheme.Colors.background)
    }

    private var unifiedHeader: some View {
        VStack(spacing: 0) {
            // Row 1: Titlebar — connection info centered, actions on right
            ZStack {
                // Center: connection name + badges
                connectionInfo

                // Right: action buttons
                HStack {
                    Spacer()
                    headerActions
                }
            }
            .padding(.horizontal, SbqlTheme.Spacing.lg)
            .padding(.top, SbqlTheme.Spacing.xs)
            .padding(.bottom, SbqlTheme.Spacing.xs)

            SbqlTheme.Colors.border.frame(height: 1).opacity(0.5)

            // Row 2: Tab bar — mode pills + table tabs
            HStack(spacing: SbqlTheme.Spacing.xs) {
                // Mode pills
                ForEach(AppViewModel.ActiveTab.allCases, id: \.self) { tab in
                    Button {
                        withAnimation(SbqlTheme.Animations.quick) {
                            appVM.activeTab = tab
                        }
                        if tab == .diagram {
                            Task { await appVM.loadDiagram() }
                        }
                    } label: {
                        Text(tab.rawValue)
                            .font(SbqlTheme.Typography.captionBold)
                            .foregroundStyle(
                                appVM.activeTab == tab
                                    ? SbqlTheme.Colors.textPrimary
                                    : SbqlTheme.Colors.textTertiary
                            )
                            .padding(.horizontal, SbqlTheme.Spacing.md)
                            .padding(.vertical, SbqlTheme.Spacing.xs)
                            .background(
                                appVM.activeTab == tab
                                    ? SbqlTheme.Colors.surfaceElevated
                                    : Color.clear
                            )
                            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
                    }
                    .buttonStyle(.plain)
                }

                // Divider + table tabs
                if appVM.activeTab == .query, !appVM.results.tabs.isEmpty {
                    SbqlTheme.Colors.border
                        .frame(width: 1, height: 16)
                        .padding(.horizontal, SbqlTheme.Spacing.xxs)

                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: SbqlTheme.Spacing.xxs) {
                            ForEach(appVM.results.tabs) { tab in
                                queryTab(tab, isActive: tab.id == appVM.results.activeTabId)
                            }
                        }
                    }
                }

                Spacer()
            }
            .padding(.horizontal, SbqlTheme.Spacing.lg)
            .padding(.vertical, SbqlTheme.Spacing.xs)
        }
        .background(SbqlTheme.Colors.surface)
        .overlay(alignment: .bottom) {
            SbqlTheme.Colors.border.frame(height: 1)
        }
    }

    private var headerActions: some View {
        HStack(spacing: SbqlTheme.Spacing.md) {
            Button {
                Task { await appVM.refreshTables() }
            } label: {
                Image(systemName: "arrow.clockwise")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(SbqlTheme.Colors.textSecondary)
            }
            .buttonStyle(.plain)
        }
    }

    @ViewBuilder
    private var connectionInfo: some View {
        if let conn = appVM.connections.activeConnection {
            HStack(spacing: SbqlTheme.Spacing.sm) {
                // Connection name
                Text(conn.name)
                    .font(SbqlTheme.Typography.bodyMedium)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)

                // Backend badge
                Text(conn.backend == .postgres ? "PG" : "SQLite")
                    .font(SbqlTheme.Typography.captionBold)
                    .foregroundStyle(SbqlTheme.Colors.accent)
                    .padding(.horizontal, SbqlTheme.Spacing.sm)
                    .padding(.vertical, 2)
                    .background(SbqlTheme.Colors.accent.opacity(0.15))
                    .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))

                // Database name
                badgePill(
                    icon: "cylinder",
                    text: conn.backend == .sqlite
                        ? (conn.filePath.flatMap { URL(fileURLWithPath: $0).lastPathComponent } ?? "memory")
                        : conn.database
                )

                // Query duration
                if let d = appVM.editor.lastQueryDuration {
                    badgePill(icon: nil, text: formatDuration(d))
                }
            }
        } else {
            Text("sbql")
                .font(SbqlTheme.Typography.bodyMedium)
                .foregroundStyle(SbqlTheme.Colors.textTertiary)
        }
    }

    private func badgePill(icon: String?, text: String) -> some View {
        HStack(spacing: 3) {
            if let icon {
                Image(systemName: icon)
                    .font(.system(size: 9))
            }
            Text(text)
                .font(SbqlTheme.Typography.captionBold)
        }
        .foregroundStyle(SbqlTheme.Colors.textTertiary)
        .padding(.horizontal, SbqlTheme.Spacing.sm)
        .padding(.vertical, 2)
        .background(SbqlTheme.Colors.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
    }

    private func formatDuration(_ d: Duration) -> String {
        let ms = d.components.seconds * 1000 + d.components.attoseconds / 1_000_000_000_000_000
        if ms < 1 { return "<1ms" }
        if ms < 1000 { return "\(ms)ms" }
        let seconds = Double(ms) / 1000.0
        return String(format: "%.1fs", seconds)
    }

    private func queryTab(_ tab: QueryTab, isActive: Bool) -> some View {
        HStack(spacing: SbqlTheme.Spacing.xs) {
            Image(systemName: "tablecells")
                .font(.system(size: 10))
                .foregroundStyle(isActive ? SbqlTheme.Colors.accent : SbqlTheme.Colors.textTertiary)

            Text(tab.displayName)
                .font(SbqlTheme.Typography.captionBold)
                .foregroundStyle(isActive ? SbqlTheme.Colors.textPrimary : SbqlTheme.Colors.textSecondary)
                .lineLimit(1)

            Button {
                let sql = appVM.results.closeTab(id: tab.id)
                if let sql {
                    appVM.editor.sqlText = sql
                }
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 8, weight: .bold))
                    .foregroundStyle(SbqlTheme.Colors.textTertiary)
                    .frame(width: 14, height: 14)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .opacity(isActive ? 1 : 0.5)
        }
        .padding(.horizontal, SbqlTheme.Spacing.sm)
        .padding(.vertical, SbqlTheme.Spacing.xs)
        .background(isActive ? SbqlTheme.Colors.surfaceElevated : Color.clear)
        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
        .overlay(alignment: .bottom) {
            if isActive {
                SbqlTheme.Colors.accent
                    .frame(height: 2)
                    .clipShape(RoundedRectangle(cornerRadius: 1))
            }
        }
        .contentShape(Rectangle())
        .onTapGesture {
            if let sql = appVM.results.switchToTab(id: tab.id, currentSql: appVM.editor.sqlText) {
                appVM.editor.sqlText = sql
            }
        }
    }

    private var queryContent: some View {
        VStack(spacing: 0) {
            if appVM.editor.isVisible {
                VStack(spacing: 0) {
                    SQLEditorView()
                    EditorToolbar()
                }
                .frame(minHeight: SbqlTheme.Size.editorMinHeight, maxHeight: 300)
                .transition(.move(edge: .top).combined(with: .opacity))
            }

            ResultsToolbar()
            if appVM.results.isFilterBarVisible {
                FilterBar()
            }
            ResultsView()
        }
        .animation(SbqlTheme.Animations.quick, value: appVM.editor.isVisible)
    }
}
