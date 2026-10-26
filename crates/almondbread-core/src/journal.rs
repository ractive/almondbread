//! The travel journal (kb: decisions/controls "Travel journal"): every place the user came to
//! rest at after moving away, with back and forward like undo and redo (⌘Z, ⇧⌘Z / ⌘Y).
//!
//! An entry is a location only: the view (centre at full precision, scale, angle) and the
//! fractal (formula with its parameters, a Julia set's k). Looks, colours, interior and
//! iteration settings are not part of it: going back is travel, never a settings undo.
//!
//! **Recording** (all decided here; the [`crate::input::Navigator`] feeds it once per frame
//! from `advance` and marks travel from `jumped`, `fly_to` and `note_travel`):
//!
//! - *Travel* (a jump: places, pasted or typed coordinates, a Julia set opened or left, a tour
//!   step, home; a flight: fly-to, seek minibrot, a place's Fly, a map's Option-click): the view
//!   it leaves is recorded first when it is [`far`] from the current entry (or the journal is
//!   empty), and the view it comes to rest at is recorded on arrival.
//! - *Autonomous motion* (autopilot, journey, cruise): its start records the view it leaves as
//!   travel does, the view where it stops (or pauses) is recorded once at rest.
//! - *Manual motion* (pan, zoom, pinch, keys): a view at rest for [`DWELL_SECONDS`] with the hand
//!   off that is [`far`] from the current entry. The hand on a gesture (trackpad fingers resting
//!   mid-scroll, a drag held still, a finger down) is not rest.
//! - Consecutive entries at the [`same_place`] merge: the newer replaces the older.
//! - A new entry while the journal is back in its history drops the forward entries (undo
//!   semantics). Back and forward themselves record nothing.
//! - At most [`CAPACITY`] entries; the oldest go first.
//!
//! Per frame the work is a comparison of the view with the last one seen (a clone only when it
//! changed); the distance tests run only at transitions and once per rest.

use crate::formula::{Formula, JuliaK};
use crate::viewport::{Center, Scale, Viewport};

/// Entries kept (and persisted).
pub const CAPACITY: usize = 100;
/// A manual move settles into an entry after this long at rest (s), hand off.
pub const DWELL_SECONDS: f64 = 2.0;
/// "Far" from the current entry: the zoom changed by at least this factor ...
pub const FAR_ZOOM: f64 = 4.0;
/// ... or the centre moved at least this many view widths (of the wider of the two views).
pub const FAR_WIDTHS: f64 = 1.5;
/// "The same place" (consecutive entries merge): zoom within this factor, centre within this
/// many view widths, angle within this many radians, same fractal.
pub const SAME_ZOOM: f64 = 1.25;
pub const SAME_WIDTHS: f64 = 0.1;
pub const SAME_ANGLE: f64 = 0.1;
/// The HUD note after a step ("Back · 3 of 7") shows this long (s).
pub const NOTE_SECONDS: f64 = 2.0;

/// A location: view and fractal.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub view: Viewport,
    pub formula: Formula,
}

impl Entry {
    pub fn new(view: Viewport, formula: Formula) -> Self {
        Self { view, formula }
    }

    fn is(&self, view: &Viewport, formula: Formula) -> bool {
        self.formula == formula && self.view == *view
    }
}

/// A step through the journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Back,
    Forward,
}

/// |ln(a / b)| of two scales.
fn zoom_apart(a: Scale, b: Scale) -> f64 {
    (a.ln() - b.ln()).abs()
}

/// Centre distance in view widths of the wider view (`width`: the view's width in points).
fn widths_apart(a: &Viewport, b: &Viewport, width: f64) -> f64 {
    let unit = a.scale.max(b.scale);
    let w = if width.is_finite() && width > 0.0 { width } else { 1.0 };
    let d = a.center.distance_in(&b.center, unit) / w;
    if d.is_nan() { f64::INFINITY } else { d }
}

/// Far apart: another fractal, the zoom changed by [`FAR_ZOOM`] or more, or the centre moved
/// [`FAR_WIDTHS`] view widths or more. Log-scale and view-size units, so the measure is the
/// same at every depth.
pub fn far(a: &Entry, b: &Entry, width: f64) -> bool {
    a.formula != b.formula
        || zoom_apart(a.view.scale, b.view.scale) >= FAR_ZOOM.ln()
        || widths_apart(&a.view, &b.view, width) >= FAR_WIDTHS
}

/// Near-identical: same fractal, zoom within [`SAME_ZOOM`], centre within [`SAME_WIDTHS`] view
/// widths, angle within [`SAME_ANGLE`].
pub fn same_place(a: &Entry, b: &Entry, width: f64) -> bool {
    let turn = crate::viewport::wrap_angle(a.view.angle - b.view.angle).abs();
    a.formula == b.formula
        && zoom_apart(a.view.scale, b.view.scale) <= SAME_ZOOM.ln()
        && turn <= SAME_ANGLE
        && widths_apart(&a.view, &b.view, width) <= SAME_WIDTHS
}

/// What the navigator tells the journal about a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Nothing moves the view and no hand is on it.
    pub at_rest: bool,
    /// The autopilot (not paused), a journey or a cruise drives the view.
    pub autonomous: bool,
    /// The view's width in points.
    pub width: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Journal {
    entries: Vec<Entry>,
    /// The current entry (meaningless while `entries` is empty).
    at: usize,
    /// The location seen at the last frame.
    last: Option<Entry>,
    /// Since when the view has been at rest (None: moving).
    rest_since: Option<f64>,
    /// This rest's dwell test is done.
    dwell_done: bool,
    /// Travel started since the last frame (its departure is to be recorded).
    departing: bool,
    /// A travel's arrival is to be recorded at the next rest.
    pending: bool,
    /// A journal step is under way: its arrival records nothing.
    stepping: bool,
    autonomous: bool,
    note: Option<(String, f64)>,
    revision: u64,
}

impl Journal {
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The current entry's index and the entry count.
    pub fn position(&self) -> (usize, usize) {
        (self.at, self.entries.len())
    }

    /// Bumped on every change of the entries or the current one (the platform persists then).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The HUD note of the last step while it shows.
    pub fn note(&self, now: f64) -> Option<&str> {
        self.note
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(t, _)| t.as_str())
    }

    /// Travel starts (a jump or a flight): the view it leaves is a departure.
    pub fn travel_started(&mut self) {
        self.departing = true;
    }

    /// One frame: `view`, `formula` are where the view is now (after the frame's motion).
    pub fn observe(&mut self, view: &Viewport, formula: Formula, f: Frame, now: f64) {
        let changed = !self.last.as_ref().is_some_and(|l| l.is(view, formula));
        // Mid-travel views (a flight, a step) are never departures.
        let settled = !self.pending && !self.stepping;
        if std::mem::take(&mut self.departing) {
            if settled && let Some(l) = self.last.clone() {
                self.depart(l, f.width);
            }
            self.stepping = false;
            self.pending = true;
        }
        if f.autonomous != self.autonomous {
            if f.autonomous {
                if settled && let Some(l) = self.last.clone() {
                    self.depart(l, f.width);
                }
            } else {
                self.pending = true;
            }
            self.autonomous = f.autonomous;
        }
        // Leaving a rest that lasted the dwell (frames may have stopped meanwhile).
        if (changed || !f.at_rest) && !self.dwell_done && self.rest_since.is_some_and(|t| now - t >= DWELL_SECONDS) {
            if let Some(l) = self.last.clone() {
                self.dwell(l, f.width);
            }
        }
        if changed {
            self.last = Some(Entry::new(view.clone(), formula));
            self.rest_since = None;
            self.dwell_done = false;
        }
        if !f.at_rest {
            self.rest_since = None;
            self.dwell_done = false;
            return;
        }
        let since = *self.rest_since.get_or_insert(now);
        if self.stepping {
            self.stepping = false;
            self.dwell_done = true;
        } else if self.pending {
            self.pending = false;
            self.dwell_done = true;
            self.record(Entry::new(view.clone(), formula), f.width);
        } else if !self.dwell_done && now - since >= DWELL_SECONDS {
            self.dwell(Entry::new(view.clone(), formula), f.width);
        }
    }

    /// The view at rest now has dwelt long enough (the platform asks before persisting: frames
    /// stop at rest, so the dwell may be due without one).
    pub fn flush(&mut self, now: f64, width: f64) {
        if !self.dwell_done
            && !self.pending
            && !self.stepping
            && self.rest_since.is_some_and(|t| now - t >= DWELL_SECONDS)
            && let Some(l) = self.last.clone()
        {
            self.dwell(l, width);
        }
    }

    fn depart(&mut self, here: Entry, width: f64) {
        if self.entries.is_empty() || far(&here, &self.entries[self.at], width) {
            self.record(here, width);
        }
    }

    fn dwell(&mut self, here: Entry, width: f64) {
        self.dwell_done = true;
        if !self.pending && !self.stepping && (self.entries.is_empty() || far(&here, &self.entries[self.at], width)) {
            self.record(here, width);
        }
    }

    /// A new entry: the forward entries go, the same place merges (the newer replaces).
    fn record(&mut self, e: Entry, width: f64) {
        if !self.entries.is_empty() {
            self.entries.truncate(self.at + 1);
        }
        match self.entries.last_mut() {
            Some(top) if same_place(top, &e, width) => {
                if *top == e {
                    return;
                }
                *top = e;
            }
            _ => self.entries.push(e),
        }
        let over = self.entries.len().saturating_sub(CAPACITY);
        self.entries.drain(..over);
        self.at = self.entries.len() - 1;
        self.revision += 1;
    }

    /// Where a step from `here` goes: back to the current entry when the view has left it (an
    /// unrecorded far move, or a travel that has not arrived), else the previous one; forward to
    /// the next one.
    fn target(&self, step: Step, here: &Entry, width: f64) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        match step {
            Step::Back => {
                let cur = &self.entries[self.at];
                let away = if self.pending {
                    !same_place(here, cur, width)
                } else {
                    far(here, cur, width)
                };
                if away { Some(self.at) } else { self.at.checked_sub(1) }
            }
            Step::Forward => (self.at + 1 < self.entries.len()).then_some(self.at + 1),
        }
    }

    pub fn can_step(&self, step: Step, view: &Viewport, formula: Formula, width: f64) -> bool {
        self.target(step, &Entry::new(view.clone(), formula), width).is_some()
    }

    /// The entry a step would go to (nothing changes).
    pub fn peek(&self, step: Step, view: &Viewport, formula: Formula, width: f64) -> Option<&Entry> {
        let i = self.target(step, &Entry::new(view.clone(), formula), width)?;
        self.entries.get(i)
    }

    /// Takes a step from `view`: the entry to travel to (the caller travels without recording:
    /// the journal ignores the travel this step starts). Back from an unrecorded far view first
    /// records it (so forward returns there).
    pub fn step(&mut self, step: Step, view: &Viewport, formula: Formula, width: f64, now: f64) -> Option<Entry> {
        let here = Entry::new(view.clone(), formula);
        let i = self.target(step, &here, width)?;
        let i = if step == Step::Back && i == self.at && !self.pending {
            // An unrecorded far view: it becomes an entry, the step goes to the one before it.
            self.record(here, width);
            self.at - 1
        } else {
            i
        };
        self.at = i;
        self.pending = false;
        self.departing = false;
        self.stepping = true;
        self.dwell_done = true;
        self.revision += 1;
        let word = if step == Step::Back { "Back" } else { "Forward" };
        self.note = Some((
            format!("{word} · {} of {}", self.at + 1, self.entries.len()),
            now + NOTE_SECONDS,
        ));
        Some(self.entries[i].clone())
    }

    /// The travel a step started (fly or jump) is the step's own: no departure, no arrival.
    pub fn stepped(&mut self) {
        self.departing = false;
        self.pending = false;
        self.stepping = true;
    }

    // MARK: Persistence

    /// The entries and the current one as text (format `almondbread-journal 1`, then `at <i>`,
    /// then one line per entry: `re im scale angle formula [k_re k_im k_bits]`, decimal, the centre to
    /// about 1/1000 of a point, k exactly).
    pub fn export(&self) -> String {
        let mut out = format!("almondbread-journal 1\nat {}\n", self.at);
        for e in &self.entries {
            let v = &e.view;
            let digits = coordinate_digits(v);
            let (re, im) = v.center.to_strings_digits(digits);
            out.push_str(&format!(
                "{re} {im} {} {:?} {}",
                v.scale.to_text(17),
                v.angle,
                e.formula.name()
            ));
            if let Some(k) = e.formula.julia_k() {
                let kv = k.value();
                // k exactly (it names the set: a rounded k would be another one).
                // Round-trip digits at its precision, then the precision: parsed back at
                // it, the value is bit for bit k's.
                out.push_str(&format!(
                    " {} {} {}",
                    kv.re.to_decimal_string(None),
                    kv.im.to_decimal_string(None),
                    kv.precision_bits()
                ));
            }
            out.push('\n');
        }
        out
    }

    /// Restores exported entries before the ones recorded since launch (those stay newest);
    /// malformed lines are skipped. Returns the entries restored.
    pub fn import(&mut self, text: &str, width: f64) -> usize {
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some("almondbread-journal 1") {
            return 0;
        }
        let mut at = None;
        let mut loaded = Vec::new();
        for line in lines {
            let p: Vec<&str> = line.split_whitespace().collect();
            if p.len() == 2 && p[0] == "at" {
                at = p[1].parse::<usize>().ok();
                continue;
            }
            if let Some(e) = parse_entry(&p) {
                loaded.push(e);
            }
        }
        let n = loaded.len();
        if n == 0 {
            return 0;
        }
        let session = std::mem::take(&mut self.entries);
        let fresh = session.is_empty();
        self.entries = loaded;
        self.at = at.unwrap_or(n - 1).min(n - 1);
        if fresh {
            // Nothing recorded yet: the restored position stands (forward entries kept).
            self.revision += 1;
        } else {
            self.at = self.entries.len() - 1;
            for e in session {
                self.record(e, width);
            }
        }
        let over = self.entries.len().saturating_sub(CAPACITY);
        if over > 0 {
            self.entries.drain(..over);
            self.at = self.at.saturating_sub(over);
        }
        self.revision += 1;
        n
    }
}

/// Significant digits that place the centre of `v` to about 1/1000 of a point (as the
/// clipboard's coordinates).
fn coordinate_digits(v: &Viewport) -> usize {
    let mag = v
        .center
        .re()
        .abs()
        .max(v.center.im().abs())
        .max(1e-300)
        .log10()
        .ceil()
        .max(0.0);
    ((mag - v.scale.log10()).ceil().max(0.0) as usize + 4).max(17)
}

fn parse_entry(p: &[&str]) -> Option<Entry> {
    if p.len() != 5 && p.len() != 8 {
        return None;
    }
    let center = Center::parse(p[0], p[1]).filter(|c| c.re().is_finite() && c.im().is_finite())?;
    let scale = Scale::parse(p[2])?;
    let angle: f64 = p[3].parse().ok().filter(|a: &f64| a.is_finite())?;
    let formula = if p[4] == "julia" {
        let bits: usize = p
            .get(7)?
            .parse()
            .ok()
            .filter(|b| (64..=crate::viewport::MAX_CENTER_BITS).contains(b))?;
        let k = crate::math::BigComplex::parse(p.get(5)?, p.get(6)?, bits)?;
        let f = k.to_f64();
        if !(f[0].is_finite() && f[1].is_finite()) {
            return None;
        }
        Formula::Julia(JuliaK::new(&k))
    } else {
        Formula::parse(p[4])?
    };
    let mut view = Viewport::with_center(center, scale).with_angle(angle);
    view.ensure_precision();
    Some(Entry::new(view, formula))
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f64 = 800.0;
    const REST: Frame = Frame {
        at_rest: true,
        autonomous: false,
        width: W,
    };
    const MOVING: Frame = Frame {
        at_rest: false,
        autonomous: false,
        width: W,
    };

    fn v(re: f64, im: f64, scale: f64) -> Viewport {
        Viewport::new(re, im, scale)
    }

    fn m() -> Formula {
        Formula::Mandelbrot
    }

    /// Settles the journal at `view` (a few frames at rest).
    fn rest(j: &mut Journal, view: &Viewport, t: f64) {
        j.observe(view, m(), REST, t);
        j.observe(view, m(), REST, t + 0.1);
    }

    fn jump(j: &mut Journal, from_t: f64, to: &Viewport) {
        j.travel_started();
        rest(j, to, from_t);
    }

    fn scales(j: &Journal) -> Vec<f64> {
        j.entries().iter().map(|e| e.view.scale.to_f64()).collect()
    }

    #[test]
    fn a_jump_records_where_it_left_and_where_it_arrived() {
        let mut j = Journal::default();
        let home = v(-0.5, 0.0, 0.004);
        rest(&mut j, &home, 0.0);
        assert!(j.entries().is_empty(), "a short rest records nothing");
        jump(&mut j, 1.0, &v(-0.75, 0.1, 1e-6));
        assert_eq!(scales(&j), vec![0.004, 1e-6]);
        assert_eq!(j.position(), (1, 2));
    }

    #[test]
    fn a_flight_records_on_arrival_only() {
        let mut j = Journal::default();
        let home = v(-0.5, 0.0, 0.004);
        rest(&mut j, &home, 0.0);
        j.travel_started();
        for i in 0..10 {
            let s = 0.004 * 0.5f64.powi(i);
            j.observe(&v(-0.6, 0.0, s), m(), MOVING, 1.0 + i as f64 * 0.1);
        }
        assert_eq!(scales(&j), vec![0.004], "mid-flight views are not entries");
        let target = v(-0.6, 0.0, 1e-8);
        j.observe(&target, m(), REST, 3.0);
        assert_eq!(scales(&j), vec![0.004, 1e-8]);
    }

    #[test]
    fn manual_moves_record_after_the_dwell_when_far() {
        let mut j = Journal::default();
        let home = v(-0.5, 0.0, 0.004);
        rest(&mut j, &home, 0.0);
        j.observe(&home, m(), REST, 2.5);
        assert_eq!(j.entries().len(), 1, "the start view after its dwell");
        // A small zoom (×2) and a short pan: not far.
        let near = v(-0.5 + 0.004 * 400.0, 0.0, 0.002);
        j.observe(&near, m(), MOVING, 3.0);
        rest(&mut j, &near, 3.1);
        j.observe(&near, m(), REST, 10.0);
        assert_eq!(j.entries().len(), 1, "a near view is no entry");
        // ×8: far, but only after 2 s at rest.
        let deep = v(-0.5, 0.0, 0.0005);
        j.observe(&deep, m(), MOVING, 11.0);
        j.observe(&deep, m(), REST, 11.1);
        j.observe(&deep, m(), REST, 12.0);
        assert_eq!(j.entries().len(), 1, "before the dwell");
        j.observe(&deep, m(), REST, 13.2);
        assert_eq!(scales(&j), vec![0.004, 0.0005]);
    }

    #[test]
    fn a_pan_far_away_counts_in_view_widths() {
        let mut j = Journal::default();
        let a = v(0.0, 0.0, 1e-10);
        rest(&mut j, &a, 0.0);
        j.observe(&a, m(), REST, 3.0);
        // 1.4 view widths: not far; 1.6: far (any depth).
        let b = v(1.4 * W * 1e-10, 0.0, 1e-10);
        j.observe(&b, m(), REST, 4.0);
        j.observe(&b, m(), REST, 7.0);
        assert_eq!(j.entries().len(), 1);
        let c = v(1.6 * W * 1e-10, 0.0, 1e-10);
        j.observe(&c, m(), REST, 8.0);
        j.observe(&c, m(), REST, 11.0);
        assert_eq!(j.entries().len(), 2);
    }

    #[test]
    fn the_hand_on_a_paused_scroll_is_not_rest() {
        let mut j = Journal::default();
        let a = v(-0.5, 0.0, 0.004);
        rest(&mut j, &a, 0.0);
        j.observe(&a, m(), REST, 3.0);
        let b = v(-0.5, 0.0, 1e-4);
        // Fingers rest on the trackpad for 5 s mid-gesture: the navigator says not at rest.
        for i in 0..50 {
            j.observe(&b, m(), MOVING, 4.0 + i as f64 * 0.1);
        }
        assert_eq!(j.entries().len(), 1);
        // Then the gesture goes on elsewhere: the paused view never was an entry.
        let c = v(-0.5, 0.0, 1e-3);
        j.observe(&c, m(), MOVING, 9.5);
        j.observe(&c, m(), REST, 9.6);
        j.observe(&c, m(), REST, 10.0);
        assert_eq!(j.entries().len(), 1);
    }

    #[test]
    fn a_rest_without_frames_is_recorded_when_the_view_moves_on_or_on_flush() {
        let mut j = Journal::default();
        let a = v(-0.5, 0.0, 0.004);
        rest(&mut j, &a, 0.0);
        j.observe(&a, m(), REST, 3.0);
        let b = v(-0.5, 0.0, 1e-5);
        j.observe(&b, m(), REST, 4.0);
        // No frames for 10 s, then a new pan.
        j.observe(&v(-0.5, 1e-5, 1e-5), m(), MOVING, 14.0);
        assert_eq!(scales(&j), vec![0.004, 1e-5]);
        let c = v(-0.5, 0.0, 1e-9);
        j.observe(&c, m(), REST, 15.0);
        j.flush(15.5, W);
        assert_eq!(j.entries().len(), 2, "not yet");
        j.flush(17.5, W);
        assert_eq!(j.entries().len(), 3);
    }

    #[test]
    fn back_and_forward_walk_the_entries_and_record_nothing() {
        let mut j = Journal::default();
        let a = v(-0.5, 0.0, 0.004);
        let b = v(-0.75, 0.1, 1e-6);
        let c = v(-1.75, 0.0, 1e-9);
        rest(&mut j, &a, 0.0);
        jump(&mut j, 1.0, &b);
        jump(&mut j, 2.0, &c);
        assert_eq!(j.position(), (2, 3));
        let e = j.step(Step::Back, &c, m(), W, 3.0).unwrap();
        assert_eq!(e.view, b);
        assert_eq!(j.note(3.5), Some("Back · 2 of 3"));
        assert_eq!(j.note(5.1), None, "the note times out");
        // The step's own travel and arrival record nothing.
        j.travel_started();
        j.stepped();
        rest(&mut j, &b, 3.1);
        let e = j.step(Step::Back, &b, m(), W, 4.0).unwrap();
        assert_eq!(e.view, a);
        j.stepped();
        rest(&mut j, &a, 4.1);
        assert!(!j.can_step(Step::Back, &a, m(), W));
        assert_eq!(j.entries().len(), 3);
        let e = j.step(Step::Forward, &a, m(), W, 5.0).unwrap();
        assert_eq!(e.view, b);
        j.stepped();
        rest(&mut j, &b, 5.1);
        assert_eq!(j.position(), (1, 3));
        assert!(j.can_step(Step::Forward, &b, m(), W));
    }

    #[test]
    fn new_travel_after_going_back_drops_the_forward_entries() {
        let mut j = Journal::default();
        let a = v(-0.5, 0.0, 0.004);
        let b = v(-0.75, 0.1, 1e-6);
        let c = v(-1.75, 0.0, 1e-9);
        rest(&mut j, &a, 0.0);
        jump(&mut j, 1.0, &b);
        jump(&mut j, 2.0, &c);
        j.step(Step::Back, &c, m(), W, 3.0);
        j.stepped();
        rest(&mut j, &b, 3.1);
        let d = v(0.25, 0.0, 1e-7);
        jump(&mut j, 4.0, &d);
        assert_eq!(scales(&j), vec![0.004, 1e-6, 1e-7]);
        assert!(!j.can_step(Step::Forward, &d, m(), W));
    }

    #[test]
    fn back_from_an_unrecorded_far_view_returns_to_the_current_entry_and_forward_comes_back() {
        let mut j = Journal::default();
        let a = v(-0.5, 0.0, 0.004);
        let b = v(-0.75, 0.1, 1e-6);
        rest(&mut j, &a, 0.0);
        jump(&mut j, 1.0, &b);
        // A quick manual dive (no dwell yet), then ⌘Z.
        let deep = v(-0.75, 0.1, 1e-9);
        j.observe(&deep, m(), REST, 5.0);
        let e = j.step(Step::Back, &deep, m(), W, 5.5).unwrap();
        assert_eq!(e.view, b, "back to where the dive began");
        j.stepped();
        rest(&mut j, &b, 5.6);
        let e = j.step(Step::Forward, &b, m(), W, 6.0).unwrap();
        assert_eq!(e.view, deep);
    }

    #[test]
    fn back_mid_flight_returns_to_the_departure() {
        let mut j = Journal::default();
        let a = v(-0.5, 0.0, 0.004);
        rest(&mut j, &a, 0.0);
        j.travel_started();
        let mid = v(-0.5, 0.0, 0.003);
        j.observe(&mid, m(), MOVING, 1.0);
        let e = j.step(Step::Back, &mid, m(), W, 1.1).unwrap();
        assert_eq!(e.view, a);
        assert_eq!(j.entries().len(), 1);
    }

    #[test]
    fn the_same_place_merges() {
        let mut j = Journal::default();
        let a = v(-0.5, 0.0, 0.004);
        let b = v(-0.75, 0.1, 1e-6);
        rest(&mut j, &a, 0.0);
        jump(&mut j, 1.0, &b);
        // A tour step or place at practically the same view: the newer replaces.
        let b2 = v(-0.75 + 1e-6 * 10.0, 0.1, 1.1e-6);
        jump(&mut j, 2.0, &b2);
        assert_eq!(j.entries().len(), 2);
        assert_eq!(j.entries()[1].view, b2);
        // ×2 is not the same place (explicit travel records it).
        let b3 = v(-0.75, 0.1, 0.5e-6);
        jump(&mut j, 3.0, &b3);
        assert_eq!(j.entries().len(), 3);
    }

    #[test]
    fn autonomous_motion_records_its_start_and_its_stop() {
        let mut j = Journal::default();
        let a = v(-0.5, 0.0, 0.004);
        rest(&mut j, &a, 0.0);
        let auto = Frame {
            at_rest: false,
            autonomous: true,
            width: W,
        };
        for i in 0..30 {
            j.observe(&v(-0.5, 0.0, 0.004 * 0.7f64.powi(i)), m(), auto, 1.0 + i as f64 * 0.1);
        }
        assert_eq!(j.entries().len(), 1, "the start view");
        let stop = v(-0.5, 0.0, 1e-7);
        j.observe(&stop, m(), MOVING, 5.0);
        j.observe(&stop, m(), REST, 5.5);
        assert_eq!(scales(&j), vec![0.004, 1e-7]);
    }

    #[test]
    fn formulas_and_julia_sets_are_part_of_the_location() {
        let mut j = Journal::default();
        let a = v(-0.5, 0.0, 0.004);
        rest(&mut j, &a, 0.0);
        let k = Formula::Julia(JuliaK::new(Center::new(-0.123, 0.745).big()));
        let julia = v(0.0, 0.0, 0.004);
        j.travel_started();
        j.observe(&julia, k, REST, 1.0);
        assert_eq!(j.entries().len(), 2);
        assert_eq!(j.entries()[1].formula, k);
        // Same view, another fractal: far.
        assert!(far(
            &Entry::new(a.clone(), m()),
            &Entry::new(a.clone(), Formula::Mandelbar),
            W
        ));
        let e = j.step(Step::Back, &julia, k, W, 2.0).unwrap();
        assert_eq!(e.formula, m());
    }

    #[test]
    fn the_journal_keeps_the_last_hundred() {
        let mut j = Journal::default();
        rest(&mut j, &v(0.0, 0.0, 1.0), 0.0);
        for i in 0..150 {
            jump(&mut j, 1.0 + i as f64, &v(0.0, 0.0, 0.5f64.powi(3 * (i + 1))));
        }
        assert_eq!(j.entries().len(), CAPACITY);
        assert_eq!(j.position(), (CAPACITY - 1, CAPACITY));
    }

    #[test]
    fn export_and_import_round_trip() {
        let mut j = Journal::default();
        let a = v(-0.5, 0.0, 0.004);
        let deep = Viewport::with_center(
            Center::parse(
                "-1.4303584327259864377084589925832673307971990283708853",
                "-1.3419188621437467531262066807358441689824585030600614e-11",
            )
            .unwrap(),
            Scale::parse("4.21597e-48").unwrap(),
        )
        .with_angle(0.25);
        let k = Formula::Julia(JuliaK::new(Center::new(-0.123, 0.745).big()));
        rest(&mut j, &a, 0.0);
        jump(&mut j, 1.0, &deep);
        j.travel_started();
        j.observe(&v(0.0, 0.0, 0.004), k, REST, 2.0);
        j.step(Step::Back, &v(0.0, 0.0, 0.004), k, W, 3.0);
        let text = j.export();
        let mut r = Journal::default();
        assert_eq!(r.import(&text, W), 3);
        assert_eq!(r.position(), (1, 3), "the position survives");
        for (x, y) in r.entries().iter().zip(j.entries()) {
            assert_eq!(x.formula, y.formula);
            assert!(same_place(x, y, W), "{x:?} vs {y:?}");
            assert!((x.view.scale.log10() - y.view.scale.log10()).abs() < 1e-12);
            assert!(x.view.center.distance_in(&y.view.center, y.view.scale) < 1e-2);
        }
        // Entries recorded before the import stay newest.
        let mut s = Journal::default();
        rest(&mut s, &v(0.3, 0.0, 0.004), 0.0);
        s.observe(&v(0.3, 0.0, 0.004), m(), REST, 3.0);
        assert_eq!(s.import(&text, W), 3);
        assert_eq!(s.position(), (3, 4));
        assert_eq!(s.import("garbage", W), 0);
    }
}

/// The journal fed by the navigator (`advance`, `jumped`, `fly_to`, `travel_to`, `journal_go`).
#[cfg(test)]
mod navigator_tests {
    use super::*;
    use crate::input::{InputEvent, NavKey, Navigator, TRAVEL_FLY_MAX_SECONDS};

    fn nav() -> (Navigator, Viewport) {
        let mut n = Navigator::default();
        let mut v = Viewport::fitting(800.0, 500.0);
        n.handle(
            InputEvent::Resize {
                width: 800.0,
                height: 500.0,
            },
            &mut v,
            0.0,
        );
        (n, v)
    }

    /// Frames at 60 Hz until nothing moves (at most `limit` s of model time).
    fn run(n: &mut Navigator, v: &mut Viewport, t: &mut f64, limit: f64) {
        let end = *t + limit;
        loop {
            *t += 1.0 / 60.0;
            n.advance(v, *t);
            if !n.is_moving() || *t > end {
                break;
            }
        }
        *t += 1.0 / 60.0;
        n.advance(v, *t);
    }

    #[test]
    fn travel_to_flies_short_ways_and_jumps_long_ones() {
        let (mut n, v) = nav();
        let near = Viewport::new(-0.75, 0.1, v.scale.to_f64() / 20.0);
        assert!(n.travel_to(&near, &v, 0.0), "×20: a flight");
        assert!(n.flight().is_some());
        let (mut n, v) = nav();
        let deep = Viewport::new(-0.75, 0.1, 1e-30);
        assert!(!n.travel_to(&deep, &v, 0.0), "1e-30: a jump");
        assert!(n.flight().is_none());
        // A flight that travel_to starts lands within its limit.
        let (mut n, mut v) = nav();
        let mut t = 0.0;
        assert!(n.travel_to(&near, &v.clone(), t));
        run(&mut n, &mut v, &mut t, 30.0);
        assert_eq!(v, near);
        assert!(t <= TRAVEL_FLY_MAX_SECONDS + 0.5, "{t} s");
    }

    #[test]
    fn a_fly_to_records_departure_and_landing_and_back_flies_home() {
        let (mut n, mut v) = nav();
        let home = v.clone();
        let mut t = 0.0;
        run(&mut n, &mut v, &mut t, 1.0);
        let target = Viewport::new(-0.7436, 0.1318, home.scale.to_f64() / 300.0);
        n.fly_to(&target, &v.clone(), t);
        run(&mut n, &mut v, &mut t, 30.0);
        assert_eq!(v, target);
        assert_eq!(n.journal().position(), (1, 2));
        let mut f = Formula::Mandelbrot;
        assert_eq!(n.journal_go(Step::Back, &mut v, &mut f, t), Some(false));
        run(&mut n, &mut v, &mut t, 30.0);
        assert_eq!(v, home);
        assert_eq!(n.journal().position(), (0, 2), "back recorded nothing");
        assert_eq!(n.journal().entries().len(), 2);
    }

    #[test]
    fn autopilot_stop_and_reset_record() {
        let (mut n, mut v) = nav();
        let mut t = 0.0;
        v = Viewport::new(-0.7436, 0.1318, 1e-7);
        n.jumped();
        run(&mut n, &mut v, &mut t, 1.0);
        assert_eq!(n.journal().position(), (0, 1));
        // Z: the autopilot from here; Esc: it stops; where it stops is an entry.
        n.handle(InputEvent::KeyDown(NavKey::Autopilot), &mut v, t);
        n.handle(InputEvent::KeyUp(NavKey::Autopilot), &mut v, t);
        for _ in 0..240 {
            t += 1.0 / 60.0;
            n.advance(&mut v, t);
        }
        n.handle(InputEvent::KeyDown(NavKey::Stop), &mut v, t);
        n.handle(InputEvent::KeyUp(NavKey::Stop), &mut v, t);
        run(&mut n, &mut v, &mut t, 30.0);
        assert_eq!(n.journal().entries().len(), 2, "{:?}", n.journal().position());
        assert_eq!(n.journal().entries()[1].view, v);
        // R: home is an entry.
        n.handle(InputEvent::KeyDown(NavKey::Reset), &mut v, t);
        run(&mut n, &mut v, &mut t, 1.0);
        assert_eq!(n.journal().entries().len(), 3);
    }

    #[test]
    fn a_step_into_another_fractal_jumps_and_sets_it() {
        let (mut n, mut v) = nav();
        let mut t = 0.0;
        run(&mut n, &mut v, &mut t, 2.5);
        let k = Formula::Julia(JuliaK::new(Center::new(-0.8, 0.156).big()));
        n.jumped();
        n.set_formula(k);
        v = Viewport::new(0.0, 0.0, 0.005);
        run(&mut n, &mut v, &mut t, 1.0);
        assert_eq!(n.journal().position(), (1, 2));
        let mut f = k;
        assert_eq!(n.journal_go(Step::Back, &mut v, &mut f, t), Some(true));
        assert_eq!(f, Formula::Mandelbrot);
        assert!(n.flight().is_none(), "a jump");
        run(&mut n, &mut v, &mut t, 1.0);
        assert_eq!(n.journal().position(), (0, 2));
    }
}
