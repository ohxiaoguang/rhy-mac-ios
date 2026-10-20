import XCTest

/// Device-only interaction evidence. Screenshots and host logs still require review.
final class DeviceSmoke: XCTestCase {
    func testLaunchAndTouch() throws {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchEnvironment = [
            "NX_FPS": "1", "NX_DUMP_AUDIO": "work/ipad.wav",
            "NX_DUMP_AT": "8,20,40,60", "NX_TRACE_FRAME": "1"
        ]
        app.launch()
        let start = app.buttons["启动游戏"]
        XCTAssertTrue(start.waitForExistence(timeout: 15), "Launcher must appear")
        XCTAssertTrue(start.isEnabled, "Copy game resources before running this test")
        capture("launcher")
        start.tap()
        Thread.sleep(forTimeInterval: 12)
        XCTAssertEqual(app.state, .runningForeground)
        capture("game-started")
        // Touch the actual UIKit gamepad A control, rather than injecting HID state.
        let size = app.frame.size
        let unit = min(size.width, size.height) / 10
        let point = app.coordinate(withNormalizedOffset: CGVector(dx: 0, dy: 0))
            .withOffset(CGVector(dx: size.width - 1.65 * unit, dy: size.height - 3.2 * unit))
        for index in 0..<6 {
            point.press(forDuration: 0.12)
            Thread.sleep(forTimeInterval: 5)
            capture("touch-A-\(index)")
            XCTAssertEqual(app.state, .runningForeground)
        }
        Thread.sleep(forTimeInterval: 12)
        capture("final")
    }

    private func capture(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
