import Foundation

extension Duration {
    /// Total milliseconds from a `Duration` value.
    var totalMilliseconds: Int64 {
        let c = components
        return c.seconds * 1000 + c.attoseconds / 1_000_000_000_000_000
    }

    /// Human-readable query duration string (e.g. "<1ms", "42ms", "1.3s").
    var formattedQueryDuration: String {
        let ms = totalMilliseconds
        if ms < 1 { return "<1ms" }
        if ms < 1000 { return "\(ms)ms" }
        let seconds = Double(ms) / 1000.0
        return String(format: "%.1fs", seconds)
    }
}
