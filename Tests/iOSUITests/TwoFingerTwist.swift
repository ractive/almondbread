import XCTest

/// A two-finger twist of exactly the given angle, synthesised through XCTest's event record
/// (kb: decisions/controls "Touch", QA pass "twist dead band").
///
/// `XCUIElement.rotate(_:withVelocity:)` cannot test a small twist: on iOS 26 its fingers come
/// down on a line 18.4° (atan 1/3) off and jump onto the start line 10 ms later, then turn in 12°
/// keyframes of which the last one is dropped. `rotate(0.15)` (8.6°) is therefore that 18.4° jump
/// and nothing else, and UIKit measures the 10 ms jump as 12.3° or 24.6° (same synthesised
/// events, timing-dependent): past the 15° dead band in the second case. This path starts on its
/// own line and turns smoothly, so UIKit sees the angle asked for.
///
/// `XCPointerEventPath` / `XCSynthesizedEventRecord` are XCTest's own classes (the ones `rotate`
/// records, see the xcresult's "Synthesized Event" attachments), not public API: test-only use.
enum TwoFingerTwist {
    /// Turns two fingers `radians` (clockwise on screen positive, like `rotate`) about `center`
    /// (screen points, portrait) at `radius` from it over `duration` seconds. Returns false if
    /// XCTest's event classes are missing (the caller fails the test).
    @discardableResult
    static func perform(center: CGPoint, radius: CGFloat, radians: Double, duration: Double, steps: Int = 30) -> Bool {
        guard let pathClass = NSClassFromString("XCPointerEventPath"),
              let recordClass = NSClassFromString("XCSynthesizedEventRecord") else { return false }
        typealias InitTouch = @convention(c) (AnyObject, Selector, CGPoint, Double) -> AnyObject?
        typealias MoveTo = @convention(c) (AnyObject, Selector, CGPoint, Double) -> Void
        typealias LiftUp = @convention(c) (AnyObject, Selector, Double) -> Void
        typealias InitRecord = @convention(c) (AnyObject, Selector, NSString, Int) -> AnyObject?
        typealias AddPath = @convention(c) (AnyObject, Selector, AnyObject) -> Void
        typealias Synthesize = @convention(c) (AnyObject, Selector, UnsafeMutablePointer<NSError?>?) -> Bool
        func method<T>(_ cls: AnyClass, _ name: String, _: T.Type) -> (Selector, T)? {
            let sel = NSSelectorFromString(name)
            guard class_getInstanceMethod(cls, sel) != nil, let imp = class_getMethodImplementation(cls, sel) else { return nil }
            return (sel, unsafeBitCast(imp, to: T.self))
        }
        // alloc's +1 is never balanced: a few leaked objects per test run, no over-release.
        func alloc(_ cls: AnyClass) -> AnyObject? {
            (cls as AnyObject).perform(NSSelectorFromString("alloc"))?.takeUnretainedValue()
        }
        guard let (initTouchSel, initTouch) = method(pathClass, "initForTouchAtPoint:offset:", InitTouch.self),
              let (moveSel, move) = method(pathClass, "moveToPoint:atOffset:", MoveTo.self),
              let (liftSel, lift) = method(pathClass, "liftUpAtOffset:", LiftUp.self),
              let (initRecordSel, initRecord) = method(recordClass, "initWithName:interfaceOrientation:", InitRecord.self),
              let (addSel, add) = method(recordClass, "addPointerEventPath:", AddPath.self),
              let (synthSel, synthesize) = method(recordClass, "synthesizeWithError:", Synthesize.self)
        else { return false }

        let at = { (finger: Int, t: Double) -> CGPoint in
            // Finger 0 starts left of the centre, finger 1 right; screen y is down, so a growing
            // angle turns them clockwise on screen.
            let a = Double(finger) * .pi + t * radians + .pi
            return CGPoint(x: center.x + radius * CGFloat(cos(a)), y: center.y + radius * CGFloat(sin(a)))
        }
        guard let record = alloc(recordClass).flatMap({ initRecord($0, initRecordSel, "twist" as NSString, 1) }) else { return false }
        for finger in 0..<2 {
            guard let path = alloc(pathClass).flatMap({ initTouch($0, initTouchSel, at(finger, 0), 0) }) else { return false }
            for i in 1...steps {
                let t = Double(i) / Double(steps)
                move(path, moveSel, at(finger, t), t * duration)
            }
            // Held still a moment at the end: the lift adds no turn (or velocity).
            lift(path, liftSel, duration + 0.15)
            add(record, addSel, path)
        }
        var error: NSError?
        return synthesize(record, synthSel, &error) && error == nil
    }
}
