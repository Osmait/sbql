import SwiftUI

extension SbqlTheme {
    enum Typography {
        // Headings
        static let title = Font.system(size: 16, weight: .semibold)

        // Body
        static let body = Font.system(size: 13, weight: .regular)
        static let bodyMedium = Font.system(size: 13, weight: .medium)
        static let caption = Font.system(size: 11, weight: .regular)
        static let captionBold = Font.system(size: 11, weight: .medium)

        // Code / monospace
        static let code = Font.system(size: 13, design: .monospaced)
        static let codeSmall = Font.system(size: 11, design: .monospaced)
    }
}
