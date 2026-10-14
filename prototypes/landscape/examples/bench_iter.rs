#[path = "../src/mandel.rs"]
mod mandel;
fn main() {
    // exterior point near the seahorse minibrot (slow escape) and an interior one
    let pts = [(-0.74364391185274215, 0.13182589715597984), (-0.74364390138939729, 0.13182587743631066), (-0.75, 0.001)];
    for (cr, ci) in pts {
        let t = std::time::Instant::now();
        let mut it = 0u64;
        let mut s = mandel::eval(cr, ci, 50_000);
        for k in 0..200 {
            s = mandel::eval(cr + k as f64 * 1e-17, ci, 50_000);
            it += s.iters as u64;
        }
        let ns = t.elapsed().as_nanos() as f64;
        let ps: Vec<(f64, f64)> = (0..200).map(|k| (cr + k as f64 * 1e-17, ci)).collect();
        let mut out = vec![mandel::Sample::default(); 200];
        let t2 = std::time::Instant::now();
        mandel::eval_many(&ps, &mut out, 50_000);
        let ns2 = t2.elapsed().as_nanos() as f64;
        assert!(out.iter().zip(&ps).all(|(o, p)| { let e = mandel::eval(p.0, p.1, 50_000); e.interior == o.interior && (e.interior || (e.iters == o.iters && (e.de - o.de).abs() <= 1e-12 * e.de)) }));
        println!("  many: {:.2} ns/iter", ns2 / it as f64);
        println!("{cr},{ci}: iters {} interior {} mu {:.1}  {:.2} ns/iter", s.iters, s.interior, s.mu, ns / it as f64);
    }
}
#[allow(dead_code)]
fn _x() {}
