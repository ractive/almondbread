import XCTest
@testable import AlmondbreadKit

/// The control center's search-local open / close (kb: decisions/controls "Control center",
/// Search): a click during a search flips a section or More for that search only.
@MainActor
final class SearchTogglesTests: XCTestCase {
    func testOutsideASearchTheStoredStateDecides() {
        for stored in [false, true] {
            for flipped in [false, true] {
                XCTAssertEqual(SearchToggles.isOpen(stored: stored, searchDefault: true, searching: false, flipped: flipped), stored)
            }
        }
    }

    func testDuringASearchAClickFlipsTheSearchDefault() {
        // A matching section shows open; a click closes it, another opens it again.
        XCTAssertTrue(SearchToggles.isOpen(stored: false, searchDefault: true, searching: true, flipped: false))
        XCTAssertFalse(SearchToggles.isOpen(stored: true, searchDefault: true, searching: true, flipped: true))
        // A More without a matching row keeps its stored state; a click flips that.
        XCTAssertFalse(SearchToggles.isOpen(stored: false, searchDefault: false, searching: true, flipped: false))
        XCTAssertTrue(SearchToggles.isOpen(stored: false, searchDefault: false, searching: true, flipped: true))
    }

    func testTogglesFlipAndReset() {
        let t = SearchToggles()
        t.searching = true
        t.toggle("tours")
        t.toggle(SearchToggles.moreID(.lighting))
        XCTAssertTrue(t.isFlipped("tours"))
        XCTAssertTrue(t.isFlipped("more.lighting"))
        t.toggle("tours")
        XCTAssertFalse(t.isFlipped("tours"))
        t.reset()
        XCTAssertFalse(t.isFlipped("more.lighting"))
    }

    func testMatchKeyChangesWithTheMatchesOnly() {
        let a = SearchToggles.matchKey(searching: true, shown: [.tours, .quality], forcedMores: [.quality])
        XCTAssertEqual(a, SearchToggles.matchKey(searching: true, shown: [.tours, .quality], forcedMores: [.quality]))
        XCTAssertNotEqual(a, SearchToggles.matchKey(searching: true, shown: [.quality], forcedMores: [.quality]))
        XCTAssertNotEqual(a, SearchToggles.matchKey(searching: true, shown: [.tours, .quality], forcedMores: []))
        XCTAssertEqual(SearchToggles.matchKey(searching: false, shown: [.tours], forcedMores: []), "")
    }
}
