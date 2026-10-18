import XCTest

/// The throttle, a position lever (kb: decisions/controls "Touch"), as the UI tests drive it.
/// With `ALMONDBREAD_UITEST=1` its accessibility value carries its layout: "gear 3 in; ticks
/// 1,2,4,7,14,26 pitch 11.20 cap 26" (the ticks' gears on one side from the middle out, their
/// spacing and the chevrons' cells, in points).
struct ThrottleLeverUI {
    let element: XCUIElement

    private var value: String { element.value as? String ?? "" }

    /// The gears the ticks stand for (one side).
    var ticks: [Int] {
        guard let r = value.range(of: "ticks ") else { return [] }
        return value[r.upperBound...].prefix { $0 != " " }.split(separator: ",").compactMap { Int($0) }
    }

    var pitch: Double { number(after: "pitch ") ?? 11 }
    var cap: Double { number(after: "cap ") ?? 26 }

    /// The gear the knob shows (signed; 0 stopped).
    var gear: Int {
        let v = value
        guard let r = v.range(of: "gear ") else { return 0 }
        let n = Int(v[r.upperBound...].prefix { $0.isNumber }) ?? 0
        return v.contains(" out") ? -n : n
    }

    /// The knob's gear for signed tick `k` (0: the middle).
    func gear(tick k: Int) -> Int {
        let t = ticks
        guard k != 0, !t.isEmpty else { return 0 }
        let g = t[min(abs(k), t.count) - 1]
        return k > 0 ? g : -g
    }

    private var middle: XCUICoordinate { element.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)) }

    /// A drag from the middle (M) to signed tick `k`, released there.
    func drag(toTick k: Int) {
        let to = middle.withOffset(CGVector(dx: 0, dy: -Double(k) * pitch))
        middle.press(forDuration: 0.05, thenDragTo: to, withVelocity: 120, thenHoldForDuration: 0.2)
    }

    /// A tap on signed tick `k`.
    func tap(tick k: Int) {
        middle.withOffset(CGVector(dx: 0, dy: -Double(k) * pitch)).tap()
    }

    /// A tap on the chevron at the top (`up`) or bottom end.
    func tapChevron(up: Bool) {
        let end = element.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: up ? 0 : 1))
        end.withOffset(CGVector(dx: 0, dy: up ? cap / 2 : -cap / 2)).tap()
    }

    private func number(after key: String) -> Double? {
        let v = value
        guard let r = v.range(of: key) else { return nil }
        return Double(v[r.upperBound...].prefix { $0 != " " && $0 != ";" })
    }
}
