import LocalAuthentication
import os

/// Wrapper for Touch ID / system password authentication.
enum BiometricService {
    /// Returns true if the device supports biometric or password auth.
    static var isAvailable: Bool {
        let context = LAContext()
        var error: NSError?
        return context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error)
    }

    /// Prompt the user for Touch ID or system password.
    /// Returns true if authenticated, false if cancelled/failed.
    static func authenticate(reason: String) async -> Bool {
        let context = LAContext()
        context.localizedCancelTitle = "Cancel"

        do {
            return try await context.evaluatePolicy(
                .deviceOwnerAuthentication,
                localizedReason: reason
            )
        } catch {
            os_log(.error, "Biometric auth failed: %{public}@", error.localizedDescription)
            return false
        }
    }
}
