import simd

/// A cyclic colour gradient defined by sRGB stops, sampled into a linear-RGB lookup table.
public struct Palette: Identifiable, Hashable, Sendable {
    public let id: String
    public let name: String
    /// (position 0..<1, sRGB hex). The gradient wraps from the last stop back to the first.
    let stops: [(Double, UInt32)]

    public static func == (a: Palette, b: Palette) -> Bool { a.id == b.id }
    public func hash(into h: inout Hasher) { h.combine(id) }

    public static let all: [Palette] = [
        Palette(id: "classic", name: "Classic", stops: [
            (0.0, 0x000764), (0.16, 0x206BCB), (0.42, 0xEDFFFF), (0.6425, 0xFFAA00), (0.8575, 0x000200),
        ]),
        Palette(id: "fire", name: "Fire", stops: [
            (0.0, 0x000000), (0.25, 0x7A0000), (0.5, 0xFF5A00), (0.75, 0xFFE070), (0.9, 0xFFFFFF),
        ]),
        Palette(id: "ocean", name: "Ocean", stops: [
            (0.0, 0x001018), (0.3, 0x005F73), (0.55, 0x0A9396), (0.75, 0x94D2BD), (0.9, 0xE9D8A6),
        ]),
        Palette(id: "electric", name: "Electric", stops: [
            (0.0, 0x05001A), (0.2, 0x3A0CA3), (0.4, 0x4CC9F0), (0.6, 0xF72585), (0.8, 0xFFF3B0),
        ]),
        Palette(id: "forest", name: "Forest", stops: [
            (0.0, 0x0B1A0B), (0.3, 0x2D6A4F), (0.55, 0x95D5B2), (0.75, 0xD8C99B), (0.9, 0x6B4226),
        ]),
        Palette(id: "grey", name: "Greyscale", stops: [
            (0.0, 0x000000), (0.5, 0xFFFFFF),
        ]),
        Palette(id: "rainbow", name: "Rainbow", stops: (0..<12).map { i in
            let h = Double(i) / 12
            return (h, Palette.hsvHex(h, 0.85, 1.0))
        }),
    ]

    public static let `default` = all[0]

    public static func named(_ id: String) -> Palette { all.first { $0.id == id } ?? .default }

    /// Samples the gradient into `size` linear-RGB entries.
    func table(size: Int = 1024) -> [SIMD4<Float>] {
        let s = stops.map { ($0.0, Palette.linear($0.1)) }
        return (0..<size).map { i in
            let t = Double(i) / Double(size)
            // Find the stop pair (a, b) around t, wrapping.
            var ai = s.count - 1
            for (k, stop) in s.enumerated() where stop.0 <= t { ai = k }
            let bi = (ai + 1) % s.count
            let (ta, ca) = s[ai]
            var (tb, cb) = s[bi]
            var tt = t
            if tb <= ta { tb += 1; if tt < ta { tt += 1 } }
            let f = Float((tt - ta) / max(tb - ta, 1e-9))
            // Smoothstep between stops avoids visible kinks at the stop positions.
            let w = f * f * (3 - 2 * f)
            let c = ca + (cb - ca) * w
            return SIMD4(c.x, c.y, c.z, 1)
        }
    }

    static func linear(_ hex: UInt32) -> SIMD3<Float> {
        func ch(_ v: UInt32) -> Float {
            let c = Float(v & 0xFF) / 255
            return c <= 0.04045 ? c / 12.92 : powf((c + 0.055) / 1.055, 2.4)
        }
        return SIMD3(ch(hex >> 16), ch(hex >> 8), ch(hex))
    }

    static func hsvHex(_ h: Double, _ s: Double, _ v: Double) -> UInt32 {
        let i = Int(h * 6) % 6
        let f = h * 6 - Double(Int(h * 6))
        let p = v * (1 - s), q = v * (1 - f * s), t = v * (1 - (1 - f) * s)
        let (r, g, b): (Double, Double, Double)
        switch i {
        case 0: (r, g, b) = (v, t, p)
        case 1: (r, g, b) = (q, v, p)
        case 2: (r, g, b) = (p, v, t)
        case 3: (r, g, b) = (p, q, v)
        case 4: (r, g, b) = (t, p, v)
        default: (r, g, b) = (v, p, q)
        }
        return (UInt32(r * 255) << 16) | (UInt32(g * 255) << 8) | UInt32(b * 255)
    }
}
