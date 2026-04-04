import SwiftUI

/// Persisted theme selection.
enum ThemeName: String, CaseIterable, Identifiable {
    case mocha = "Mocha"
    case frappe = "Frappé"
    case latte = "Latte"
    case dracula = "Dracula"
    case oneDark = "One Dark Pro"
    case nord = "Nord"
    case tokyoNight = "Tokyo Night"

    var id: String { rawValue }

    var description: String {
        switch self {
        case .mocha: return "Dark — deep purples and warm accents"
        case .frappe: return "Medium — muted tones, easier on the eyes"
        case .latte: return "Light — bright and clean"
        case .dracula: return "Dark — iconic purple and pink palette"
        case .oneDark: return "Dark — warm Atom-inspired tones"
        case .nord: return "Dark — arctic cool blue hues"
        case .tokyoNight: return "Dark — neon-tinged Tokyo palette"
        }
    }
}

/// Holds the full color palette for a single theme.
struct ThemePalette {
    let background: Color
    let surface: Color
    let surfaceElevated: Color
    let surfaceHover: Color
    let accent: Color
    let accentHover: Color
    let danger: Color
    let success: Color
    let warning: Color
    let textPrimary: Color
    let textSecondary: Color
    let textTertiary: Color
    let border: Color
    let borderSubtle: Color

    var selection: Color { accent.opacity(0.15) }

    /// Distinct colors for FK relationship lines in the ER diagram.
    let fkLinePalette: [Color]
}

/// Global observable that provides the active color palette.
@Observable
final class ThemeManager {
    static let shared = ThemeManager()

    private static let storageKey = "selectedTheme"

    var activeThemeName: ThemeName {
        didSet {
            UserDefaults.standard.set(activeThemeName.rawValue, forKey: Self.storageKey)
        }
    }

    var palette: ThemePalette {
        Self.palette(for: activeThemeName)
    }

    private init() {
        if let stored = UserDefaults.standard.string(forKey: Self.storageKey),
           let name = ThemeName(rawValue: stored)
        {
            activeThemeName = name
        } else {
            activeThemeName = .mocha
        }
    }

    // MARK: - Palettes

    static func palette(for theme: ThemeName) -> ThemePalette {
        switch theme {
        case .mocha: return mochaPalette
        case .frappe: return frappePalette
        case .latte: return lattePalette
        case .dracula: return draculaPalette
        case .oneDark: return oneDarkPalette
        case .nord: return nordPalette
        case .tokyoNight: return tokyoNightPalette
        }
    }

    // MARK: - Catppuccin Mocha (dark)

    private static let mochaPalette = ThemePalette(
        background: Color(hex: 0x11111B),
        surface: Color(hex: 0x1E1E2E),
        surfaceElevated: Color(hex: 0x313244),
        surfaceHover: Color(hex: 0x45475A),
        accent: Color(hex: 0xCBA6F7),
        accentHover: Color(hex: 0xB4BEFE),
        danger: Color(hex: 0xF38BA8),
        success: Color(hex: 0xA6E3A1),
        warning: Color(hex: 0xF9E2AF),
        textPrimary: Color(hex: 0xCDD6F4),
        textSecondary: Color(hex: 0xA6ADC8),
        textTertiary: Color(hex: 0x8E93AB),
        border: Color(hex: 0x45475A),
        borderSubtle: Color(hex: 0x313244),
        fkLinePalette: [
            Color(hex: 0xCBA6F7), Color(hex: 0x89B4FA), Color(hex: 0xA6E3A1),
            Color(hex: 0xFAB387), Color(hex: 0xF38BA8), Color(hex: 0x94E2D5),
            Color(hex: 0xF9E2AF), Color(hex: 0xF5C2E7), Color(hex: 0x74C7EC),
            Color(hex: 0xB4BEFE),
        ]
    )

    // MARK: - Catppuccin Frappé (medium)

    private static let frappePalette = ThemePalette(
        background: Color(hex: 0x232634),
        surface: Color(hex: 0x303446),
        surfaceElevated: Color(hex: 0x414559),
        surfaceHover: Color(hex: 0x51576D),
        accent: Color(hex: 0xCA9EE6),
        accentHover: Color(hex: 0xBABBF1),
        danger: Color(hex: 0xE78284),
        success: Color(hex: 0xA6D189),
        warning: Color(hex: 0xE5C890),
        textPrimary: Color(hex: 0xC6D0F5),
        textSecondary: Color(hex: 0xA5ADCE),
        textTertiary: Color(hex: 0x949CB8),
        border: Color(hex: 0x51576D),
        borderSubtle: Color(hex: 0x414559),
        fkLinePalette: [
            Color(hex: 0xCA9EE6), Color(hex: 0x8CAAEE), Color(hex: 0xA6D189),
            Color(hex: 0xEF9F76), Color(hex: 0xE78284), Color(hex: 0x81C8BE),
            Color(hex: 0xE5C890), Color(hex: 0xF4B8E4), Color(hex: 0x85C1DC),
            Color(hex: 0xBABBF1),
        ]
    )

    // MARK: - Catppuccin Latte (light)

    private static let lattePalette = ThemePalette(
        background: Color(hex: 0xEFF1F5), // Crust
        surface: Color(hex: 0xE6E9EF), // Base
        surfaceElevated: Color(hex: 0xCCD0DA), // Surface0
        surfaceHover: Color(hex: 0xBCC0CC), // Surface1
        accent: Color(hex: 0x8839EF), // Mauve
        accentHover: Color(hex: 0x7287FD), // Lavender
        danger: Color(hex: 0xD20F39), // Red
        success: Color(hex: 0x40A02B), // Green
        warning: Color(hex: 0xDF8E1D), // Yellow
        textPrimary: Color(hex: 0x3C3F55), // Darkened Text (~9:1)
        textSecondary: Color(hex: 0x4C4F69), // Text (official, ~7:1)
        textTertiary: Color(hex: 0x5C5F73), // Subtext1 (~5.2:1)
        border: Color(hex: 0xACB0BE), // Darker border for visibility
        borderSubtle: Color(hex: 0xBCC0CC),
        fkLinePalette: [
            Color(hex: 0x8839EF), Color(hex: 0x1E66F5), Color(hex: 0x40A02B),
            Color(hex: 0xFE640B), Color(hex: 0xD20F39), Color(hex: 0x179299),
            Color(hex: 0xDF8E1D), Color(hex: 0xEA76CB), Color(hex: 0x209FB5),
            Color(hex: 0x7287FD),
        ]
    )

    // MARK: - Dracula (dracula-theme.com)

    private static let draculaPalette = ThemePalette(
        background: Color(hex: 0x282A36), // Background (official)
        surface: Color(hex: 0x2D303D), // Slightly lighter than bg
        surfaceElevated: Color(hex: 0x343746),
        surfaceHover: Color(hex: 0x44475A), // Current Line (official)
        accent: Color(hex: 0xBD93F9), // Purple
        accentHover: Color(hex: 0xFF79C6), // Pink
        danger: Color(hex: 0xFF5555), // Red
        success: Color(hex: 0x50FA7B), // Green
        warning: Color(hex: 0xF1FA8C), // Yellow
        textPrimary: Color(hex: 0xF8F8F2), // Foreground (official)
        textSecondary: Color(hex: 0xBDBFC6), // Dimmed foreground
        textTertiary: Color(hex: 0x7E8DB8), // Brightened comment
        border: Color(hex: 0x44475A), // Current Line
        borderSubtle: Color(hex: 0x343746),
        fkLinePalette: [
            Color(hex: 0xBD93F9), Color(hex: 0x8BE9FD), Color(hex: 0x50FA7B),
            Color(hex: 0xFFB86C), Color(hex: 0xFF5555), Color(hex: 0xFF79C6),
            Color(hex: 0xF1FA8C), Color(hex: 0x6272A4), Color(hex: 0x8BE9FD),
            Color(hex: 0xBD93F9),
        ]
    )

    // MARK: - One Dark Pro (Atom / VS Code)

    private static let oneDarkPalette = ThemePalette(
        background: Color(hex: 0x21252B), // Sidebar (official)
        surface: Color(hex: 0x282C34), // Editor (official)
        surfaceElevated: Color(hex: 0x2C313A), // Activity bar
        surfaceHover: Color(hex: 0x3E4451), // Selection
        accent: Color(hex: 0xC678DD), // Purple/Magenta
        accentHover: Color(hex: 0x61AFEF), // Blue
        danger: Color(hex: 0xE06C75), // Red
        success: Color(hex: 0x98C379), // Green
        warning: Color(hex: 0xE5C07B), // Yellow
        textPrimary: Color(hex: 0xABB2BF), // Foreground (official)
        textSecondary: Color(hex: 0x9DA2AC), // Mid-tone
        textTertiary: Color(hex: 0x7A808C), // Brightened comment
        border: Color(hex: 0x3E4451),
        borderSubtle: Color(hex: 0x2C313A),
        fkLinePalette: [
            Color(hex: 0xC678DD), Color(hex: 0x61AFEF), Color(hex: 0x98C379),
            Color(hex: 0xD19A66), Color(hex: 0xE06C75), Color(hex: 0x56B6C2),
            Color(hex: 0xE5C07B), Color(hex: 0xBE5046), Color(hex: 0x61AFEF),
            Color(hex: 0xC678DD),
        ]
    )

    // MARK: - Nord (nordtheme.com)

    private static let nordPalette = ThemePalette(
        background: Color(hex: 0x242933), // Deeper dark for depth
        surface: Color(hex: 0x2E3440), // Polar Night 1 (official bg)
        surfaceElevated: Color(hex: 0x3B4252), // Polar Night 2
        surfaceHover: Color(hex: 0x434C5E), // Polar Night 3
        accent: Color(hex: 0x88C0D0), // Frost 2
        accentHover: Color(hex: 0x81A1C1), // Frost 3
        danger: Color(hex: 0xBF616A), // Aurora Red
        success: Color(hex: 0xA3BE8C), // Aurora Green
        warning: Color(hex: 0xEBCB8B), // Aurora Yellow
        textPrimary: Color(hex: 0xECEFF4), // Snow Storm 3
        textSecondary: Color(hex: 0xA5ADBA), // Mid-tone
        textTertiary: Color(hex: 0x7B88A1), // Readable muted
        border: Color(hex: 0x434C5E), // Polar Night 3
        borderSubtle: Color(hex: 0x3B4252), // Polar Night 2
        fkLinePalette: [
            Color(hex: 0x88C0D0), Color(hex: 0x81A1C1), Color(hex: 0xA3BE8C),
            Color(hex: 0xD08770), Color(hex: 0xBF616A), Color(hex: 0x8FBCBB),
            Color(hex: 0xEBCB8B), Color(hex: 0xB48EAD), Color(hex: 0x5E81AC),
            Color(hex: 0x88C0D0),
        ]
    )

    // MARK: - Tokyo Night (github.com/enkia/tokyo-night-vscode-theme)

    private static let tokyoNightPalette = ThemePalette(
        background: Color(hex: 0x16161E), // bg_dark
        surface: Color(hex: 0x1A1B26), // bg (official)
        surfaceElevated: Color(hex: 0x24283B), // bg_highlight
        surfaceHover: Color(hex: 0x292E42), // Selection
        accent: Color(hex: 0x7AA2F7), // Blue
        accentHover: Color(hex: 0xBB9AF7), // Purple
        danger: Color(hex: 0xF7768E), // Red
        success: Color(hex: 0x9ECE6A), // Green
        warning: Color(hex: 0xE0AF68), // Yellow
        textPrimary: Color(hex: 0xA9B1D6), // Foreground (official)
        textSecondary: Color(hex: 0x9099B7), // Mid-tone
        textTertiary: Color(hex: 0x6B7394), // Readable comment
        border: Color(hex: 0x292E42),
        borderSubtle: Color(hex: 0x24283B),
        fkLinePalette: [
            Color(hex: 0x7AA2F7), Color(hex: 0xBB9AF7), Color(hex: 0x9ECE6A),
            Color(hex: 0xFF9E64), Color(hex: 0xF7768E), Color(hex: 0x73DACA),
            Color(hex: 0xE0AF68), Color(hex: 0x2AC3DE), Color(hex: 0x7DCFFF),
            Color(hex: 0xBB9AF7),
        ]
    )
}
