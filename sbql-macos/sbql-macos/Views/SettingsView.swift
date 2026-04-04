import SwiftUI

/// Settings window with theme selector.
struct SettingsView: View {
    private var theme: ThemeManager { ThemeManager.shared }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Appearance")
                .font(.system(size: 15, weight: .semibold))
                .padding(.bottom, 12)

            VStack(spacing: 8) {
                ForEach(ThemeName.allCases) { themeName in
                    ThemeRowView(
                        name: themeName,
                        isSelected: theme.activeThemeName == themeName,
                        onSelect: { theme.activeThemeName = themeName }
                    )
                }
            }
        }
        .padding(24)
        .frame(width: 420)
        .background(Color(nsColor: .windowBackgroundColor))
    }
}

/// A single theme row extracted to help the compiler.
private struct ThemeRowView: View {
    let name: ThemeName
    let isSelected: Bool
    let onSelect: () -> Void

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: 12) {
                swatches
                labels
                Spacer()
                checkmark
            }
            .padding(10)
            .background(rowBackground)
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .overlay(rowBorder)
        }
        .buttonStyle(.plain)
    }

    private var swatches: some View {
        let palette = ThemeManager.palette(for: name)
        return HStack(spacing: 3) {
            swatch(palette.background)
            swatch(palette.surface)
            swatch(palette.accent)
            swatch(palette.textPrimary)
        }
        .padding(4)
        .background(palette.background)
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(Color.primary.opacity(0.1), lineWidth: 1)
        )
    }

    private func swatch(_ color: Color) -> some View {
        RoundedRectangle(cornerRadius: 3)
            .fill(color)
            .frame(width: 18, height: 32)
    }

    private var labels: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(name.rawValue)
                .font(.system(size: 13, weight: .medium))
            Text(name.description)
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
        }
    }

    @ViewBuilder
    private var checkmark: some View {
        if isSelected {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 16))
                .foregroundStyle(Color.accentColor)
        }
    }

    private var rowBackground: Color {
        isSelected ? Color.accentColor.opacity(0.08) : Color.primary.opacity(0.03)
    }

    private var rowBorder: some View {
        RoundedRectangle(cornerRadius: 8)
            .stroke(
                isSelected ? Color.accentColor.opacity(0.3) : Color.clear,
                lineWidth: 1
            )
    }
}
