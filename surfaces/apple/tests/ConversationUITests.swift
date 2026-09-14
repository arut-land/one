import XCTest

final class ConversationUITests: XCTestCase {
    func testMultilineDraftAndSend() {
        let app = XCUIApplication()
        app.launch()
        defer { app.terminate() }
        let composer = app.textFields["message-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 10))
        XCTAssertFalse(app.buttons["send-message"].isEnabled)
        composer.click()
        composer.typeText("First line")
        // Option-Return is the multiline field's own line break; the surface
        // adds no key handling of its own.
        composer.typeKey(.return, modifierFlags: .option)
        composer.typeKey(.return, modifierFlags: .option)
        composer.typeText("Third line")
        XCTAssertEqual(composer.value as? String, "First line\n\nThird line")
        for index in 4...12 {
            composer.typeKey(.return, modifierFlags: .option)
            composer.typeText("Line \(index)")
        }
        let text = "First line\n\nThird line\n" + (4...12).map { "Line \($0)" }.joined(separator: "\n")
        XCTAssertEqual(composer.value as? String, text)
        XCTAssertTrue(app.buttons["send-message"].isHittable)
        composer.typeKey(.return, modifierFlags: .command)
        let message = app.staticTexts.matching(NSPredicate(format: "value == %@", text)).firstMatch
        XCTAssertTrue(message.waitForExistence(timeout: 5))
        XCTAssertEqual(composer.value as? String, "")
        XCTAssertFalse(app.buttons["send-message"].isEnabled)
    }

    func testKeyboardSendingAndConversationDrafts() {
        let app = XCUIApplication()
        app.launch()
        defer { app.terminate() }
        let composer = app.textFields["message-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 10))
        composer.click()
        composer.typeText("A complete message from the keyboard")
        XCTAssertEqual(composer.value as? String, "A complete message from the keyboard")
        composer.typeKey(.return, modifierFlags: .command)
        let firstMessage = app.staticTexts.matching(NSPredicate(format: "value == %@", "A complete message from the keyboard")).firstMatch
        XCTAssertTrue(firstMessage.waitForExistence(timeout: 5))
        XCTAssertEqual(composer.value as? String, "")

        composer.click()
        composer.typeText("Keep this draft")
        app.typeKey("n", modifierFlags: .command)
        XCTAssertEqual(composer.value as? String, "")
        composer.click()
        composer.typeText("A second conversation")
        composer.typeKey(.return, modifierFlags: [])
        let secondMessage = app.staticTexts.matching(NSPredicate(format: "value == %@", "A second conversation")).firstMatch
        XCTAssertTrue(secondMessage.waitForExistence(timeout: 5))

        let firstConversation = app.outlines.staticTexts["A complete message from the keyboard"].firstMatch
        XCTAssertTrue(firstConversation.exists)
        firstConversation.click()
        XCTAssertEqual(composer.value as? String, "Keep this draft")
        app.typeKey("l", modifierFlags: .command)
        composer.typeText(" is still here")
        XCTAssertEqual(composer.value as? String, "Keep this draft is still here")
    }
}
