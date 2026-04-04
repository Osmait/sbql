import SwiftUI

struct ConnectionFormSheet: View {
    @Environment(AppViewModel.self) private var appVM
    @Environment(\.dismiss) private var dismiss

    @State var connection: Connection
    @State private var password: String = ""
    @State private var sshPassword: String = ""
    @State private var isSaving = false

    private let backends: [(Connection.Backend, String, String, Color)] = [
        (.postgres, "PG", "PostgreSQL", Color(hex: 0x336791)),
        (.mysql, "MY", "MySQL", Color(hex: 0x00758F)),
        (.sqlite, "SQ", "SQLite", Color(hex: 0x44A8D6)),
        (.mongodb, "MG", "MongoDB", Color(hex: 0x47A248)),
        (.redis, "RD", "Redis", Color(hex: 0xD82C20)),
        (.dynamodb, "DB", "DynamoDB", Color(hex: 0x4053D6)),
        (.sqlserver, "MS", "SQL Server", Color(hex: 0xCC2927)),
    ]

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

                    // Backend grid
                    VStack(alignment: .leading, spacing: SbqlTheme.Spacing.sm) {
                        Text("Backend")
                            .font(SbqlTheme.Typography.bodyMedium)
                            .foregroundStyle(SbqlTheme.Colors.textSecondary)

                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 90), spacing: 6)], spacing: 6) {
                            ForEach(backends, id: \.0) { backend, abbr, label, color in
                                Button {
                                    connection.backend = backend
                                } label: {
                                    VStack(spacing: 3) {
                                        Text(abbr)
                                            .font(.system(size: 11, weight: .bold, design: .monospaced))
                                            .foregroundStyle(connection.backend == backend ? .white : color)
                                        Text(label)
                                            .font(.system(size: 9))
                                            .foregroundStyle(connection.backend == backend ? .white.opacity(0.9) : SbqlTheme.Colors.textSecondary)
                                    }
                                    .frame(maxWidth: .infinity)
                                    .padding(.vertical, 6)
                                    .background(
                                        connection.backend == backend
                                            ? color
                                            : SbqlTheme.Colors.surfaceElevated
                                    )
                                    .clipShape(RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium))
                                    .overlay(
                                        RoundedRectangle(cornerRadius: SbqlTheme.Radius.medium)
                                            .stroke(connection.backend == backend ? color : Color.clear, lineWidth: 1)
                                    )
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }

                    // Dynamic form fields based on backend
                    Group {
                        switch connection.backend {
                        case .postgres, .mysql, .sqlserver:
                            sqlFormFields(
                                defaultUser: connection.backend == .mysql ? "root" : connection.backend == .sqlserver ? "sa" : "postgres",
                                defaultDb: connection.backend == .mysql ? "mydb" : connection.backend == .sqlserver ? "master" : "postgres",
                                showSSL: connection.backend != .sqlserver
                            )
                        case .mongodb:
                            formField("Host", text: $connection.host, prompt: "localhost")
                            formField("Port", value: $connection.port)
                            formField("User", text: $connection.user, prompt: "admin (optional)")
                            formField("Database", text: $connection.database, prompt: "mydb")
                            formField("Password", text: $password, prompt: "Enter password (optional)", isSecure: true)
                        case .redis:
                            formField("Host", text: $connection.host, prompt: "localhost")
                            formField("Port", value: $connection.port)
                            formField("Password", text: $password, prompt: "Enter password (optional)", isSecure: true)
                        case .dynamodb:
                            formField("Endpoint", text: $connection.host, prompt: "localhost")
                            formField("Port", value: $connection.port)
                            formField("Region", text: $connection.database, prompt: "us-east-1")
                            formField("Access Key", text: $connection.user, prompt: "AKIAIOSFODNN7EXAMPLE")
                            formField("Secret Key", text: $password, prompt: "Enter secret key", isSecure: true)
                        case .sqlite:
                            formField("File Path", text: Binding(
                                get: { connection.filePath ?? "" },
                                set: { connection.filePath = $0.isEmpty ? nil : $0 }
                            ), prompt: "/path/to/database.db")
                        }
                    }

                    // SSH Tunnel toggle (not for SQLite)
                    if connection.backend != .sqlite {
                        Divider().background(SbqlTheme.Colors.border)

                        HStack {
                            Toggle(isOn: $connection.sshEnabled) {
                                HStack(spacing: SbqlTheme.Spacing.xs) {
                                    Image(systemName: "lock.shield")
                                        .font(.system(size: 14))
                                        .foregroundStyle(connection.sshEnabled ? SbqlTheme.Colors.accent : SbqlTheme.Colors.textTertiary)
                                    VStack(alignment: .leading, spacing: 1) {
                                        Text("SSH Tunnel")
                                            .font(SbqlTheme.Typography.bodyMedium)
                                            .foregroundStyle(SbqlTheme.Colors.textPrimary)
                                        Text("Connect through an SSH server")
                                            .font(SbqlTheme.Typography.caption)
                                            .foregroundStyle(SbqlTheme.Colors.textTertiary)
                                    }
                                }
                            }
                            .toggleStyle(.switch)
                            .tint(SbqlTheme.Colors.accent)
                        }

                        if connection.sshEnabled {
                            formField("SSH Host", text: $connection.sshHost, prompt: "bastion.example.com")
                            formField("SSH Port", value: $connection.sshPort)
                            formField("SSH User", text: $connection.sshUser, prompt: "ubuntu")

                            // Auth method picker
                            HStack {
                                Text("Auth")
                                    .font(SbqlTheme.Typography.bodyMedium)
                                    .foregroundStyle(SbqlTheme.Colors.textSecondary)
                                    .frame(width: 80, alignment: .leading)
                                Picker("", selection: $connection.sshAuthMethod) {
                                    Text("Password").tag("password")
                                    Text("Key File").tag("key")
                                }
                                .pickerStyle(.segmented)
                            }

                            if connection.sshAuthMethod == "key" {
                                formField("Key Path", text: Binding(
                                    get: { connection.sshKeyPath ?? "" },
                                    set: { connection.sshKeyPath = $0.isEmpty ? nil : $0 }
                                ), prompt: "~/.ssh/id_rsa")
                            } else {
                                formField("SSH Password", text: $sshPassword, prompt: "SSH password", isSecure: true)
                            }
                        }
                    }

                    // Safe Mode toggle
                    if BiometricService.isAvailable {
                        HStack {
                            Toggle(isOn: $connection.requiresBiometric) {
                                HStack(spacing: SbqlTheme.Spacing.xs) {
                                    Image(systemName: "touchid")
                                        .font(.system(size: 14))
                                        .foregroundStyle(connection.requiresBiometric ? SbqlTheme.Colors.accent : SbqlTheme.Colors.textTertiary)
                                    VStack(alignment: .leading, spacing: 1) {
                                        Text("Safe Mode")
                                            .font(SbqlTheme.Typography.bodyMedium)
                                            .foregroundStyle(SbqlTheme.Colors.textPrimary)
                                        Text("Require Touch ID to connect")
                                            .font(SbqlTheme.Typography.caption)
                                            .foregroundStyle(SbqlTheme.Colors.textTertiary)
                                    }
                                }
                            }
                            .toggleStyle(.switch)
                            .tint(SbqlTheme.Colors.accent)
                        }
                    }
                }
                .padding(SbqlTheme.Spacing.lg)
            }

            Divider().background(SbqlTheme.Colors.border)

            // Actions
            HStack {
                Spacer()
                Button("Save") { save() }
                    .buttonStyle(.borderedProminent)
                    .tint(SbqlTheme.Colors.accent)
                    .disabled(isSaving || connection.name.isEmpty)
            }
            .padding(SbqlTheme.Spacing.lg)
        }
        .frame(width: 440, height: connection.sshEnabled ? 700 : 520)
        .background(SbqlTheme.Colors.surface)
    }

    // MARK: - SQL Form Fields (PG/MySQL shared)

    @ViewBuilder
    private func sqlFormFields(defaultUser: String, defaultDb: String, showSSL: Bool) -> some View {
        formField("Host", text: $connection.host, prompt: "localhost")
        formField("Port", value: $connection.port)
        formField("User", text: $connection.user, prompt: defaultUser)
        formField("Database", text: $connection.database, prompt: defaultDb)
        formField("Password", text: $password, prompt: "Enter password", isSecure: true)

        if showSSL {
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
        }
    }

    // MARK: - Form Fields

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

            TextField("", value: value, format: .number)
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
                let sshPw = sshPassword.isEmpty ? nil : sshPassword
                try await appVM.connections.saveConnection(connection, password: pw, sshPassword: sshPw)
                // Persist biometric flag in UserDefaults
                UserDefaults.standard.set(connection.requiresBiometric, forKey: "biometric_\(connection.id)")
                dismiss()
            } catch {
                appVM.showError(error)
            }
            isSaving = false
        }
    }
}
