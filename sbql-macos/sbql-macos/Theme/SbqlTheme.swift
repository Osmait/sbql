import SwiftUI

/// Design tokens for the sbql macOS app — Linear/Arc-inspired dark-first aesthetic.
enum SbqlTheme {

    // MARK: - Colors

    enum Colors {
        static let background       = Color(hex: 0x0A0A0B)
        static let surface          = Color(hex: 0x141416)
        static let surfaceElevated  = Color(hex: 0x1C1C1F)
        static let surfaceHover     = Color(hex: 0x232328)
        static let accent           = Color(hex: 0x5B6AF0)
        static let accentHover      = Color(hex: 0x6E7BF7)
        static let danger           = Color(hex: 0xE5484D)
        static let success          = Color(hex: 0x30A46C)
        static let warning          = Color(hex: 0xF5A623)
        static let textPrimary      = Color(hex: 0xECECF0)
        static let textSecondary    = Color(hex: 0x8B8B9A)
        static let textTertiary     = Color(hex: 0x5C5C6A)
        static let border           = Color(hex: 0x2A2A30)
        static let borderSubtle     = Color(hex: 0x1F1F25)
        static let selection        = Color(hex: 0x5B6AF0).opacity(0.15)
    }

    // MARK: - Corner Radii

    enum Radius {
        static let small:  CGFloat = 4
        static let medium: CGFloat = 8
        static let large:  CGFloat = 12
        static let xl:     CGFloat = 16
    }

    // MARK: - Spacing (4pt grid)

    enum Spacing {
        static let xxs: CGFloat = 2
        static let xs:  CGFloat = 4
        static let sm:  CGFloat = 8
        static let md:  CGFloat = 12
        static let lg:  CGFloat = 16
        static let xl:  CGFloat = 24
        static let xxl: CGFloat = 32
    }

    // MARK: - Sizing

    enum Size {
        static let sidebarWidth:   CGFloat = 220
        static let editorMinHeight: CGFloat = 120
        static let rowHeight:      CGFloat = 28
        static let toolbarHeight:  CGFloat = 36
        static let iconSize:       CGFloat = 14
    }
}

// MARK: - Color hex initializer

extension Color {
    init(hex: UInt32, opacity: Double = 1.0) {
        let r = Double((hex >> 16) & 0xFF) / 255.0
        let g = Double((hex >> 8)  & 0xFF) / 255.0
        let b = Double(hex         & 0xFF) / 255.0
        self.init(.sRGB, red: r, green: g, blue: b, opacity: opacity)
    }
}
