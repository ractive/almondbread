#!/usr/bin/env python3
"""Independent oracle for the almondbread-math tests (tests/oracle.rs).

Writes JSON fixtures into ../fixtures. Arithmetic, parsing, formatting and the reference orbit
use exact Python integers/fractions with an explicit round-to-nearest-even step, so the
expected results are bit-exact. Nuclei use mpmath (Newton at higher precision).

Value encoding: [negative, significand_hex, exponent] meaning (-1)^negative * S * 2^exponent,
with S normalised to exactly `prec` bits; zero is null.

Run:  python3 -m venv venv && venv/bin/pip install mpmath && venv/bin/python generate_fixtures.py
      (`--only-multibrot` writes just multibrot.json, the z^p + c orbits and nuclei of F3)
"""
import json
import math
import struct
import os
import random
import sys
from fractions import Fraction

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures")
rng = random.Random(20261014)


# ---------- exact rounding ----------

def round_int(m, e, prec):
    """Round the exact value m * 2^e (m signed int) to prec bits, ties to even. Returns (m', e')
    with |m'| < 2^prec (not normalised)."""
    if m == 0:
        return 0, 0
    s = -1 if m < 0 else 1
    a = abs(m)
    bl = a.bit_length()
    if bl <= prec:
        return m, e
    sh = bl - prec
    q = a >> sh
    r = a & ((1 << sh) - 1)
    half = 1 << (sh - 1)
    if r > half or (r == half and (q & 1)):
        q += 1
    return s * q, e + sh


def round_frac(fr, prec):
    """Round a Fraction to prec bits, ties to even -> (m, e) signed."""
    if fr == 0:
        return 0, 0
    s = -1 if fr < 0 else 1
    n, d = abs(fr.numerator), fr.denominator
    e = n.bit_length() - d.bit_length() - prec
    while True:
        if e >= 0:
            q, r = divmod(n, d << e)
            den = d << e
        else:
            q, r = divmod(n << -e, d)
            den = d
        if q >= (1 << prec):
            e += 1
            continue
        if q < (1 << (prec - 1)):
            e -= 1
            continue
        break
    if 2 * r > den or (2 * r == den and (q & 1)):
        q += 1
    if q == (1 << prec):
        q >>= 1
        e += 1
    return s * q, e


def enc(m, e, prec):
    """Encode (m, e) normalised to exactly prec bits."""
    if m == 0:
        return None
    a = abs(m)
    bl = a.bit_length()
    assert bl <= prec
    a <<= prec - bl
    e -= prec - bl
    return [m < 0, format(a, "x"), e]


def frac_of(m, e):
    return Fraction(m) * (Fraction(2) ** e)


# ---------- random operands ----------

PRECS = [64, 128, 192, 256, 320, 448, 512, 1024, 2048, 4096]


def rand_sig(prec, kind):
    if kind == "ones":
        return (1 << prec) - 1
    if kind == "pow2":
        return 1 << (prec - 1)
    if kind == "sparse":
        v = 1 << (prec - 1)
        for _ in range(3):
            v |= 1 << rng.randrange(prec)
        return v
    return rng.getrandbits(prec) | (1 << (prec - 1))


def rand_value(prec):
    kind = rng.choice(["rand"] * 6 + ["ones", "pow2", "sparse"])
    m = rand_sig(prec, kind)
    if rng.random() < 0.5:
        m = -m
    e = rng.randrange(-prec - 200, 200) - prec
    return m, e


def related_value(prec, m, e):
    """b close to a: cancellation in a - b, carries in a + b."""
    k = rng.choice(["near", "shift1", "shift2", "far", "veryfar", "same", "neg"])
    if k == "near":
        delta = rng.getrandbits(rng.randrange(1, prec))
        return m + (delta if rng.random() < 0.5 else -delta), e
    if k == "shift1":
        mm, ee = rand_value(prec)
        return mm, e + 1 + rng.randrange(-1, 2)
    if k == "shift2":
        mm, _ = rand_value(prec)
        return mm, e + rng.randrange(-3 * 64, 3 * 64)
    if k == "far":
        mm, _ = rand_value(prec)
        return mm, e - prec - rng.randrange(0, 130)
    if k == "veryfar":
        mm, _ = rand_value(prec)
        return mm, e + rng.choice([-1, 1]) * (prec * 3 + rng.randrange(1000))
    if k == "same":
        return m, e
    return -m, e


def arith_cases(count):
    cases = []
    for _ in range(count):
        prec = rng.choice(PRECS)
        am, ae = rand_value(prec)
        if rng.random() < 0.6:
            bm, be = related_value(prec, am, ae)
        else:
            bm, be = rand_value(prec)
        # Operands themselves must be representable at prec.
        am, ae = round_int(am, ae, prec)
        bm, be = round_int(bm, be, prec)
        if bm == 0:
            bm, be = 1, 0
        a = frac_of(am, ae)
        b = frac_of(bm, be)

        def r(fr):
            m, e = round_frac(fr, prec)
            return enc(m, e, prec)

        small = rng.choice([3, 10, 1000000007, 2**63 - 1, -7, -(2**40 + 1)])
        cases.append({
            "prec": prec,
            "a": enc(am, ae, prec),
            "b": enc(bm, be, prec),
            "add": r(a + b),
            "sub": r(a - b),
            "mul": r(a * b),
            "sqr": r(a * a),
            "div": r(a / b),
            "small": small,
            "mulSmall": r(a * small),
            "doubleBits": format(struct.unpack("<Q", struct.pack("<d", float(a)))[0], "016x") if abs(ae + prec) < 1000 else None,
        })
    return cases


# ---------- decimal ----------

def parse_decimal(s):
    """Exact Fraction of a decimal string (same grammar as the Swift parser)."""
    t = s.strip()
    neg = t.startswith("-")
    if t[:1] in "+-":
        t = t[1:]
    exp = 0
    if "e" in t.lower():
        i = t.lower().index("e")
        exp = int(t[i + 1:])
        t = t[:i]
    if "." in t:
        ip, fp = t.split(".")
    else:
        ip, fp = t, ""
    digits = (ip + fp) or "0"
    v = Fraction(int(digits)) * Fraction(10) ** (exp - len(fp))
    return -v if neg else v


def round_trip_digits(prec):
    return (prec * 30103 + 99999) // 100000 + 1


def format_decimal(fr, digits):
    """Mirror of BigFloat.toDecimalString."""
    if fr == 0:
        return "0"
    neg = fr < 0
    a = abs(fr)
    # floor(log10 a) by search
    x10 = len(str(a.numerator)) - len(str(a.denominator))
    while True:
        k = digits - 1 - x10
        v = a * Fraction(10) ** k
        q, r = divmod(v.numerator, v.denominator)
        # Choose x10 = floor(log10 a) on the truncated quotient, then round.
        if q >= 10 ** digits:
            x10 += 1
            continue
        if q < 10 ** (digits - 1):
            x10 -= 1
            continue
        if 2 * r > v.denominator or (2 * r == v.denominator and q & 1):
            q += 1
        if q == 10 ** digits:
            q = 10 ** (digits - 1)
            x10 += 1
        break
    s = str(q).rstrip("0") or "0"
    out = "-" if neg else ""
    if -7 < x10 < 21:
        if x10 >= 0:
            ic = x10 + 1
            if len(s) <= ic:
                out += s + "0" * (ic - len(s))
            else:
                out += s[:ic] + "." + s[ic:]
        else:
            out += "0." + "0" * (-x10 - 1) + s
    else:
        out += s[0]
        if len(s) > 1:
            out += "." + s[1:]
        out += ("e-%d" % -x10) if x10 < 0 else ("e+%d" % x10)
    return out


def rand_decimal_string():
    nd = rng.choice([1, 3, 17, 20, 40, 80, 200, 700])
    digits = "".join(rng.choice("0123456789") for _ in range(nd))
    if rng.random() < 0.2:
        digits = "000" + digits
    sign = rng.choice(["", "-", "+"])
    style = rng.choice(["int", "frac", "exp", "lead"])
    if style == "int":
        s = sign + digits
    elif style == "frac":
        p = rng.randrange(0, len(digits) + 1)
        s = sign + digits[:p] + "." + digits[p:]
        if s.endswith(".") and p == 0:
            s += "0"
    elif style == "lead":
        s = sign + "0." + digits
    else:
        p = rng.randrange(1, len(digits) + 1)
        s = sign + digits[:p] + "." + digits[p:] + rng.choice(["e", "E"]) + str(rng.randrange(-1200, 400))
    return s


def decimal_cases(count):
    cases = []
    fixed = ["-0.7436438870371587047", "0", "1", "-1.7548776662466927", "1e-300", "0.1",
             "-1.9407998065294848", "123456789012345678901234567890", "2.5", "3.5", "-0.5e1"]
    strings = fixed + [rand_decimal_string() for _ in range(count)]
    for s in strings:
        prec = rng.choice(PRECS)
        fr = parse_decimal(s)
        m, e = round_frac(fr, prec)
        cases.append({"text": s, "prec": prec, "value": enc(m, e, prec)})
    return cases


def format_cases(count):
    cases = []
    for _ in range(count):
        prec = rng.choice(PRECS)
        m, e = rand_value(prec)
        e = rng.choice([e, -prec - rng.randrange(0, 10), -prec + rng.randrange(0, 60)])
        fr = frac_of(m, e)
        digits = rng.choice([None, None, 1, 5, 17, 30])
        d = digits if digits is not None else round_trip_digits(prec)
        cases.append({"prec": prec, "value": enc(m, e, prec), "digits": digits,
                      "text": format_decimal(fr, d)})
    return cases


# ---------- reference orbit (bit-exact emulation of both Swift formulations) ----------

def orbit(cx, cy, prec, iters, variant):
    R = lambda m, e: round_int(m, e, prec)

    def add(a, b):
        (am, ae), (bm, be) = a, b
        if am == 0:
            return b
        if bm == 0:
            return a
        e = min(ae, be)
        return R((am << (ae - e)) + (bm << (be - e)), e)

    def neg(a):
        return (-a[0], a[1])

    def mul(a, b):
        return R(a[0] * b[0], a[1] + b[1])

    def tod(a):
        m, e = a
        if m == 0:
            return 0.0
        sh = max(0, abs(m).bit_length() - 64)
        t = abs(m) >> sh
        v = math.ldexp(float(t), max(-1100, min(1100, e + sh)))
        return -v if m < 0 else v

    x, y = (0, 0), (0, 0)
    C = (cx, cy)
    count = 0
    for _ in range(iters):
        if variant == "squarings":
            x2 = mul(x, x)
            y2 = mul(y, y)
            s = add(x, y)
            s2 = mul(s, s)
            nrm = add(x2, y2)
            x = add(add(x2, neg(y2)), C[0])
            y = add(add(s2, neg(nrm)), C[1])
        else:
            s = add(x, y)
            d = add(x, neg(y))
            p1 = mul(s, d)
            p2 = mul(x, y)
            p2 = (p2[0], p2[1] + 1) if p2[0] else p2
            x = add(p1, C[0])
            y = add(p2, C[1])
        count += 1
        xd, yd = tod(x), tod(y)
        if xd * xd + yd * yd > 4.0:
            break
    return x, y, count


def orbit_cases():
    out = []
    locs = [
        ("-0.743643887037158704752191506114774", "0.131825904205311970493132056385139", 128, 3000),
        ("-1.7548776662466927600495088963585286918946", "0", 192, 2000),
        ("-0.7436438870371587047521915061147741186045822", "0.1318259042053119704931320563851393183398", 256, 2000),
        ("-1.25066", "0.02012", 64, 5000),
        ("0.25000000000000000000000000000000000000000000000000000000000000000001", "0.0000001", 512, 1500),
        ("-0.77568377", "0.13646737", 1024, 700),
        ("-1.999985881", "0.000000000000000000000000000000001", 4096, 120),
    ]
    for re_s, im_s, prec, iters in locs:
        cx = round_frac(parse_decimal(re_s), prec)
        cy = round_frac(parse_decimal(im_s), prec)
        case = {"re": re_s, "im": im_s, "prec": prec, "iterations": iters}
        for v in ("squarings", "multiplications"):
            x, y, cnt = orbit(cx, cy, prec, iters, v)
            case[v] = [enc(x[0], x[1], prec), enc(y[0], y[1], prec)]
            case[v + "Count"] = cnt
        # High-precision orbit for the accuracy check.
        hx, hy, _ = orbit(cx, cy, prec + 256, iters, "multiplications")
        hd = int((prec + 256) * 0.30103)
        case["high"] = [format_decimal(frac_of(*hx), hd) if hx[0] else "0",
                        format_decimal(frac_of(*hy), hd) if hy[0] else "0"]
        out.append(case)
    return out


# ---------- nuclei (mpmath) ----------

def nucleus_cases():
    try:
        import mpmath
    except ImportError:
        print("mpmath not installed; skipping nucleus fixtures", file=sys.stderr)
        return []
    mp = mpmath.mp

    def newton(c, p, prec):
        mp.prec = prec
        c = mpmath.mpc(c)
        for _ in range(200):
            z = mpmath.mpc(0)
            dz = mpmath.mpc(0)
            for _ in range(p):
                dz = 2 * z * dz + 1
                z = z * z + c
            step = z / dz
            c = c - step
            if abs(step) < mpmath.mpf(2) ** (-prec + 20):
                break
        return c

    def ball_period(c, r, maxp, prec):
        mp.prec = prec
        z = mpmath.mpc(0)
        R = mpmath.mpf(0)
        r = mpmath.mpf(r)
        for p in range(1, maxp + 1):
            R = 2 * abs(z) * R + R * R + r
            z = z * z + c
            if abs(z) < R:
                return p
            if abs(z) - R > 1e10:
                return None
        return None

    cases = []
    mp.prec = 300
    starts = [
        ("-1.75", "0", "1e-3", 3, 128),
        ("-1.94", "0", "1e-3", 4, 128),
        ("-0.1225611668766536", "0.7448617666197442", "1e-6", None, 128),
        ("-0.743643887037158704752191506114774", "0.131825904205311970493132056385139", "1e-25", None, 192),
        ("-0.743643887037158704752191506114774", "0.131825904205311970493132056385139", "1e-32", None, 256),
    ]
    # Deep real-axis minibrots near -2 (period p, size ~4^-2p): nucleus by Newton from -2,
    # size by mathr's estimate, then a start point offset by 0.3*size*(1+i) and radius = size.
    for p, prec in [(60, 512), (120, 1024), (250, 2048)]:
        mp.prec = prec + 128
        nuc = newton(mpmath.mpc(-2), p, prec + 128)
        z = mpmath.mpc(0); l = mpmath.mpc(1); b = mpmath.mpc(1)
        for _ in range(1, p):
            z = z * z + nuc
            l = 2 * z * l
            b = b + 1 / l
        size = abs(1 / (b * l * l))
        start = nuc + mpmath.mpf("0.3") * size * mpmath.mpc(1, 1)
        digits = int(prec * 0.30103) + 5
        starts.append((mpmath.nstr(start.real, digits, min_fixed=-1, max_fixed=-1),
                       mpmath.nstr(start.imag, 25, min_fixed=-1, max_fixed=-1),
                       mpmath.nstr(size, 17, min_fixed=-1, max_fixed=-1), None, prec))
    for re_s, im_s, r_s, p, prec in starts:
        mp.prec = prec + 64
        c0 = mpmath.mpc(mpmath.mpf(re_s), mpmath.mpf(im_s))
        bp = ball_period(c0, mpmath.mpf(r_s), 200000, prec + 64)
        if p is None:
            p = bp
        n = newton(c0, p, prec + 128)
        mp.prec = prec + 128
        cases.append({"re": re_s, "im": im_s, "radius": r_s, "period": p, "ballPeriod": bp, "prec": prec,
                      "nucleusRe": mpmath.nstr(n.real, int(prec * 0.30103) + 10, min_fixed=-1, max_fixed=-1),
                      "nucleusIm": mpmath.nstr(n.imag, int(prec * 0.30103) + 10, min_fixed=-1, max_fixed=-1)})
        print("nucleus", re_s, im_s, r_s, "period", p, file=sys.stderr)
    return cases


# ---------- Multibrot z^p + c (F3) ----------

def orbit_power(cx, cy, prec, iters, power):
    """Bit-exact emulation of the generic step (orbit.rs `orbit_loop_power`): w = z, then
    p - 1 times w <- w z (four rounded products, two rounded sums), then z = w + c."""
    R = lambda m, e: round_int(m, e, prec)

    def add(a, b):
        (am, ae), (bm, be) = a, b
        if am == 0:
            return b
        if bm == 0:
            return a
        e = min(ae, be)
        return R((am << (ae - e)) + (bm << (be - e)), e)

    def neg(a):
        return (-a[0], a[1])

    def mul(a, b):
        return R(a[0] * b[0], a[1] + b[1])

    def tod(a):
        m, e = a
        if m == 0:
            return 0.0
        sh = max(0, abs(m).bit_length() - 64)
        t = abs(m) >> sh
        v = math.ldexp(float(t), max(-1100, min(1100, e + sh)))
        return -v if m < 0 else v

    x, y = (0, 0), (0, 0)
    count = 0
    for _ in range(iters):
        wx, wy = x, y
        for _ in range(power - 1):
            p1 = mul(wx, x)
            p2 = mul(wy, y)
            p3 = mul(wx, y)
            p4 = mul(wy, x)
            wx = add(p1, neg(p2))
            wy = add(p3, p4)
        x = add(wx, cx)
        y = add(wy, cy)
        count += 1
        xd, yd = tod(x), tod(y)
        if xd * xd + yd * yd > 4.0:
            break
    return x, y, count


def multibrot_cases():
    """Reference orbits (bit-exact) and nuclei (mpmath) of z^p + c for p = 3..8: the oracle of
    `Formula::Multibrot(p)` (kb: plans/f3-multibrot "Verification")."""
    import mpmath
    mp = mpmath.mp
    out = {"orbit": [], "nuclei": []}

    def escapes(c, p, n):
        z = mpmath.mpc(0)
        for _ in range(n):
            z = z ** p + c
            if abs(z) > 2:
                return True
        return False

    def boundary(p, angle):
        # Bisect along the ray at `angle` from 0 (inside) outwards (2: outside).
        mp.prec = 80
        lo, hi = mpmath.mpf(0), mpmath.mpf(2)
        u = mpmath.expjpi(mpmath.mpf(angle) / mpmath.pi)
        for _ in range(40):
            mid = (lo + hi) / 2
            if escapes(mid * u, p, 3000):
                hi = mid
            else:
                lo = mid
        return lo * u

    def ball_period(c, r, maxp, prec, p):
        mp.prec = prec
        z = mpmath.mpc(0)
        R = mpmath.mpf(0)
        r = mpmath.mpf(r)
        for k in range(1, maxp + 1):
            R = (abs(z) + R) ** p - abs(z) ** p + r
            z = z ** p + c
            if abs(z) < R:
                return k
            if abs(z) - R > 1e10:
                return None
        return None

    def newton(c, k, prec, p):
        mp.prec = prec
        c = mpmath.mpc(c)
        for _ in range(100):
            z = mpmath.mpc(0)
            dz = mpmath.mpc(0)
            for _ in range(k):
                dz = p * z ** (p - 1) * dz + 1
                z = z ** p + c
            step = z / dz
            c = c - step
            if abs(step) < mpmath.mpf(2) ** (-prec + 20):
                break
        return c

    for p in range(3, 9):
        angle = 0.37 + 0.11 * p
        cb = boundary(p, angle)
        re_s = mpmath.nstr(cb.real, 20, min_fixed=-1, max_fixed=-1)
        im_s = mpmath.nstr(cb.imag, 20, min_fixed=-1, max_fixed=-1)
        # Orbits: the boundary point (long, bounded or slow) and a point just outside it.
        for (ore, oim, prec, iters) in [(re_s, im_s, 128, 1500), (re_s, im_s, 1024, 300),
                                        (mpmath.nstr(cb.real * 1.01, 20), mpmath.nstr(cb.imag * 1.01, 20), 256, 3000)]:
            cx = round_frac(parse_decimal(ore), prec)
            cy = round_frac(parse_decimal(oim), prec)
            x, y, cnt = orbit_power(cx, cy, prec, iters, p)
            hx, hy, _ = orbit_power(round_frac(parse_decimal(ore), prec + 256),
                                    round_frac(parse_decimal(oim), prec + 256), prec + 256, iters, p)
            hd = int((prec + 256) * 0.30103)
            out["orbit"].append({
                "power": p, "re": ore, "im": oim, "prec": prec, "iterations": iters,
                "z": [enc(x[0], x[1], prec), enc(y[0], y[1], prec)], "count": cnt,
                "high": [format_decimal(frac_of(*hx), hd) if hx[0] else "0",
                         format_decimal(frac_of(*hy), hd) if hy[0] else "0"],
            })
        # Nuclei: the lowest period touching discs around the boundary point; the deep one
        # (radius 1e-12 at 1024 bits) runs Newton into its exact finish.
        # Only discs whose nucleus Newton finds within 4 radii (plain Newton from a poor start
        # wanders; the fixture is about accuracy, not basins).
        for radii, prec in [(["1e-4", "1e-5", "1e-6", "1e-7"], 128), (["1e-12", "1e-13", "1e-14"], 1024)]:
            for r_s in radii:
                mp.prec = prec + 64
                c0 = mpmath.mpc(mpmath.mpf(re_s), mpmath.mpf(im_s))
                bp = ball_period(c0, mpmath.mpf(r_s), 20000, prec + 64, p)
                if bp is None:
                    continue
                n = newton(c0, bp, prec + 128, p)
                mp.prec = prec + 128
                if abs(n - c0) < 4 * mpmath.mpf(r_s):
                    break
            else:
                print("multibrot", p, prec, "no nucleus near", file=sys.stderr)
                continue
            out["nuclei"].append({
                "power": p, "re": re_s, "im": im_s, "radius": r_s, "period": bp, "prec": prec,
                "nucleusRe": mpmath.nstr(n.real, int(prec * 0.30103) + 10, min_fixed=-1, max_fixed=-1),
                "nucleusIm": mpmath.nstr(n.imag, int(prec * 0.30103) + 10, min_fixed=-1, max_fixed=-1)})
            print("multibrot", p, "nucleus", re_s, im_s, r_s, "period", bp, file=sys.stderr)
    return out


def main():
    os.makedirs(OUT, exist_ok=True)
    if "--only-multibrot" in sys.argv:
        # F3: only the Multibrot fixtures (the others stay as generated).
        with open(os.path.join(OUT, "multibrot.json"), "w") as f:
            json.dump(multibrot_cases(), f, indent=0)
        return
    data = {
        "arith": arith_cases(1500),
        "parse": decimal_cases(400),
        "format": format_cases(400),
        "orbit": orbit_cases(),
        "nuclei": nucleus_cases(),
    }
    try:
        data["multibrot"] = multibrot_cases()
    except ImportError:
        print("mpmath not installed; skipping multibrot fixtures", file=sys.stderr)
    for k, v in data.items():
        with open(os.path.join(OUT, k + ".json"), "w") as f:
            json.dump(v, f, indent=0)
        print(k, len(v), file=sys.stderr)


if __name__ == "__main__":
    main()
