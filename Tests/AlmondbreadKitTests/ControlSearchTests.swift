import XCTest
import AlmondbreadCore
@testable import AlmondbreadKit

/// The control center's search of place and palette names (kb: decisions/controls "Control
/// center", Search): every fractal's places, by name only.
final class ControlSearchTests: XCTestCase {
    private let all = ExplorerModel.allPlaces
    private let options = ExplorerModel.formulaOptions

    func testPlaceNameMatchIgnoresCase() {
        let hits = ControlSearch.places(all, query: "SEAHORSE").map(\.name)
        XCTAssertTrue(hits.contains("Seahorse Valley"), "\(hits)")
        XCTAssertTrue(hits.allSatisfy { $0.lowercased().contains("seahorse") }, "\(hits)")
        XCTAssertEqual(ControlSearch.places(all, query: "  seahorse ").map(\.name), hits, "surrounding spaces count")
    }

    func testDiacriticsAndSubstrings() {
        XCTAssertTrue(ControlSearch.nameMatches("Café Spiral", query: "cafe"))
        XCTAssertTrue(ControlSearch.nameMatches("Spiral Galaxy", query: "al gal"))
        XCTAssertFalse(ControlSearch.nameMatches("Spiral Galaxy", query: "nebula"))
    }

    /// Names only: a word that is only in a description finds nothing.
    func testDescriptionsAreNotSearched() {
        let p = all.first { !$0.description.isEmpty }!
        let word = p.description.split(separator: " ").map(String.init)
            .first { w in w.count > 6 && !all.contains { $0.name.localizedCaseInsensitiveContains(w) } }
        if let word {
            XCTAssertFalse(ControlSearch.places(all, query: word).contains { $0.id == p.id }, word)
        }
    }

    /// Another fractal's place is found from the Mandelbrot set and labelled with its fractal.
    func testOtherFormulaHit() throws {
        let hit = try XCTUnwrap(ControlSearch.places(all, query: "supernova").first)
        XCTAssertEqual(hit.formula, "burningship")
        XCTAssertEqual(ControlSearch.formulaLabel(hit.formula, current: "mandelbrot", options: options), "Burning Ship")
        XCTAssertNil(ControlSearch.formulaLabel(hit.formula, current: "burningship", options: options))
        // A Multibrot power: the formula itself, without "+ c".
        let cubic = try XCTUnwrap(all.first { $0.formula == "multibrot3" })
        XCTAssertEqual(ControlSearch.formulaLabel(cubic.formula, current: "mandelbrot", options: options), "z³")
        // Search covers every formula's places, not only the current one's.
        XCTAssertFalse(ControlSearch.places(all, query: "ship").filter { $0.formula == "burningship" }.isEmpty)
    }

    func testEmptyQueryMatchesNothing() {
        XCTAssertTrue(ControlSearch.places(all, query: "").isEmpty)
        XCTAssertTrue(ControlSearch.places(all, query: "   ").isEmpty)
        XCTAssertFalse(ControlSearch.nameMatches("Seahorse Valley", query: ""))
    }

    func testNoMatch() {
        XCTAssertTrue(ControlSearch.places(all, query: "qqxzv").isEmpty)
        XCTAssertTrue(ControlSearch.palettes(AlmondbreadCore.paletteLibrary(), query: "qqxzv").isEmpty)
    }

    func testPaletteNameMatch() throws {
        let library = AlmondbreadCore.paletteLibrary()
        let p = try XCTUnwrap(library.first)
        let hits = ControlSearch.palettes(library, query: p.name.uppercased())
        XCTAssertTrue(hits.contains { $0.id == p.id })
        XCTAssertTrue(ControlSearch.palettes(library, query: "").isEmpty)
    }

    /// The pinned fractal row is found by its words and by every family's name; no section
    /// keeps the words (they would open a section without the picker).
    func testFractalRowSearch() {
        for q in ["multibrot", "Fractal", " power ", "burning", "tricorn"] {
            XCTAssertTrue(ControlSearch.fractalMatches(q), q)
        }
        for f in ExplorerModel.formulaFamilies {
            XCTAssertTrue(ControlSearch.fractalMatches(f.title.components(separatedBy: " ")[0]), f.title)
        }
        XCTAssertFalse(ControlSearch.fractalMatches(""))
        XCTAssertFalse(ControlSearch.fractalMatches("palette"))
        for s in ControlSection.allCases {
            XCTAssertFalse(s.matches("multibrot"), "\(s) still claims the fractal picker")
        }
    }

    func testArrowMoves() {
        // 10 tiles in 3 columns.
        XCTAssertEqual(ControlSearch.move(from: 0, count: 10, columns: 3, dx: 1, dy: 0), 1)
        XCTAssertNil(ControlSearch.move(from: 0, count: 10, columns: 3, dx: -1, dy: 0))
        XCTAssertEqual(ControlSearch.move(from: 1, count: 10, columns: 3, dx: 0, dy: 1), 4)
        XCTAssertNil(ControlSearch.move(from: 8, count: 10, columns: 3, dx: 0, dy: 1))
        XCTAssertEqual(ControlSearch.move(from: 5, count: 10, columns: 3, dx: 0, dy: -1), 2)
        XCTAssertNil(ControlSearch.move(from: 9, count: 10, columns: 3, dx: 1, dy: 0))
    }
}
