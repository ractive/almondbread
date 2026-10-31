//! The parameter lab in the CLI (F10, kb: plans/f10-parameter-lab): `render out.png --lab-plane
//! c|z0|p [--z0 re,im] [--c re,im] [--p re,im] [--lab-seed n] [--lab-panels 1]` renders the
//! lab's main view of that plane (its home view unless `--center` / `--scale` frame another),
//! with the knobs given (Reset's z₀ = 0, c = −0.75, p = 2 where not; `--lab-seed n`: the Random
//! button's values for seed n first). `--lab-panels 1` draws the three plane panels over it.

use almondbread_core::lab::{self, LabValues, Plane};
use almondbread_render::{GpuBackend, Renderer};

use crate::options::{Options, Result, err};

/// The lab's knobs and plane from the options, if `--lab-plane` is given.
pub fn options(o: &Options) -> Result<Option<(LabValues, Plane)>> {
    let Some(name) = o.get("lab-plane") else {
        return Ok(None);
    };
    let Some(plane) = Plane::parse(name) else {
        return err(format!("bad --lab-plane {name} (c, z0 or p)"));
    };
    let mut v = match o.get("lab-seed") {
        Some(s) => match s.parse::<u64>() {
            Ok(seed) => lab::random(seed),
            Err(_) => return err(format!("bad --lab-seed {s}")),
        },
        None => LabValues::default(),
    };
    for (key, p) in [("z0", Plane::Z0), ("c", Plane::C), ("p", Plane::P)] {
        if let Some(x) = o.pair(key)? {
            v.set(p, x);
        }
    }
    Ok(Some((v, plane)))
}

/// Turns the lab on in `r` (before the overlay is set up): the plane's home view at `width` x
/// `height` unless the options frame a view.
pub fn enter<B: GpuBackend>(r: &mut Renderer<B>, o: &Options, width: usize, height: usize) -> Result<bool> {
    let Some((values, plane)) = options(o)? else {
        return Ok(false);
    };
    let framed = o.has("center") || o.has("place") || o.has("location");
    let view = r.viewport.clone();
    r.enter_lab(values, plane, (width as f64, height as f64));
    if framed {
        r.viewport = view;
    }
    let v = values.sanitized();
    println!(
        "lab: {} plane, z0 = {}, c = {}, p = {}",
        plane.name(),
        almondbread_render::overlay::complex_text(v.z0),
        almondbread_render::overlay::complex_text(v.c),
        almondbread_render::overlay::complex_text(v.p)
    );
    Ok(true)
}
