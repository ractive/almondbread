import AlmondbreadCore
import XCTest
@testable import AlmondbreadKit

/// What the failure screen says when the renderer cannot be created (kb: platform/ios-device
/// "GPU floor"): a GPU error's message as it is, without the enum case `"\(error)"` puts round it.
@MainActor
final class RenderFailureTests: XCTestCase {
    func testGpuErrorReadsAsItsMessage() {
        let message = "This device's GPU is too old: Almondbread needs an A14 chip or newer"
        XCTAssertEqual(ExplorerModel.failureText(CoreError.Gpu(message: message)), message)
    }

    func testOtherErrorSaysWhatFailed() {
        let text = ExplorerModel.failureText(CoreError.InvalidArgument(message: "layer must not be null"))
        XCTAssertTrue(text.hasPrefix("The renderer could not be created: "), text)
    }
}
