import AlmondbreadCore
import GameController

/// Game controllers (GameController framework, macOS and iOS) decoded into
/// `InputEvent.gamepad` (kb: decisions/controls, gamepad): raw stick and trigger values,
/// sent whenever the controller reports a change. Dead zones, response curves, speeds and
/// easing are Rust's `Navigator`; nothing here interprets motion.
///
/// Mapping: left stick pans, right trigger / left trigger zoom in / out, right stick up /
/// down zooms too, A (south button) is the speed boost, the shoulders are the cruise
/// pedals like Q / E, Y resets the view.
@MainActor
final class GamepadInput {
    private let send: (InputEvent) -> Void
    private var observers: [NSObjectProtocol] = []
    private var last: [Double] = []
    private var lastBoost = false

    init(send: @escaping (InputEvent) -> Void) {
        self.send = send
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: .GCControllerDidConnect, object: nil, queue: .main) { [weak self] n in
            MainActor.assumeIsolated {
                if let c = n.object as? GCController { self?.configure(c) }
            }
        })
        observers.append(center.addObserver(forName: .GCControllerDidDisconnect, object: nil, queue: .main) { [weak self] _ in
            // Centred state: motion eases out instead of continuing forever.
            MainActor.assumeIsolated { self?.release() }
        })
        GCController.controllers().forEach(configure)
    }

    deinit {
        observers.forEach(NotificationCenter.default.removeObserver)
    }

    private func configure(_ controller: GCController) {
        guard let pad = controller.extendedGamepad else { return }
        // Handlers run on the controller's handler queue, the main queue by default.
        pad.valueChangedHandler = { [weak self] pad, element in
            MainActor.assumeIsolated { self?.changed(pad, element) }
        }
    }

    private func release() {
        last = []
        lastBoost = false
        send(.gamepad(lx: 0, ly: 0, rx: 0, ry: 0, lt: 0, rt: 0, boost: false))
    }

    private func changed(_ pad: GCExtendedGamepad, _ element: GCControllerElement) {
        // Buttons with a discrete meaning, on press.
        if element === pad.leftShoulder, pad.leftShoulder.isPressed { tap(.slowerZoom) }
        if element === pad.rightShoulder, pad.rightShoulder.isPressed { tap(.fasterZoom) }
        if element === pad.buttonY, pad.buttonY.isPressed { tap(.reset) }
        let axes = [pad.leftThumbstick.xAxis.value, pad.leftThumbstick.yAxis.value,
                    pad.rightThumbstick.xAxis.value, pad.rightThumbstick.yAxis.value,
                    pad.leftTrigger.value, pad.rightTrigger.value].map(Double.init)
        let boost = pad.buttonA.isPressed
        guard axes != last || boost != lastBoost else { return }
        last = axes
        lastBoost = boost
        send(.gamepad(lx: axes[0], ly: axes[1], rx: axes[2], ry: axes[3], lt: axes[4], rt: axes[5], boost: boost))
    }

    private func tap(_ key: NavKey) {
        send(.keyDown(key: key))
        send(.keyUp(key: key))
    }
}
