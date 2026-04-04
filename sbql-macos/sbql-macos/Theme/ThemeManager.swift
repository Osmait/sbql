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
    case githubDark = "GitHub Dark"
    case gruvbox = "Gruvbox Dark"
    case solarized = "Solarized Dark"
    case moonlight = "Moonlight"
    case kanagawa = "Kanagawa"
    case rosePine = "Rosé Pine"
    case ayuDark = "Ayu Dark"
    case everforest = "Everforest Dark"
    // Light themes
    case githubLight = "GitHub Light"
    case solarizedLight = "Solarized Light"
    case rosePineDawn = "Rosé Pine Dawn"
    case ayuLight = "Ayu Light"
    case gruvboxLight = "Gruvbox Light"
    case everforestLight = "Everforest Light"
    case tokyoNightDay = "Tokyo Night Day"
    case nordLight = "Nord Light"

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
        case .githubDark: return "Dark — GitHub's official dark theme"
        case .gruvbox: return "Dark — retro warm Vim classic"
        case .solarized: return "Dark — Ethan Schoonover's precision palette"
        case .moonlight: return "Dark — soft purple moonlit tones"
        case .kanagawa: return "Dark — Japanese ink-inspired Neovim theme"
        case .rosePine: return "Dark — elegant muted rose tones"
        case .ayuDark: return "Dark — warm orange Sublime/VS Code theme"
        case .everforest: return "Dark — nature-inspired green hues"
        case .githubLight: return "Light — GitHub's clean bright interface"
        case .solarizedLight: return "Light — warm cream precision palette"
        case .rosePineDawn: return "Light — soft warm rose tones"
        case .ayuLight: return "Light — crisp warm whites"
        case .gruvboxLight: return "Light — warm retro cream tones"
        case .everforestLight: return "Light — soft green nature tones"
        case .tokyoNightDay: return "Light — bright Tokyo blue-white"
        case .nordLight: return "Light — arctic snow and frost"
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
        case .githubDark: return githubDarkPalette
        case .gruvbox: return gruvboxPalette
        case .solarized: return solarizedPalette
        case .moonlight: return moonlightPalette
        case .kanagawa: return kanagawaPalette
        case .rosePine: return rosePinePalette
        case .ayuDark: return ayuDarkPalette
        case .everforest: return everforestPalette
        case .githubLight: return githubLightPalette
        case .solarizedLight: return solarizedLightPalette
        case .rosePineDawn: return rosePineDawnPalette
        case .ayuLight: return ayuLightPalette
        case .gruvboxLight: return gruvboxLightPalette
        case .everforestLight: return everforestLightPalette
        case .tokyoNightDay: return tokyoNightDayPalette
        case .nordLight: return nordLightPalette
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

    // MARK: - GitHub Dark (github.com)

    private static let githubDarkPalette = ThemePalette(
        background: Color(hex: 0x0D1117),
        surface: Color(hex: 0x161B22),
        surfaceElevated: Color(hex: 0x21262D),
        surfaceHover: Color(hex: 0x30363D),
        accent: Color(hex: 0x58A6FF),
        accentHover: Color(hex: 0x79C0FF),
        danger: Color(hex: 0xF85149),
        success: Color(hex: 0x3FB950),
        warning: Color(hex: 0xD29922),
        textPrimary: Color(hex: 0xE6EDF3),
        textSecondary: Color(hex: 0xB1BAC4),
        textTertiary: Color(hex: 0x8B949E),
        border: Color(hex: 0x30363D),
        borderSubtle: Color(hex: 0x21262D),
        fkLinePalette: [
            Color(hex: 0x58A6FF), Color(hex: 0xBC8CFF), Color(hex: 0x3FB950),
            Color(hex: 0xD29922), Color(hex: 0xF85149), Color(hex: 0x79C0FF),
            Color(hex: 0xDB6D28), Color(hex: 0xF778BA), Color(hex: 0x56D364),
            Color(hex: 0xA371F7),
        ]
    )

    // MARK: - Gruvbox Dark (github.com/morhetz/gruvbox)

    private static let gruvboxPalette = ThemePalette(
        background: Color(hex: 0x1D2021), // bg0_h (hard contrast)
        surface: Color(hex: 0x282828), // bg0
        surfaceElevated: Color(hex: 0x3C3836), // bg1
        surfaceHover: Color(hex: 0x504945), // bg2
        accent: Color(hex: 0xFE8019), // orange
        accentHover: Color(hex: 0xFABD2F), // yellow
        danger: Color(hex: 0xFB4934), // red
        success: Color(hex: 0xB8BB26), // green
        warning: Color(hex: 0xFABD2F), // yellow
        textPrimary: Color(hex: 0xEBDBB2), // fg (light0)
        textSecondary: Color(hex: 0xBDAE93), // fg3
        textTertiary: Color(hex: 0xA89984), // fg4
        border: Color(hex: 0x504945), // bg2
        borderSubtle: Color(hex: 0x3C3836), // bg1
        fkLinePalette: [
            Color(hex: 0xFE8019), Color(hex: 0x83A598), Color(hex: 0xB8BB26),
            Color(hex: 0xFABD2F), Color(hex: 0xFB4934), Color(hex: 0x8EC07C),
            Color(hex: 0xD3869B), Color(hex: 0xD65D0E), Color(hex: 0x689D6A),
            Color(hex: 0x458588),
        ]
    )

    // MARK: - Solarized Dark (ethanschoonover.com/solarized)

    private static let solarizedPalette = ThemePalette(
        background: Color(hex: 0x002B36), // base03
        surface: Color(hex: 0x073642), // base02
        surfaceElevated: Color(hex: 0x0A4050), // between base02 and base01
        surfaceHover: Color(hex: 0x586E75), // base01
        accent: Color(hex: 0x268BD2), // blue
        accentHover: Color(hex: 0x2AA198), // cyan
        danger: Color(hex: 0xDC322F), // red
        success: Color(hex: 0x859900), // green
        warning: Color(hex: 0xB58900), // yellow
        textPrimary: Color(hex: 0xFDF6E3), // base3
        textSecondary: Color(hex: 0xEEE8D5), // base2
        textTertiary: Color(hex: 0x93A1A1), // base1
        border: Color(hex: 0x586E75), // base01
        borderSubtle: Color(hex: 0x073642), // base02
        fkLinePalette: [
            Color(hex: 0x268BD2), Color(hex: 0x2AA198), Color(hex: 0x859900),
            Color(hex: 0xB58900), Color(hex: 0xDC322F), Color(hex: 0xD33682),
            Color(hex: 0xCB4B16), Color(hex: 0x6C71C4), Color(hex: 0x2AA198),
            Color(hex: 0x268BD2),
        ]
    )

    // MARK: - Moonlight (github.com/atomiks/moonlight-vscode-theme)

    private static let moonlightPalette = ThemePalette(
        background: Color(hex: 0x1E2030),
        surface: Color(hex: 0x222436),
        surfaceElevated: Color(hex: 0x2F334D),
        surfaceHover: Color(hex: 0x3B3F5C),
        accent: Color(hex: 0x82AAFF), // blue
        accentHover: Color(hex: 0xC099FF), // purple
        danger: Color(hex: 0xFF757F),
        success: Color(hex: 0xC3E88D),
        warning: Color(hex: 0xFFC777),
        textPrimary: Color(hex: 0xC8D3F5),
        textSecondary: Color(hex: 0xA9B8E8),
        textTertiary: Color(hex: 0x7A88CF),
        border: Color(hex: 0x3B3F5C),
        borderSubtle: Color(hex: 0x2F334D),
        fkLinePalette: [
            Color(hex: 0x82AAFF), Color(hex: 0xC099FF), Color(hex: 0xC3E88D),
            Color(hex: 0xFFC777), Color(hex: 0xFF757F), Color(hex: 0x4FD6BE),
            Color(hex: 0xFF966C), Color(hex: 0xFCA7EA), Color(hex: 0x86E1FC),
            Color(hex: 0xB4C2F0),
        ]
    )

    // MARK: - Kanagawa (github.com/rebelot/kanagawa.nvim)

    private static let kanagawaPalette = ThemePalette(
        background: Color(hex: 0x16161D), // sumiInk0
        surface: Color(hex: 0x1F1F28), // sumiInk1 (default bg)
        surfaceElevated: Color(hex: 0x2A2A37), // sumiInk3
        surfaceHover: Color(hex: 0x363646), // sumiInk4
        accent: Color(hex: 0x957FB8), // oniViolet
        accentHover: Color(hex: 0x7E9CD8), // crystalBlue
        danger: Color(hex: 0xE82424), // samuraiRed
        success: Color(hex: 0x76946A), // autumnGreen
        warning: Color(hex: 0xE6C384), // carpYellow
        textPrimary: Color(hex: 0xDCD7BA), // fujiWhite
        textSecondary: Color(hex: 0xC8C093), // oldWhite
        textTertiary: Color(hex: 0x727169), // fujiGray
        border: Color(hex: 0x363646), // sumiInk4
        borderSubtle: Color(hex: 0x2A2A37), // sumiInk3
        fkLinePalette: [
            Color(hex: 0x957FB8), Color(hex: 0x7E9CD8), Color(hex: 0x76946A),
            Color(hex: 0xE6C384), Color(hex: 0xE82424), Color(hex: 0x6A9589),
            Color(hex: 0xFFA066), Color(hex: 0xD27E99), Color(hex: 0x7FB4CA),
            Color(hex: 0x938AA9),
        ]
    )

    // MARK: - Rosé Pine (rosepinetheme.com)

    private static let rosePinePalette = ThemePalette(
        background: Color(hex: 0x191724), // base
        surface: Color(hex: 0x1F1D2E), // surface
        surfaceElevated: Color(hex: 0x26233A), // overlay
        surfaceHover: Color(hex: 0x312D45),
        accent: Color(hex: 0xEBBCBA), // rose
        accentHover: Color(hex: 0xC4A7E7), // iris
        danger: Color(hex: 0xEB6F92), // love
        success: Color(hex: 0x9CCFD8), // foam
        warning: Color(hex: 0xF6C177), // gold
        textPrimary: Color(hex: 0xE0DEF4), // text
        textSecondary: Color(hex: 0xC4C0DB),
        textTertiary: Color(hex: 0x908CAA), // subtle
        border: Color(hex: 0x312D45),
        borderSubtle: Color(hex: 0x26233A),
        fkLinePalette: [
            Color(hex: 0xEBBCBA), Color(hex: 0xC4A7E7), Color(hex: 0x9CCFD8),
            Color(hex: 0xF6C177), Color(hex: 0xEB6F92), Color(hex: 0x31748F),
            Color(hex: 0xE0DEF4), Color(hex: 0xC4A7E7), Color(hex: 0x9CCFD8),
            Color(hex: 0xEBBCBA),
        ]
    )

    // MARK: - Ayu Dark (github.com/ayu-theme)

    private static let ayuDarkPalette = ThemePalette(
        background: Color(hex: 0x0B0E14),
        surface: Color(hex: 0x0D1017),
        surfaceElevated: Color(hex: 0x131721),
        surfaceHover: Color(hex: 0x1C2029),
        accent: Color(hex: 0xFFAD66), // orange (func)
        accentHover: Color(hex: 0xD2A6FF), // purple (keyword)
        danger: Color(hex: 0xF07178),
        success: Color(hex: 0xAAD94C),
        warning: Color(hex: 0xE6B450),
        textPrimary: Color(hex: 0xBFBDB6),
        textSecondary: Color(hex: 0x9B9787),
        textTertiary: Color(hex: 0x6C7080),
        border: Color(hex: 0x1C2029),
        borderSubtle: Color(hex: 0x131721),
        fkLinePalette: [
            Color(hex: 0xFFAD66), Color(hex: 0xD2A6FF), Color(hex: 0xAAD94C),
            Color(hex: 0xE6B450), Color(hex: 0xF07178), Color(hex: 0x73B8FF),
            Color(hex: 0x95E6CB), Color(hex: 0xF29668), Color(hex: 0x59C2FF),
            Color(hex: 0xD2A6FF),
        ]
    )

    // MARK: - Everforest Dark (github.com/sainnhe/everforest)

    private static let everforestPalette = ThemePalette(
        background: Color(hex: 0x272E33), // bg_dim
        surface: Color(hex: 0x2D353B), // bg0
        surfaceElevated: Color(hex: 0x343F44), // bg1
        surfaceHover: Color(hex: 0x3D484D), // bg2
        accent: Color(hex: 0xA7C080), // green (primary)
        accentHover: Color(hex: 0x7FBBB3), // aqua
        danger: Color(hex: 0xE67E80), // red
        success: Color(hex: 0xA7C080), // green
        warning: Color(hex: 0xDBBC7F), // yellow
        textPrimary: Color(hex: 0xD3C6AA), // fg
        textSecondary: Color(hex: 0xBAAF9A),
        textTertiary: Color(hex: 0x859289), // grey1
        border: Color(hex: 0x3D484D), // bg2
        borderSubtle: Color(hex: 0x343F44), // bg1
        fkLinePalette: [
            Color(hex: 0xA7C080), Color(hex: 0x7FBBB3), Color(hex: 0xD699B6),
            Color(hex: 0xDBBC7F), Color(hex: 0xE67E80), Color(hex: 0x83C092),
            Color(hex: 0xE69875), Color(hex: 0x7FBBB3), Color(hex: 0xA7C080),
            Color(hex: 0xD699B6),
        ]
    )

    // =========================================================================
    // LIGHT THEMES
    // =========================================================================

    // MARK: - GitHub Light (github.com)

    private static let githubLightPalette = ThemePalette(
        background: Color(hex: 0xFFFFFF),
        surface: Color(hex: 0xF6F8FA),
        surfaceElevated: Color(hex: 0xEAEEF2),
        surfaceHover: Color(hex: 0xD0D7DE),
        accent: Color(hex: 0x0969DA), // blue
        accentHover: Color(hex: 0x0550AE),
        danger: Color(hex: 0xCF222E),
        success: Color(hex: 0x1A7F37),
        warning: Color(hex: 0x9A6700),
        textPrimary: Color(hex: 0x1F2328),
        textSecondary: Color(hex: 0x424A53),
        textTertiary: Color(hex: 0x656D76),
        border: Color(hex: 0xD0D7DE),
        borderSubtle: Color(hex: 0xEAEEF2),
        fkLinePalette: [
            Color(hex: 0x0969DA), Color(hex: 0x8250DF), Color(hex: 0x1A7F37),
            Color(hex: 0x9A6700), Color(hex: 0xCF222E), Color(hex: 0x0550AE),
            Color(hex: 0xBF3989), Color(hex: 0x0969DA), Color(hex: 0x1A7F37),
            Color(hex: 0x8250DF),
        ]
    )

    // MARK: - Solarized Light (ethanschoonover.com/solarized)

    private static let solarizedLightPalette = ThemePalette(
        background: Color(hex: 0xFDF6E3), // base3
        surface: Color(hex: 0xEEE8D5), // base2
        surfaceElevated: Color(hex: 0xE4DDCA),
        surfaceHover: Color(hex: 0xD6CEBA),
        accent: Color(hex: 0x268BD2), // blue
        accentHover: Color(hex: 0x2AA198), // cyan
        danger: Color(hex: 0xDC322F), // red
        success: Color(hex: 0x859900), // green
        warning: Color(hex: 0xB58900), // yellow
        textPrimary: Color(hex: 0x073642), // base02
        textSecondary: Color(hex: 0x586E75), // base01
        textTertiary: Color(hex: 0x657B83), // base00
        border: Color(hex: 0xD6CEBA),
        borderSubtle: Color(hex: 0xE4DDCA),
        fkLinePalette: [
            Color(hex: 0x268BD2), Color(hex: 0x2AA198), Color(hex: 0x859900),
            Color(hex: 0xB58900), Color(hex: 0xDC322F), Color(hex: 0xD33682),
            Color(hex: 0xCB4B16), Color(hex: 0x6C71C4), Color(hex: 0x2AA198),
            Color(hex: 0x268BD2),
        ]
    )

    // MARK: - Rosé Pine Dawn (rosepinetheme.com)

    private static let rosePineDawnPalette = ThemePalette(
        background: Color(hex: 0xFAF4ED), // base
        surface: Color(hex: 0xFFFAF3), // surface
        surfaceElevated: Color(hex: 0xF2E9E1), // overlay
        surfaceHover: Color(hex: 0xE4D8CC),
        accent: Color(hex: 0xD7827E), // rose
        accentHover: Color(hex: 0x907AA9), // iris
        danger: Color(hex: 0xB4637A), // love
        success: Color(hex: 0x56949F), // foam
        warning: Color(hex: 0xEA9D34), // gold
        textPrimary: Color(hex: 0x2A2520), // text (darkened)
        textSecondary: Color(hex: 0x575279), // muted
        textTertiary: Color(hex: 0x797593), // subtle
        border: Color(hex: 0xDFDAD2),
        borderSubtle: Color(hex: 0xF2E9E1),
        fkLinePalette: [
            Color(hex: 0xD7827E), Color(hex: 0x907AA9), Color(hex: 0x56949F),
            Color(hex: 0xEA9D34), Color(hex: 0xB4637A), Color(hex: 0x286983),
            Color(hex: 0x575279), Color(hex: 0x907AA9), Color(hex: 0x56949F),
            Color(hex: 0xD7827E),
        ]
    )

    // MARK: - Ayu Light (github.com/ayu-theme)

    private static let ayuLightPalette = ThemePalette(
        background: Color(hex: 0xFCFCFC),
        surface: Color(hex: 0xF8F9FA),
        surfaceElevated: Color(hex: 0xEFF0F1),
        surfaceHover: Color(hex: 0xE1E3E5),
        accent: Color(hex: 0xFF9940), // orange
        accentHover: Color(hex: 0xA37ACC), // purple
        danger: Color(hex: 0xF07171),
        success: Color(hex: 0x86B300),
        warning: Color(hex: 0xF2AE49),
        textPrimary: Color(hex: 0x2A2D2E),
        textSecondary: Color(hex: 0x5C6166),
        textTertiary: Color(hex: 0x787B80),
        border: Color(hex: 0xE1E3E5),
        borderSubtle: Color(hex: 0xEFF0F1),
        fkLinePalette: [
            Color(hex: 0xFF9940), Color(hex: 0xA37ACC), Color(hex: 0x86B300),
            Color(hex: 0xF2AE49), Color(hex: 0xF07171), Color(hex: 0x399EE6),
            Color(hex: 0x4CBF99), Color(hex: 0xED9366), Color(hex: 0x55B4D4),
            Color(hex: 0xA37ACC),
        ]
    )

    // MARK: - Gruvbox Light (github.com/morhetz/gruvbox)

    private static let gruvboxLightPalette = ThemePalette(
        background: Color(hex: 0xFBF1C7), // bg0 (light)
        surface: Color(hex: 0xF2E5BC), // bg1
        surfaceElevated: Color(hex: 0xEBDBB2), // bg/fg (light0)
        surfaceHover: Color(hex: 0xD5C4A1), // bg3
        accent: Color(hex: 0xAF3A03), // orange (dark variant for light bg)
        accentHover: Color(hex: 0xB57614), // yellow
        danger: Color(hex: 0xCC241D), // red
        success: Color(hex: 0x79740E), // green
        warning: Color(hex: 0xB57614), // yellow
        textPrimary: Color(hex: 0x282828), // fg0
        textSecondary: Color(hex: 0x504945), // fg2
        textTertiary: Color(hex: 0x665C54), // fg3
        border: Color(hex: 0xD5C4A1), // bg3
        borderSubtle: Color(hex: 0xEBDBB2),
        fkLinePalette: [
            Color(hex: 0xAF3A03), Color(hex: 0x427B58), Color(hex: 0x79740E),
            Color(hex: 0xB57614), Color(hex: 0xCC241D), Color(hex: 0x076678),
            Color(hex: 0x8F3F71), Color(hex: 0xAF3A03), Color(hex: 0x427B58),
            Color(hex: 0x076678),
        ]
    )

    // MARK: - Everforest Light (github.com/sainnhe/everforest)

    private static let everforestLightPalette = ThemePalette(
        background: Color(hex: 0xFDF6E3), // bg_dim
        surface: Color(hex: 0xF3EAD3), // bg0
        surfaceElevated: Color(hex: 0xEAE0C8), // bg1
        surfaceHover: Color(hex: 0xE0D5B8), // bg2
        accent: Color(hex: 0x8DA101), // green
        accentHover: Color(hex: 0x35A77C), // aqua
        danger: Color(hex: 0xF85552), // red
        success: Color(hex: 0x8DA101), // green
        warning: Color(hex: 0xDFA000), // yellow
        textPrimary: Color(hex: 0x2A3339),
        textSecondary: Color(hex: 0x4F585E), // fg
        textTertiary: Color(hex: 0x708089), // grey0
        border: Color(hex: 0xE0D5B8),
        borderSubtle: Color(hex: 0xEAE0C8),
        fkLinePalette: [
            Color(hex: 0x8DA101), Color(hex: 0x35A77C), Color(hex: 0xE66868),
            Color(hex: 0xDFA000), Color(hex: 0xF85552), Color(hex: 0x3A94C5),
            Color(hex: 0xDF69BA), Color(hex: 0x35A77C), Color(hex: 0x8DA101),
            Color(hex: 0xE66868),
        ]
    )

    // MARK: - Tokyo Night Day (github.com/enkia/tokyo-night-vscode-theme)

    private static let tokyoNightDayPalette = ThemePalette(
        background: Color(hex: 0xE1E2E7), // bg
        surface: Color(hex: 0xD5D6DB), // bg_dark
        surfaceElevated: Color(hex: 0xC4C5CB),
        surfaceHover: Color(hex: 0xB4B5BB),
        accent: Color(hex: 0x2E7DE9), // blue
        accentHover: Color(hex: 0x9854F1), // purple
        danger: Color(hex: 0xF52A65), // red
        success: Color(hex: 0x587539), // green
        warning: Color(hex: 0x8C6C3E), // yellow
        textPrimary: Color(hex: 0x1A1B26), // fg (very dark for contrast)
        textSecondary: Color(hex: 0x3B4261), // fg_dark
        textTertiary: Color(hex: 0x6172B0), // comment
        border: Color(hex: 0xB4B5BB),
        borderSubtle: Color(hex: 0xC4C5CB),
        fkLinePalette: [
            Color(hex: 0x2E7DE9), Color(hex: 0x9854F1), Color(hex: 0x587539),
            Color(hex: 0x8C6C3E), Color(hex: 0xF52A65), Color(hex: 0x007197),
            Color(hex: 0xB15C00), Color(hex: 0x118C74), Color(hex: 0x2E7DE9),
            Color(hex: 0x9854F1),
        ]
    )

    // MARK: - Nord Light (nordtheme.com — Snow Storm variant)

    private static let nordLightPalette = ThemePalette(
        background: Color(hex: 0xECEFF4), // Snow Storm 3
        surface: Color(hex: 0xE5E9F0), // Snow Storm 2
        surfaceElevated: Color(hex: 0xD8DEE9), // Snow Storm 1
        surfaceHover: Color(hex: 0xCBD3E0),
        accent: Color(hex: 0x5E81AC), // Frost 4
        accentHover: Color(hex: 0x81A1C1), // Frost 3
        danger: Color(hex: 0xBF616A), // Aurora Red
        success: Color(hex: 0xA3BE8C), // Aurora Green
        warning: Color(hex: 0xEBCB8B), // Aurora Yellow
        textPrimary: Color(hex: 0x2E3440), // Polar Night 1
        textSecondary: Color(hex: 0x3B4252), // Polar Night 2
        textTertiary: Color(hex: 0x4C566A), // Polar Night 4
        border: Color(hex: 0xCBD3E0),
        borderSubtle: Color(hex: 0xD8DEE9),
        fkLinePalette: [
            Color(hex: 0x5E81AC), Color(hex: 0x81A1C1), Color(hex: 0xA3BE8C),
            Color(hex: 0xD08770), Color(hex: 0xBF616A), Color(hex: 0x8FBCBB),
            Color(hex: 0xEBCB8B), Color(hex: 0xB48EAD), Color(hex: 0x88C0D0),
            Color(hex: 0x5E81AC),
        ]
    )
}
