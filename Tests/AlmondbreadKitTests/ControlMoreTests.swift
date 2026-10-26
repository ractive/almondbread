import XCTest
import AlmondbreadCore
@testable import AlmondbreadKit

/// The control center's "More…" (kb: decisions/controls "Essential rows and More"): which rows
/// are hidden, and the "· N changed" count against the current look (look fields) or the
/// defaults (the rest). A model without a renderer: setting its values only mirrors them.
@MainActor
final class ControlMoreTests: XCTestCase {
    /// Sections whose settings the test touches (not Navigation and Overlays: their rows are
    /// app-wide preferences stored in user defaults).
    private let sections: [ControlSection] = [.journey, .look, .lighting, .interior, .quality]

    private func model(look: String) -> ExplorerModel {
        let m = ExplorerModel()
        m.lookID = look
        for s in sections { ControlMore.reset(s, model: m) }
        return m
    }

    func testNothingChangedAfterTheMoreReset() {
        for look in ["aurora", "ember", "graphite"] {
            let m = model(look: look)
            for s in sections {
                XCTAssertEqual(ControlMore.changed(s, model: m), [], "\(look) \(s)")
            }
        }
    }

    /// Look fields count against the current look: Ember's glow width is no change in Ember,
    /// but is one against Aurora.
    func testLookFieldsCountAgainstTheLook() {
        let m = model(look: "ember")
        let ember = lookSettings(id: "ember")
        XCTAssertEqual(m.glowWidth, ember.glowWidth)
        XCTAssertNotEqual(ember.glowWidth, lookSettings(id: "aurora").glowWidth, "the test needs a look-specific value")
        m.lookID = "aurora"
        XCTAssertTrue(ControlMore.changed(.lighting, model: m).contains("Glow width"))
        m.lookID = "ember"
        m.lightAngle += 30
        XCTAssertEqual(ControlMore.changed(.lighting, model: m), ["Light angle"])
        ControlMore.reset(.lighting, model: m)
        XCTAssertEqual(m.lightAngle, ember.lightAngle)
        XCTAssertEqual(ControlMore.changed(.lighting, model: m), [])
    }

    /// Fields no look sets count against their defaults, whatever the look.
    func testOtherFieldsCountAgainstTheDefaults() {
        for look in ["aurora", "ember"] {
            let m = model(look: look)
            m.cinematicSpin += 1
            m.depthTint = 0.05
            m.interactivePreview.toggle()
            XCTAssertEqual(ControlMore.changed(.journey, model: m), ["Spin"], look)
            XCTAssertEqual(ControlMore.changed(.look, model: m), ["Depth tint"], look)
            XCTAssertEqual(ControlMore.changed(.quality, model: m), ["Interactive preview"], look)
        }
    }

    /// A conditional row counts only while it is there (Stripes: the angle colouring).
    func testHiddenConditionalRowsDoNotCount() {
        let m = model(look: "aurora")
        m.interiorColoring = .flat
        m.interiorStripes += 4
        XCTAssertEqual(ControlMore.changed(.interior, model: m), [])
        m.interiorColoring = .angle
        XCTAssertEqual(ControlMore.changed(.interior, model: m), ["Stripes"])
    }

    /// The essential rows stay out of More (the decided split).
    func testEssentialRowsAreNotHidden() {
        let hidden = Set(ControlMore.settings.map(\.name))
        for essential in ["Shading", "Outline", "Glow", "Density", "Look", "Palette", "Colouring", "Colour",
                          "Pattern density", "Automatic iterations", "Cinematic effects", "Mini-map",
                          "Orbit of the point under the pointer", "Two-finger scroll"] {
            XCTAssertFalse(hidden.contains(essential), essential)
        }
        for name in ["Light angle", "Glow colour", "Offset", "Stripes", "Anti-aliasing", "Edge panning",
                     "Status (HUD)", "Spin"] {
            XCTAssertTrue(hidden.contains(name), name)
        }
        XCTAssertFalse(ControlMore.hasMore(.places))
        XCTAssertFalse(ControlMore.hasMore(.tours))
        XCTAssertFalse(ControlMore.hasMore(.controls))
        for s in [ControlSection.view, .journey, .look, .lighting, .interior, .overlays, .quality] {
            XCTAssertTrue(ControlMore.hasMore(s), "\(s)")
        }
        XCTAssertEqual(ControlSection.view.title, "Navigation")
        XCTAssertEqual(Set(ControlSection.collapsedByDefault.split(separator: ",").map(String.init)),
                       ["controls", "quality"])
    }

    /// A search for a hidden row opens that section's More, one for an essential row does not.
    func testSearchOpensTheMoreWithTheHit() {
        XCTAssertTrue(ControlSection.interior.moreMatches("stripes"))
        XCTAssertTrue(ControlSection.lighting.moreMatches("angle"))
        XCTAssertTrue(ControlSection.overlays.moreMatches("compass"))
        XCTAssertTrue(ControlSection.quality.moreMatches("aa"))
        XCTAssertFalse(ControlSection.lighting.moreMatches("shading"))
        XCTAssertFalse(ControlSection.overlays.moreMatches("twin"))
        XCTAssertFalse(ControlSection.view.moreMatches("reset"))
        XCTAssertFalse(ControlSection.places.moreMatches("a"))
        XCTAssertFalse(ControlSection.look.moreMatches(""))
        // Every hidden row's section is found by the search at all.
        XCTAssertTrue(ControlSection.interior.matches("stripes"))
        XCTAssertTrue(ControlSection.overlays.matches("compass"))
    }
}
