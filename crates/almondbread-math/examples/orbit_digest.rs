//! Digest of reference-orbit outputs (float32 points, floatexp points, final Z at full
//! precision) over a fixed set of points and precisions, to check that a change to the
//! arithmetic keeps orbits bit-identical:
//!
//!   cargo run --release -p almondbread-math --example orbit_digest > before.txt
//!   (change)  ... > after.txt; diff before.txt after.txt

use almondbread_math::*;

fn fnv(h: &mut u64, v: u64) {
    for b in v.to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100_0000_01b3);
    }
}

fn digest(c: &BigComplex, bits: usize, iters: usize, variant: Variant) -> (u64, usize) {
    let comp = ReferenceOrbitComputer::new(c, bits).with_variant(variant);
    let (pts, ext, r) = comp.compute_vec(iters, 1e10);
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for p in &pts {
        fnv(&mut h, p[0].to_bits() as u64);
        fnv(&mut h, p[1].to_bits() as u64);
    }
    for e in &ext {
        fnv(&mut h, e.mantissa[0].to_bits() as u64);
        fnv(&mut h, e.mantissa[1].to_bits() as u64);
        fnv(&mut h, e.exponent as u64);
    }
    fnv(&mut h, r.final_z.re.to_decimal_string(None).len() as u64);
    for ch in r
        .final_z
        .re
        .to_decimal_string(None)
        .bytes()
        .chain(r.final_z.im.to_decimal_string(None).bytes())
    {
        fnv(&mut h, ch as u64);
    }
    (h, r.count)
}

fn main() {
    let points: [(&str, &str, &str); 5] = [
        (
            "full",
            "-0.743643887037158704752191506114774123456789012345678901234567890123456789",
            "0.131825904205311970493132056385139987654321098765432109876543210987654321",
        ),
        ("i", "0", "1"),
        ("needle", "-1.99999999999999999999999999999999999988", "0"),
        (
            "cardioid",
            "-0.500000000000000000001234567890123456789012345678901234567891",
            "0.500000000000000000000987654321098765432109876543210987654321",
        ),
        ("p3", "-1.754877666246692760049508896358528691894606617772793143", "0"),
    ];
    let sizes = [
        64usize, 128, 192, 256, 448, 512, 1024, 1152, 2048, 3456, 4096, 6784, 10048,
    ];
    for (name, re, im) in points {
        for &bits in &sizes {
            let c = BigComplex::parse(re, im, bits + 64).unwrap();
            // Fill the low bits too (a "random" deep point), except for the exact ones.
            let c = if name == "full" {
                let mut c = c;
                c.re += BigFloat::from_f64(0.318309886183790671, bits).times_power_of_two(-(bits as i64 - 3));
                c.im -= BigFloat::from_f64(0.577215664901532860, bits).times_power_of_two(-(bits as i64 - 5));
                c
            } else {
                c
            };
            let iters = (40_000_000 / bits).clamp(1000, 20_000);
            for v in [Variant::Squarings, Variant::Multiplications, Variant::Automatic] {
                let (h, count) = digest(&c, bits, iters, v);
                println!("{name:>9} {bits:>6} {v:?}: {count} {h:016x}");
            }
        }
    }
}
