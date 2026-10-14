// GMP comparison benchmark for the reference orbit (test tooling only; GMP is LGPL and never
// linked into the app, kb: decisions/arbitrary-precision).
//
// Build: clang -O3 -I/opt/homebrew/include gmp_orbit_bench.c -L/opt/homebrew/lib -lgmp -o gmp_orbit_bench
// Run:   ./gmp_orbit_bench            (prints bits, mpf mul-form it/s, mpf sqr-form it/s, mpn floor it/s)
//
// mpf: the same iteration as AlmondbreadMath's ReferenceOrbitComputer (multiplication and
// squaring forms), with mpf_get_d of both parts per iteration for the escape test.
// mpn floor: 3 mpn_sqr + 6 mpn_add_n per iteration on n-limb operands, i.e. the raw cost of
// a fixed-point orbit with no rounding, signs or exponent handling (a lower bound, not a
// correct orbit).
#include <gmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

static double now(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec + ts.tv_nsec * 1e-9;
}

static double bench_mpf(int bits, int squarings) {
    mpf_set_default_prec(bits);
    mpf_t x, y, cx, cy, s, d, p1, p2, x2, y2;
    mpf_inits(x, y, cx, cy, s, d, p1, p2, x2, y2, NULL);
    mpf_set_str(cx, "-0.50000000000000000000123456789012345678901234567890123456789", 10);
    mpf_set_str(cy, "0.500000000000000000000987654321098765432109876543210987654321", 10);
    // Fill the low bits.
    mpf_t tiny;
    mpf_init(tiny);
    mpf_set_d(tiny, 0.123456789);
    mpf_div_2exp(tiny, tiny, bits - 70);
    mpf_add(cx, cx, tiny);
    long iters = 1000;
    double t = 0;
    for (;;) {
        mpf_set_ui(x, 0);
        mpf_set_ui(y, 0);
        double t0 = now();
        double acc = 0;
        for (long i = 0; i < iters; i++) {
            if (squarings) {
                mpf_mul(x2, x, x);
                mpf_mul(y2, y, y);
                mpf_add(s, x, y);
                mpf_mul(p1, s, s);
                mpf_add(p2, x2, y2);
                mpf_sub(d, x2, y2);
                mpf_add(x, d, cx);
                mpf_sub(d, p1, p2);
                mpf_add(y, d, cy);
            } else {
                mpf_add(s, x, y);
                mpf_sub(d, x, y);
                mpf_mul(p1, s, d);
                mpf_mul(p2, x, y);
                mpf_mul_2exp(p2, p2, 1);
                mpf_add(x, p1, cx);
                mpf_add(y, p2, cy);
            }
            double xd = mpf_get_d(x), yd = mpf_get_d(y);
            acc += xd * xd + yd * yd;
        }
        t = now() - t0;
        if (acc < 0) printf("?");
        if (t > 0.4) break;
        iters *= 4;
    }
    mpf_clears(x, y, cx, cy, s, d, p1, p2, x2, y2, tiny, NULL);
    return iters / t;
}

static double bench_mpn(int bits) {
    int n = (bits + 63) / 64;
    mp_limb_t *a = malloc(8 * n), *b = malloc(8 * n), *c = malloc(8 * n);
    mp_limb_t *r1 = malloc(16 * n), *r2 = malloc(16 * n), *r3 = malloc(16 * n), *t = malloc(16 * n);
    for (int i = 0; i < n; i++) { a[i] = 0x9e3779b97f4a7c15ULL * (i + 1); b[i] = a[i] ^ 0x5555; c[i] = a[i] + 7; }
    long iters = 1000;
    double tt;
    for (;;) {
        double t0 = now();
        for (long i = 0; i < iters; i++) {
            mpn_sqr(r1, a, n);
            mpn_sqr(r2, b, n);
            mpn_add_n(t, a, b, n);
            mpn_sqr(r3, t, n);
            mpn_add_n(t, r1 + n, r2 + n, n);
            mpn_sub_n(a, r1 + n, r2 + n, n);
            mpn_add_n(a, a, c, n);
            mpn_sub_n(b, r3 + n, t, n);
            mpn_add_n(b, b, c, n);
            mpn_add_n(t, t, c, n);
        }
        tt = now() - t0;
        if (tt > 0.4) break;
        iters *= 4;
    }
    free(a); free(b); free(c); free(r1); free(r2); free(r3); free(t);
    return iters / tt;
}

int main(int argc, char **argv) {
    int sizes[] = {64, 128, 256, 512, 1024, 2048, 4096, 16384};
    printf("bits\tmpf_mul_form\tmpf_sqr_form\tmpn_floor\n");
    for (unsigned k = 0; k < sizeof sizes / sizeof sizes[0]; k++) {
        int bits = sizes[k];
        printf("%d\t%.0f\t%.0f\t%.0f\n", bits, bench_mpf(bits, 0), bench_mpf(bits, 1), bench_mpn(bits));
        fflush(stdout);
    }
    return 0;
}
