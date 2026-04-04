import Foundation

struct CommandItem: Identifiable {
    let id = UUID()
    let title: String
    let subtitle: String?
    let icon: String
    let category: Category
    let shortcut: String?
    let action: () -> Void

    enum Category: String {
        case command = "Commands"
        case table = "Tables"
        case connection = "Connections"
        case savedQuery = "Saved"
        case history = "History"
    }
}
