import SwiftUI

extension SbqlTheme {
    enum Animations {
        static let spring = Animation.spring(duration: 0.3, bounce: 0.2)
        static let quick = Animation.easeInOut(duration: 0.15)
    }
}
