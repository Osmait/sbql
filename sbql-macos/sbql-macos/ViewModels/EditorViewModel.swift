import Foundation

/// State for the SQL editor pane.
@Observable
final class EditorViewModel {
    var sqlText: String = ""
    var isExecuting: Bool = false
    var isVisible: Bool = false
    var lastQueryDuration: Duration?
}
