import SwiftUI

/// Island gap and radius constants.
private enum Island {
    static let gap: CGFloat = 8
    static let radius: CGFloat = 10
    static let outerPadding: CGFloat = 8
    static let trafficLightsWidth: CGFloat = 56
    static let headerTopPadding: CGFloat = 4
}

/// Top-level layout using IntelliJ-style "island" design.
/// Each section is a rounded-corner panel floating on a darker background.
struct MainWindow: View {
    @Environment(AppViewModel.self) private var appVM
    @Environment(ThemeManager.self) private var theme

    var body: some View {
        let _ = theme.activeThemeName
        let _ = theme.tabAnimation

        ZStack {
            // "Sea" background — darker than islands
            SbqlTheme.Colors.background.ignoresSafeArea()

            VStack(spacing: Island.gap) {
                // Header island — flush with top, traffic lights sit inside
                unifiedHeader

                // Content islands: sidebar + main
                HStack(spacing: Island.gap) {
                    if appVM.isSidebarVisible {
                        SidebarView()
                            .frame(width: SbqlTheme.Size.sidebarWidth)
                            .background(SbqlTheme.Colors.surface)
                            .clipShape(RoundedRectangle(cornerRadius: Island.radius))
                            .transition(.move(edge: .leading).combined(with: .opacity))
                    }

                    mainContent
                }
                .animation(SbqlTheme.Animations.quick, value: appVM.isSidebarVisible)
            }
            .padding(.horizontal, Island.outerPadding)
            .padding(.bottom, Island.outerPadding)
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
        // Global keyboard shortcuts
        .background {
            Button { appVM.isCommandPaletteOpen = true } label: { EmptyView() }
                .keyboardShortcut("k", modifiers: .command)
            Button { appVM.isTablePreviewOpen = true } label: { EmptyView() }
                .keyboardShortcut("p", modifiers: .command)
            Button { appVM.formatSQL() } label: { EmptyView() }
                .keyboardShortcut("f", modifiers: [.command, .shift])
        }
        .overlay {
            ModalPresenter(isPresented: Binding(
                get: { appVM.isShowingHistory },
                set: { appVM.isShowingHistory = $0 }
            )) {
                QueryHistoryModal().environment(appVM)
            }

            ModalPresenter(isPresented: Binding(
                get: { appVM.isShowingSavedQueries },
                set: { appVM.isShowingSavedQueries = $0 }
            )) {
                SavedQueriesModal().environment(appVM)
            }

            ModalPresenter(isPresented: Binding(
                get: { appVM.isCommandPaletteOpen },
                set: { appVM.isCommandPaletteOpen = $0 }
            )) {
                CommandPalette().environment(appVM)
            }

            ModalPresenter(isPresented: Binding(
                get: { appVM.isTablePreviewOpen },
                set: { appVM.isTablePreviewOpen = $0 }
            )) {
                TablePreviewModal().environment(appVM)
            }
        }
        .sheet(isPresented: Binding(
            get: { appVM.savedQueries.isShowingSaveSheet },
            set: { appVM.savedQueries.isShowingSaveSheet = $0 }
        )) {
            SaveQuerySheet()
                .environment(appVM)
        }
    }

    // MARK: - Main Content Island

    private var mainContent: some View {
        VStack(spacing: Island.gap) {
            switch appVM.activeTab {
            case .query:
                queryContent
            case .diagram:
                DiagramView()
                    .background(SbqlTheme.Colors.surface)
                    .clipShape(RoundedRectangle(cornerRadius: Island.radius))
            }
        }
    }

    // MARK: - Header Island

    private var unifiedHeader: some View {
        HStack(spacing: SbqlTheme.Spacing.sm) {
            // Space for macOS traffic lights
            Spacer().frame(width: Island.trafficLightsWidth)

            // Sidebar toggle
            Button {
                withAnimation(SbqlTheme.Animations.quick) {
                    appVM.isSidebarVisible.toggle()
                }
            } label: {
                Image(systemName: "sidebar.leading")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(
                        appVM.isSidebarVisible
                            ? SbqlTheme.Colors.accent
                            : SbqlTheme.Colors.textTertiary
                    )
            }
            .buttonStyle(.hoverIcon)
            .keyboardShortcut("s", modifiers: [.command, .control])
            .accessibilityLabel("Toggle Sidebar")

            SbqlTheme.Colors.border
                .frame(width: 1, height: 16)
                .opacity(0.5)

            // Mode pills
            ForEach(AppViewModel.ActiveTab.allCases, id: \.self) { tab in
                Button {
                    withAnimation(SbqlTheme.Animations.quick) {
                        appVM.activeTab = tab
                    }
                    if tab == .diagram {
                        // loadDiagram() handles its own errors via showError()
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
                        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
                }
                .buttonStyle(.hover)
            }

            // Table tabs
            if appVM.activeTab == .query, !appVM.results.tabs.isEmpty {
                SbqlTheme.Colors.border
                    .frame(width: 1, height: 16)
                    .opacity(0.5)

                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: SbqlTheme.Spacing.xxs) {
                        ForEach(appVM.results.tabs) { tab in
                            queryTab(tab, isActive: tab.id == appVM.results.activeTabId)
                        }
                    }
                }

                Button {
                    appVM.results.closeAllTabs()
                    appVM.editor.sqlText = ""
                } label: {
                    Image(systemName: "xmark.circle")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(SbqlTheme.Colors.textTertiary)
                }
                .buttonStyle(.hoverIcon)
                .help("Close all tabs")
                .accessibilityLabel("Close All Tabs")
            }

            Spacer()

            // History & Saved buttons
            Button {
                appVM.isShowingHistory = true
            } label: {
                Image(systemName: "clock.arrow.circlepath")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.6))
            }
            .buttonStyle(.hoverIcon)
            .help("Query History")
            .accessibilityLabel("Query History")

            Button {
                appVM.isShowingSavedQueries = true
            } label: {
                Image(systemName: "bookmark")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.6))
            }
            .buttonStyle(.hoverIcon)
            .help("Saved Queries")
            .accessibilityLabel("Saved Queries")

            SbqlTheme.Colors.border
                .frame(width: 1, height: 16)
                .opacity(0.5)

            connectionInfo
            headerActions
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.top, Island.headerTopPadding) // align content with macOS traffic lights
        .padding(.bottom, SbqlTheme.Spacing.sm)
        .background(SbqlTheme.Colors.surface)
        .clipShape(
            UnevenRoundedRectangle(
                topLeadingRadius: 0,
                bottomLeadingRadius: Island.radius,
                bottomTrailingRadius: Island.radius,
                topTrailingRadius: 0
            )
        )
    }

    // MARK: - Header Actions

    private var headerActions: some View {
        HStack(spacing: SbqlTheme.Spacing.md) {
            Button {
                Task { await appVM.refreshTables() }
            } label: {
                Image(systemName: "arrow.clockwise")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(SbqlTheme.Colors.accent.opacity(0.6))
            }
            .buttonStyle(.hoverIcon)
            .accessibilityLabel("Refresh Tables")
        }
    }

    // MARK: - Connection Info

    @ViewBuilder
    private var connectionInfo: some View {
        if let conn = appVM.connections.activeConnection {
            HStack(spacing: SbqlTheme.Spacing.sm) {
                Text(conn.name)
                    .font(SbqlTheme.Typography.bodyMedium)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)

                // Backend badge
                BackendBadgeView(backend: conn.backend)

                // Database name
                HStack(spacing: 3) {
                    Image(systemName: "cylinder")
                        .font(.system(size: 9))
                    Text(conn.backend == .sqlite
                        ? (conn.filePath.flatMap { URL(fileURLWithPath: $0).lastPathComponent } ?? "memory")
                        : conn.database
                    )
                    .font(SbqlTheme.Typography.captionBold)
                }
                .foregroundStyle(SbqlTheme.Colors.accent)
                .padding(.horizontal, SbqlTheme.Spacing.sm)
                .padding(.vertical, 2)
                .background(SbqlTheme.Colors.accent.opacity(0.12))
                .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))

                // Query duration
                if let d = appVM.editor.lastQueryDuration {
                    let ms = d.totalMilliseconds
                    let durationColor = ms < 500
                        ? SbqlTheme.Colors.success
                        : ms < 2000
                            ? SbqlTheme.Colors.warning
                            : SbqlTheme.Colors.danger

                    HStack(spacing: 3) {
                        Image(systemName: "bolt.fill")
                            .font(.system(size: 8))
                        Text(formatDuration(d))
                            .font(SbqlTheme.Typography.captionBold)
                    }
                    .foregroundStyle(durationColor)
                    .padding(.horizontal, SbqlTheme.Spacing.sm)
                    .padding(.vertical, 2)
                    .background(durationColor.opacity(0.12))
                    .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
                }
            }
        } else {
            Text("sbql")
                .font(SbqlTheme.Typography.bodyMedium)
                .foregroundStyle(SbqlTheme.Colors.textTertiary)
        }
    }

    private func formatDuration(_ d: Duration) -> String {
        d.formattedQueryDuration
    }

    // MARK: - Query Tab

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
            .buttonStyle(.hoverIcon)
            .opacity(isActive ? 1 : 0.5)
        }
        .padding(.horizontal, SbqlTheme.Spacing.sm)
        .padding(.vertical, SbqlTheme.Spacing.xs)
        .background(isActive ? SbqlTheme.Colors.surfaceElevated : Color.clear)
        .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
        .overlay(alignment: .bottom) {
            if isActive {
                SbqlTheme.Colors.accent
                    .frame(height: 2)
                    .clipShape(RoundedRectangle(cornerRadius: 1))
            }
        }
        .hoverHighlight()
        .contentShape(Rectangle())
        .onTapGesture {
            if let sql = appVM.results.switchToTab(id: tab.id, currentSql: appVM.editor.sqlText) {
                appVM.editor.sqlText = sql
            }
        }
    }

    // MARK: - Query Content

    private var queryContent: some View {
        VStack(spacing: Island.gap) {
            // Editor island
            if appVM.editor.isVisible {
                VStack(spacing: 0) {
                    SQLEditorView(activeTheme: theme.activeThemeName)
                        .id(theme.activeThemeName)
                    EditorToolbar()
                }
                .frame(minHeight: SbqlTheme.Size.editorMinHeight, maxHeight: 300)
                .background(SbqlTheme.Colors.surface)
                .clipShape(RoundedRectangle(cornerRadius: Island.radius))
                .transition(.move(edge: .top).combined(with: .opacity))
            }

            // Results island — animated on tab switch
            VStack(spacing: 0) {
                ResultsToolbar()
                if appVM.results.isFilterBarVisible {
                    FilterBar()
                }
                ResultsView()
            }
            .background(SbqlTheme.Colors.surface)
            .clipShape(RoundedRectangle(cornerRadius: Island.radius))
            .modifier(TabSwitchModifier(
                tabId: appVM.results.activeTabId ?? "none",
                direction: appVM.results.tabSwitchDirection,
                animation: theme.tabAnimation
            ))
        }
        .animation(SbqlTheme.Animations.quick, value: appVM.editor.isVisible)
    }

}
