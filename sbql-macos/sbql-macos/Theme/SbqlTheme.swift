import SwiftUI

/// Design tokens for the sbql macOS app — Catppuccin Mocha palette.
enum SbqlTheme {
    // MARK: - Colors (Catppuccin Mocha)

    enum Colors {
        static let background = Color(hex: 0x11111B) // Crust
        static let surface = Color(hex: 0x1E1E2E) // Base
        static let surfaceElevated = Color(hex: 0x313244) // Surface0
        static let surfaceHover = Color(hex: 0x45475A) // Surface1
        static let accent = Color(hex: 0xCBA6F7) // Mauve
        static let accentHover = Color(hex: 0xB4BEFE) // Lavender
        static let danger = Color(hex: 0xF38BA8) // Red
        static let success = Color(hex: 0xA6E3A1) // Green
        static let warning = Color(hex: 0xF9E2AF) // Yellow
        static let textPrimary = Color(hex: 0xCDD6F4) // Text
        static let textSecondary = Color(hex: 0xA6ADC8) // Subtext0
        static let textTertiary = Color(hex: 0x7F849C) // Overlay1
        static let border = Color(hex: 0x45475A) // Surface1
        static let borderSubtle = Color(hex: 0x313244) // Surface0
        static let selection = Color(hex: 0xCBA6F7).opacity(0.15) // Mauve
    }

    // MARK: - Corner Radii

    enum Radius {
        static let small: CGFloat = 4
        static let medium: CGFloat = 8
        static let large: CGFloat = 12
    }

    // MARK: - Spacing (4pt grid)

    enum Spacing {
        static let xxs: CGFloat = 2
        static let xs: CGFloat = 4
        static let sm: CGFloat = 8
        static let md: CGFloat = 12
        static let lg: CGFloat = 16
        static let xl: CGFloat = 24
    }

    // MARK: - Sizing

    enum Size {
        static let sidebarWidth: CGFloat = 220
        static let editorMinHeight: CGFloat = 120
        static let rowHeight: CGFloat = 28
    }
}

// MARK: - Color hex initializer

extension Color {
    init(hex: UInt32, opacity: Double = 1.0) {
        let r = Double((hex >> 16) & 0xFF) / 255.0
        let g = Double((hex >> 8) & 0xFF) / 255.0
        let b = Double(hex & 0xFF) / 255.0
        self.init(.sRGB, red: r, green: g, blue: b, opacity: opacity)
    }
}
