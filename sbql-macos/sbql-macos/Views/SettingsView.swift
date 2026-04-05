import SwiftUI

/// Settings window with theme selector organized by category.
struct SettingsView: View {
    // Settings window runs in separate scene — uses singleton since @Environment is not available
    private var theme: ThemeManager { ThemeManager.shared }
    @State private var searchText: String = ""

    private var darkThemes: [ThemeName] {
        ThemeName.allCases.filter { $0.isDark && matchesSearch($0) }
    }

    private var lightThemes: [ThemeName] {
        ThemeName.allCases.filter { !$0.isDark && matchesSearch($0) }
    }

    private func matchesSearch(_ name: ThemeName) -> Bool {
        guard !searchText.isEmpty else { return true }
        return name.rawValue.localizedCaseInsensitiveContains(searchText)
            || name.description.localizedCaseInsensitiveContains(searchText)
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Appearance")
                    .font(.system(size: 15, weight: .semibold))
                Spacer()
                // Search
                HStack(spacing: 4) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                    TextField("Search themes…", text: $searchText)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12))
                        .frame(width: 140)
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(Color.primary.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 6))
            }
            .padding(.horizontal, 20)
            .padding(.top, 16)
            .padding(.bottom, 12)

            Divider()

            // Settings content
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    // Tab animation picker
                    animationSection

                    Divider()

                    // Theme grid
                    if !darkThemes.isEmpty {
                        themeSection("Dark Themes", themes: darkThemes)
                    }
                    if !lightThemes.isEmpty {
                        themeSection("Light Themes", themes: lightThemes)
                    }
                }
                .padding(20)
            }
        }
        .frame(width: 520, height: 600)
        .background(Color(nsColor: .windowBackgroundColor))
    }

    // MARK: - Animation Section

    private var animationSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Tab Animation")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.secondary)
                .textCase(.uppercase)

            HStack(spacing: 8) {
                ForEach(TabAnimation.allCases) { anim in
                    AnimationOptionView(
                        animation: anim,
                        isSelected: theme.tabAnimation == anim,
                        onSelect: {
                            withAnimation(.easeInOut(duration: 0.15)) {
                                theme.tabAnimation = anim
                            }
                        }
                    )
                }
            }
        }
    }

    private func themeSection(_ title: String, themes: [ThemeName]) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.secondary)
                .textCase(.uppercase)

            LazyVGrid(
                columns: [GridItem(.adaptive(minimum: 145), spacing: 10)],
                spacing: 10
            ) {
                ForEach(themes) { themeName in
                    ThemeCardView(
                        name: themeName,
                        isSelected: theme.activeThemeName == themeName,
                        onSelect: {
                            withAnimation(.easeInOut(duration: 0.15)) {
                                theme.activeThemeName = themeName
                            }
                        }
                    )
                }
            }
        }
    }
}

/// Compact card preview for a single theme.
private struct ThemeCardView: View {
    let name: ThemeName
    let isSelected: Bool
    let onSelect: () -> Void

    @State private var isHovered = false

    var body: some View {
        let palette = ThemeManager.palette(for: name)

        Button(action: onSelect) {
            VStack(spacing: 0) {
                // Mini preview
                miniPreview(palette)

                // Label
                HStack(spacing: 4) {
                    Text(name.rawValue)
                        .font(.system(size: 11, weight: isSelected ? .semibold : .medium))
                        .lineLimit(1)
                    Spacer()
                    if isSelected {
                        Image(systemName: "checkmark.circle.fill")
                            .font(.system(size: 11))
                            .foregroundStyle(Color.accentColor)
                    }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 6)
            }
            .background(Color.primary.opacity(isHovered ? 0.06 : 0.03))
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(
                        isSelected ? Color.accentColor : Color.primary.opacity(isHovered ? 0.15 : 0.08),
                        lineWidth: isSelected ? 2 : 1
                    )
            )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }

    /// A tiny mock-up of the editor look.
    private func miniPreview(_ p: ThemePalette) -> some View {
        VStack(spacing: 0) {
            // Title bar mock
            HStack(spacing: 3) {
                Circle().fill(Color.red.opacity(0.7)).frame(width: 5, height: 5)
                Circle().fill(Color.yellow.opacity(0.7)).frame(width: 5, height: 5)
                Circle().fill(Color.green.opacity(0.7)).frame(width: 5, height: 5)
                Spacer()
                RoundedRectangle(cornerRadius: 2)
                    .fill(p.surfaceElevated)
                    .frame(width: 30, height: 5)
                Spacer()
                Color.clear.frame(width: 20)
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 4)
            .background(p.surface)

            // Content mock: sidebar + editor
            HStack(spacing: 1) {
                // Sidebar mock
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(0 ..< 4, id: \.self) { _ in
                        RoundedRectangle(cornerRadius: 1)
                            .fill(p.textTertiary.opacity(0.5))
                            .frame(width: .random(in: 20 ... 35), height: 3)
                    }
                }
                .padding(4)
                .frame(width: 42, alignment: .leading)
                .background(p.surface)

                // Editor mock
                VStack(alignment: .leading, spacing: 3) {
                    HStack(spacing: 2) {
                        RoundedRectangle(cornerRadius: 1).fill(p.accent).frame(width: 22, height: 3)
                        RoundedRectangle(cornerRadius: 1).fill(p.textPrimary.opacity(0.5)).frame(width: 8, height: 3)
                        RoundedRectangle(cornerRadius: 1).fill(p.success).frame(width: 18, height: 3)
                    }
                    HStack(spacing: 2) {
                        RoundedRectangle(cornerRadius: 1).fill(p.accent).frame(width: 16, height: 3)
                        RoundedRectangle(cornerRadius: 1).fill(p.warning).frame(width: 12, height: 3)
                    }
                    // Result rows mock
                    ForEach(0 ..< 3, id: \.self) { _ in
                        RoundedRectangle(cornerRadius: 1)
                            .fill(p.textPrimary.opacity(0.2))
                            .frame(height: 3)
                    }
                }
                .padding(4)
                .background(p.background)
            }
        }
        .frame(height: 65)
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }
}

// MARK: - ThemeName helpers

// MARK: - Animation Option Card

private struct AnimationOptionView: View {
    let animation: TabAnimation
    let isSelected: Bool
    let onSelect: () -> Void

    @State private var isHovered = false

    var body: some View {
        Button(action: onSelect) {
            VStack(spacing: 6) {
                Image(systemName: animation.icon)
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(isSelected ? Color.accentColor : .secondary)
                    .frame(width: 36, height: 36)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(isSelected
                                ? Color.accentColor.opacity(0.15)
                                : Color.primary.opacity(isHovered ? 0.06 : 0.03))
                    )

                Text(animation.rawValue)
                    .font(.system(size: 10, weight: isSelected ? .semibold : .regular))
                    .foregroundStyle(isSelected ? .primary : .secondary)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: 10)
                    .fill(Color.primary.opacity(isHovered ? 0.04 : 0))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10)
                    .stroke(isSelected ? Color.accentColor : .clear, lineWidth: 1.5)
            )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

extension ThemeName {
    var isDark: Bool {
        switch self {
        case .latte, .githubLight, .solarizedLight, .rosePineDawn,
             .ayuLight, .gruvboxLight, .everforestLight, .tokyoNightDay, .nordLight:
            return false
        default:
            return true
        }
    }
}
