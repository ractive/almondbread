import XCTest
@testable import AlmondbreadKit

/// Inline icons in tour and help texts (kb: decisions/controls "Guided tours"): `{icon:id}`
/// splits out with a known id, stays text otherwise; the help pages use only known ids (tours
/// are checked by the Rust parser).
@MainActor
final class InlineIconsTests: XCTestCase {
    func testSplitsKnownIconsAndKeepsTheRest() {
        let known: (String) -> Bool = { $0 == "orbit" || $0 == "cockpit-manual" }
        XCTAssertEqual(InlineIcons.pieces("a {icon:orbit} **b** {icon: cockpit-manual }", known: known),
                       [.text("a "), .icon("orbit"), .text(" **b** "), .icon("cockpit-manual")])
        XCTAssertEqual(InlineIcons.pieces("{icon:nope} and {icon:orbit", known: known),
                       [.text("{icon:nope} and {icon:orbit")])
        XCTAssertEqual(InlineIcons.pieces("plain", known: known), [.text("plain")])
    }

    func testRustListsTheCockpitGlyphsAndToolbarSymbols() {
        XCTAssertFalse(InlineIcons.all["cockpit-journey"]?.strokes.isEmpty ?? true)
        XCTAssertEqual(InlineIcons.symbol("control-center"), "slider.horizontal.3")
    }

    func testHelpPagesUseKnownIcons() {
        let texts = HelpPage.allCases.map { ($0.rawValue, HelpContent.page($0).body) }
            + HelpTopic.allCases.map { ($0.rawValue, HelpContent.popover($0)) }
        var seen = 0
        for (name, text) in texts {
            for case .text(let t) in InlineIcons.pieces(text, known: { InlineIcons.all[$0] != nil }) {
                XCTAssertFalse(t.contains("{icon:"), "\(name): an unknown or unclosed icon")
            }
            seen += text.components(separatedBy: "{icon:").count - 1
        }
        XCTAssertGreaterThan(seen, 0)
    }
}
