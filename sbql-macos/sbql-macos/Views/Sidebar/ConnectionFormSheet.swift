import SwiftUI

struct ConnectionFormSheet: View {
    @Environment(AppViewModel.self) private var appVM
    @Environment(\.dismiss) private var dismiss

    @State var connection: Connection
    @State private var password: String = ""
    @State private var isSaving = false

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text(connection.name.isEmpty ? "New Connection" : "Edit Connection")
                    .font(SbqlTheme.Typography.title)
                    .foregroundStyle(SbqlTheme.Colors.textPrimary)
                Spacer()
                Button("Cancel") { dismiss() }
                    .buttonStyle(.plain)
                    .foregroundStyle(SbqlTheme.Colors.textSecondary)
            }
            .padding(SbqlTheme.Spacing.lg)

            Divider().background(SbqlTheme.Colors.border)

            ScrollView {
                VStack(spacing: SbqlTheme.Spacing.lg) {
                    formField("Name", text: $connection.name, prompt: "My Database")

                    // Backend picker
                    HStack {
                        Text("Backend")
                            .font(SbqlTheme.Typography.bodyMedium)
                            .foregroundStyle(SbqlTheme.Colors.textSecondary)
                            .frame(width: 80, alignment: .leading)

                        Picker("", selection: $connection.backend) {
                            Text("PostgreSQL").tag(Connection.Backend.postgres)
                            Text("SQLite").tag(Connection.Backend.sqlite)
                        }
                        .pickerStyle(.segmented)
                    }

                    if connection.backend == .postgres {
                        formField("Host", text: $connection.host, prompt: "localhost")
                        formField("Port", value: $connection.port)
                        formField("User", text: $connection.user, prompt: "postgres")
                        formField("Database", text: $connection.database, prompt: "postgres")
                        formField("Password", text: $password, prompt: "Enter password", isSecure: true)

                        // SSL mode
                        HStack {
                            Text("SSL Mode")
                                .font(SbqlTheme.Typography.bodyMedium)
                                .foregroundStyle(SbqlTheme.Colors.textSecondary)
                                .frame(width: 80, alignment: .leading)

                            Picker("", selection: $connection.sslMode) {
                                ForEach(Connection.SSLMode.allCases, id: \.self) { mode in
                                    Text(mode.displayName).tag(mode)
                                }
                            }
                        }
                    } else {
                        formField("File Path", text: Binding(
                            get: { connection.filePath ?? "" },
                            set: { connection.filePath = $0.isEmpty ? nil : $0 }
                        ), prompt: "/path/to/database.db")
                    }
                }
                .padding(SbqlTheme.Spacing.lg)
            }

            Divider().background(SbqlTheme.Colors.border)

            // Actions
            HStack {
                Spacer()
                Button("Save") {
                    save()
                }
                .buttonStyle(.borderedProminent)
                .tint(SbqlTheme.Colors.accent)
                .disabled(isSaving || connection.name.isEmpty)
            }
            .padding(SbqlTheme.Spacing.lg)
        }
        .frame(width: 420, height: 480)
        .background(SbqlTheme.Colors.surface)
    }

    private func formField(_ label: String, text: Binding<String>, prompt: String, isSecure: Bool = false) -> some View {
        HStack {
            Text(label)
                .font(SbqlTheme.Typography.bodyMedium)
                .foregroundStyle(SbqlTheme.Colors.textSecondary)
                .frame(width: 80, alignment: .leading)

            if isSecure {
                SecureField(prompt, text: text)
                    .textFieldStyle(.plain)
                    .font(SbqlTheme.Typography.body)
                    .padding(SbqlTheme.Spacing.sm)
                    .background(SbqlTheme.Colors.surfaceElevated)
                    .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
            } else {
                TextField(prompt, text: text)
                    .textFieldStyle(.plain)
                    .font(SbqlTheme.Typography.body)
                    .padding(SbqlTheme.Spacing.sm)
                    .background(SbqlTheme.Colors.surfaceElevated)
                    .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
            }
        }
    }

    private func formField(_ label: String, value: Binding<UInt16>) -> some View {
        HStack {
            Text(label)
                .font(SbqlTheme.Typography.bodyMedium)
                .foregroundStyle(SbqlTheme.Colors.textSecondary)
                .frame(width: 80, alignment: .leading)

            TextField("5432", value: value, format: .number)
                .textFieldStyle(.plain)
                .font(SbqlTheme.Typography.body)
                .padding(SbqlTheme.Spacing.sm)
                .background(SbqlTheme.Colors.surfaceElevated)
                .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
        }
    }

    private func save() {
        isSaving = true
        Task {
            do {
                let pw = password.isEmpty ? nil : password
                try await appVM.connections.saveConnection(connection, password: pw)
                dismiss()
            } catch {
                appVM.showError(error)
            }
            isSaving = false
        }
    }
}
