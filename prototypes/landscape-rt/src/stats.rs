//! Frame-time statistics and frame-difference metrics.

use crate::sim::Sim;

pub fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let i = ((s.len() - 1) as f64 * p).round() as usize;
    s[i]
}

fn line(name: &str, v: &[f64]) -> String {
    format!(
        "{name:<22} p50 {:6.2}  p95 {:6.2}  p99 {:6.2}  max {:6.2}  (n={})",
        pct(v, 0.5),
        pct(v, 0.95),
        pct(v, 0.99),
        pct(v, 1.0),
        v.len()
    )
}

/// Prints frame pacing for a window run: frame-start intervals (CPU), presentation intervals
/// (from `presentedTime`), dropped refreshes, GPU times and CPU encode times. The first 30
/// frames (start-up) are skipped.
pub fn report_frames(starts: &[f64], presented: &[(u64, f64)], sim: &Sim, refresh: f64) {
    let skip = 30;
    let iv: Vec<f64> = starts.windows(2).skip(skip).map(|w| (w[1] - w[0]) * 1e3).collect();
    let mut pres = presented.to_vec();
    pres.sort_by_key(|p| p.0);
    let piv: Vec<f64> = pres.windows(2).skip(skip).map(|w| (w[1].1 - w[0].1) * 1e3).collect();
    let dropped: f64 = piv.iter().map(|&x| ((x / (refresh * 1e3)).round() - 1.0).max(0.0)).sum();
    let late = piv.iter().filter(|&&x| x > refresh * 1e3 * 1.5).count();
    let comp: Vec<f64> = sim.gpu_ms.iter().skip(skip).map(|g| g.1).filter(|&x| x > 0.0).collect();
    let rend: Vec<f64> = sim.gpu_ms.iter().skip(skip).map(|g| g.2).collect();
    let cpu: Vec<f64> = sim.recs.iter().skip(skip).map(|r| r.cpu_ms).collect();
    let tiles: Vec<f64> = sim.recs.iter().skip(skip).map(|r| r.tiles as f64).collect();
    let total_t = pres.last().map(|l| l.1).unwrap_or(0.0) - pres.get(skip).map(|f| f.1).unwrap_or(0.0);
    println!("{}", line("frame interval ms", &iv));
    println!("{}", line("presented interval ms", &piv));
    println!(
        "presented {} frames in {:.2} s = {:.1} fps; refreshes missed {} (intervals > 1.5 refresh: {})",
        piv.len(),
        total_t,
        piv.len() as f64 / total_t.max(1e-9),
        dropped,
        late
    );
    println!("{}", line("GPU DE tiles ms", &comp));
    println!("{}", line("GPU render ms", &rend));
    println!("{}", line("CPU frame ms", &cpu));
    println!(
        "tiles/frame mean {:.0}, ms/tile {:.4}, missing at end {}, S end {:.3e}, blocks drawn (last) {}",
        tiles.iter().sum::<f64>() / tiles.len().max(1) as f64,
        sim.ms_per_tile,
        sim.clip.missing,
        sim.flight.s,
        sim.last_blocks
    );
}

/// Temporal stability of a frame sequence (luma, any size): first differences (motion) and
/// second differences (acceleration of pixel values). Smooth motion has small second
/// differences; popping and shimmer show as spikes.
#[derive(Default)]
pub struct Stability {
    prev: Option<Vec<f32>>,
    prev2: Option<Vec<f32>>,
    pub d1: Vec<f64>,
    pub d2: Vec<f64>,
    /// fraction of pixels whose |second difference| > 24/255
    pub pop: Vec<f64>,
}

impl Stability {
    pub fn push(&mut self, luma: Vec<f32>) {
        if let Some(p) = &self.prev {
            let d1 = luma.iter().zip(p).map(|(a, b)| (a - b).abs() as f64).sum::<f64>() / luma.len() as f64;
            self.d1.push(d1);
            if let Some(q) = &self.prev2 {
                let mut s = 0.0;
                let mut pops = 0usize;
                for i in 0..luma.len() {
                    let d = (luma[i] - 2.0 * p[i] + q[i]).abs();
                    s += d as f64;
                    if d > 24.0 / 255.0 {
                        pops += 1;
                    }
                }
                self.d2.push(s / luma.len() as f64);
                self.pop.push(pops as f64 / luma.len() as f64);
            }
        }
        self.prev2 = self.prev.take();
        self.prev = Some(luma);
    }

    pub fn report(&self, label: &str) {
        let k = 255.0;
        let d1: Vec<f64> = self.d1.iter().map(|x| x * k).collect();
        let d2: Vec<f64> = self.d2.iter().map(|x| x * k).collect();
        let pop: Vec<f64> = self.pop.iter().map(|x| x * 100.0).collect();
        // spikes: second difference > 2.5× its local median (±15 frames)
        let mut spikes = Vec::new();
        for i in 0..d2.len() {
            let lo = i.saturating_sub(15);
            let hi = (i + 16).min(d2.len());
            let med = pct(&d2[lo..hi], 0.5);
            if d2[i] > 2.5 * med && d2[i] > 0.3 {
                spikes.push((i + 2, d2[i], med));
            }
        }
        println!("[{label}] frames {}", d1.len() + 1);
        println!("  |f(t)-f(t-1)|        mean {:.3}  p50 {:.3}  p99 {:.3}  (0..255 luma)", mean(&d1), pct(&d1, 0.5), pct(&d1, 0.99));
        println!("  |f(t)-2f(t-1)+f(t-2)| mean {:.3}  p50 {:.3}  p99 {:.3}  max {:.3}", mean(&d2), pct(&d2, 0.5), pct(&d2, 0.99), pct(&d2, 1.0));
        println!("  ratio d2/d1 (jitter per unit motion) {:.3}", mean(&d2) / mean(&d1).max(1e-9));
        println!("  popping pixels (|d2| > 24) mean {:.4} %  p99 {:.4} %  max {:.4} %", mean(&pop), pct(&pop, 0.99), pct(&pop, 1.0));
        println!("  spike frames (d2 > 2.5x local median): {}", spikes.len());
        for (f, v, m) in spikes.iter().take(12) {
            println!("    frame {f}: d2 {v:.3} (local median {m:.3})");
        }
    }
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len().max(1) as f64
}
