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

    /// A row another setting disables is shown (disabled) and counts (kb: decisions/controls
    /// "Disabled, not hidden"): Stripes outside the angle colouring, the Upper limit with manual
    /// iterations; the ↺ puts them back too.
    func testDisabledRowsCount() {
        let m = model(look: "aurora")
        m.interiorColoring = .flat
        m.interiorStripes += 4
        XCTAssertEqual(ControlMore.changed(.interior, model: m), ["Stripes"])
        m.interiorColoring = .angle
        XCTAssertEqual(ControlMore.changed(.interior, model: m), ["Stripes"])
        let cap = m.maxIterationsCap
        m.autoIterations = false
        m.maxIterationsCap = cap == 1_000_000 ? 10_000_000 : 1_000_000
        XCTAssertEqual(ControlMore.changed(.quality, model: m), ["Upper limit"])
        ControlMore.reset(.quality, model: m)
        XCTAssertEqual(m.maxIterationsCap, cap)
        // Every setting row is there whatever the switches say: only the fractal hides rows.
        for s in ControlMore.settings {
            XCTAssertTrue(s.shown(m), s.name)
        }
    }

    /// "Orbit iterations shown" is an essential row under the Orbit switch (not behind More),
    /// found by the search words the user named.
    func testOrbitDotsRow() {
        XCTAssertFalse(ControlMore.settings.contains { $0.name == OrbitDotsRow.title || $0.name == "Iterates" })
        for q in ["dots", "points", "orbit length", "iterates", "Orbit iterations"] {
            XCTAssertTrue(ControlSection.overlays.matches(q), q)
            XCTAssertFalse(ControlSection.overlays.moreMatches(q), "\(q) opens Overlays' More")
        }
        // The small map's size (no "Large mini-map" row: the map's arrow button enlarges it).
        XCTAssertFalse(ControlMore.settings.contains { $0.name == "Large mini-map" })
        XCTAssertTrue(ControlMore.settings.contains { $0.name == "Mini-map size (small)" })
        for q in ["large", "enlarge", "bigger"] {
            XCTAssertTrue(ControlSection.overlays.moreMatches(q), q)
        }
        XCTAssertEqual(OrbitDotsRow.text(50), "50 dots")
        XCTAssertEqual(OrbitDotsRow.text(1), "1 dot")
        XCTAssertTrue(OrbitDotsRow.presets.contains(50))
        XCTAssertLessThanOrEqual(OrbitDotsRow.presets.max() ?? 0, 1000)
    }

    /// The essential rows stay out of More (the decided split).
    func testEssentialRowsAreNotHidden() {
        let hidden = Set(ControlMore.settings.map(\.name))
        for essential in ["Shading", "Outline", "Glow", "Density", "Look", "Palette", "Colouring", "Colour",
                          "Pattern density", "Automatic iterations", "Cinematic effects", "Mini-map",
                          "Orbit of the point under the pointer", "Orbit iterations shown", "Two-finger scroll", "Maximum"] {
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
