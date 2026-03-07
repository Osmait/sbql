import SwiftUI

struct EditorToolbar: View {
    @Environment(AppViewModel.self) private var appVM

    var body: some View {
        HStack(spacing: SbqlTheme.Spacing.sm) {
            // Run button
            Button {
                Task { await appVM.runQuery() }
            } label: {
                HStack(spacing: SbqlTheme.Spacing.xs) {
                    if appVM.editor.isExecuting {
                        ProgressView()
                            .scaleEffect(0.5)
                            .frame(width: 12, height: 12)
                    } else {
                        Image(systemName: "play.fill")
                            .font(.system(size: 10))
                    }
                    Text("Run")
                        .font(SbqlTheme.Typography.captionBold)
                }
                .foregroundStyle(.white)
                .padding(.horizontal, SbqlTheme.Spacing.md)
                .padding(.vertical, SbqlTheme.Spacing.xs)
                .background(SbqlTheme.Colors.accent)
                .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
            }
            .buttonStyle(.plain)
            .disabled(appVM.editor.isExecuting || appVM.editor.sqlText.isEmpty)
            .keyboardShortcut(.return, modifiers: .command)

            Spacer()

            Text("Cmd+Enter to run")
                .font(SbqlTheme.Typography.caption)
                .foregroundStyle(SbqlTheme.Colors.textTertiary)
        }
        .padding(.horizontal, SbqlTheme.Spacing.lg)
        .padding(.vertical, SbqlTheme.Spacing.sm)
        .background(SbqlTheme.Colors.surface)
        .overlay(alignment: .bottom) {
            SbqlTheme.Colors.border.frame(height: 1)
        }
    }
}
