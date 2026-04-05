import SwiftUI

/// Small colored badge/pill for inline metadata (connection name, duration, count).
struct BadgePillView: View {
    let text: String
    let color: Color
    var fontSize: CGFloat = 9
    var fontWeight: Font.Weight = .medium
    var fontDesign: Font.Design = .default

    var body: some View {
        Text(text)
            .font(.system(size: fontSize, weight: fontWeight, design: fontDesign))
            .foregroundStyle(color)
            .padding(.horizontal, SbqlTheme.Spacing.xs)
            .padding(.vertical, 1)
            .background(color.opacity(0.12))
            .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.small))
    }
}
