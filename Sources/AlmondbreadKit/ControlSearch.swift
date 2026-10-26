import AlmondbreadCore
import Foundation
import Observation

/// What the control center's search finds besides sections (kb: decisions/controls "Control
/// center", Search): places of every fractal and palettes of the library, by name only
/// (case- and diacritic-insensitive substring; descriptions are not searched). Pure, so the
/// unit tests run it without a renderer.
enum ControlSearch {
    /// Whether `name` contains `query` (trimmed; case and diacritics ignored). An empty query
    /// matches nothing: names are only searched while searching.
    static func nameMatches(_ name: String, query: String) -> Bool {
        let q = query.trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return false }
        return name.range(of: q, options: [.caseInsensitive, .diacriticInsensitive]) != nil
    }

    /// What finds the pinned fractal row (`FractalRow`): it is in no section, so the search marks
    /// it when the query is part of one of these (as a section's keywords match).
    static let fractalKeywords = ["fractal", "formula", "mandelbrot", "multibrot", "mandelbar", "tricorn",
                                  "burning", "ship", "power", "exponent"]

    /// Whether the query finds the fractal row (trimmed, case-insensitive; empty: no).
    static func fractalMatches(_ query: String) -> Bool {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return false }
        return fractalKeywords.contains { $0.contains(q) }
    }

    /// The places whose name matches, in catalogue order (every formula's: `ExplorerModel.allPlaces`).
    static func places(_ all: [PlaceInfo], query: String) -> [PlaceInfo] {
        all.filter { nameMatches($0.name, query: query) }
    }

    /// The library palettes whose name matches, in library order.
    static func palettes(_ library: [LibraryPaletteInfo], query: String) -> [LibraryPaletteInfo] {
        library.filter { nameMatches($0.name, query: query) }
    }

    /// The small label on a result tile from another fractal than the current one: the family's
    /// name ("Burning Ship", "Mandelbar"), or for a family of several formulas the formula itself
    /// without "+ c" ("z⁸"). Nil for the current formula's own places.
    static func formulaLabel(_ formula: String, current: String, options: [FormulaOption]) -> String? {
        guard formula != current, let o = options.first(where: { $0.formula == formula }) else { return nil }
        let several = options.filter { $0.family == o.family }.count > 1
        guard several else { return o.familyTitle }
        let t = o.title
        return t.hasSuffix(" + c") ? String(t.dropLast(4)) : t
    }

    /// Where an arrow key moves the keyboard focus in a grid of `count` tiles in `columns`
    /// columns (left / right: the previous / next tile, up / down: a row); nil at an edge.
    static func move(from index: Int, count: Int, columns: Int, dx: Int, dy: Int) -> Int? {
        let n = index + dx + dy * max(1, columns)
        return n >= 0 && n < count && n != index ? n : nil
    }
}

/// The section headers and Mores the user opened or closed during the current search (kb:
/// decisions/controls "Control center", Search). A search shows every matching section open and
/// every More that has a matching row open; a click during the search flips that here, so the
/// stored open / closed state stays untouched and comes back when the search is cleared. Starts
/// empty whenever the matches change (another set of sections or forced Mores) and when the
/// search ends. Shared by the control center (headers) and its `MoreRows` (environment).
@Observable @MainActor
final class SearchToggles {
    /// Whether the control center is searching now (a click flips here, else the stored state).
    var searching = false
    private(set) var flipped: Set<String> = []

    /// The id of a section's More ("more.<section>"; a section's header is its raw value).
    nonisolated static func moreID(_ section: ControlSection) -> String { "more." + section.rawValue }

    func isFlipped(_ id: String) -> Bool { flipped.contains(id) }

    func toggle(_ id: String) {
        if flipped.contains(id) { flipped.remove(id) } else { flipped.insert(id) }
    }

    /// The matches changed or the search ended: every disclosure shows its search default again.
    func reset() {
        if !flipped.isEmpty { flipped = [] }
    }

    /// Whether a disclosure shows open: outside a search its stored state; during one its search
    /// default (a section: open; a More: open when a row behind it matches, else as stored)
    /// unless the user flipped it during this search.
    nonisolated static func isOpen(stored: Bool, searchDefault: Bool, searching: Bool, flipped: Bool) -> Bool {
        searching ? searchDefault != flipped : stored
    }

    /// What identifies the current matches (empty: not searching): the shown sections and the
    /// sections whose More a matching row forces open. A change starts the toggles over.
    nonisolated static func matchKey(searching: Bool, shown: [ControlSection], forcedMores: [ControlSection]) -> String {
        guard searching else { return "" }
        return shown.map(\.rawValue).joined(separator: ",") + "|" + forcedMores.map(\.rawValue).joined(separator: ",")
    }
}
