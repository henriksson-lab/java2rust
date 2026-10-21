package org.apache.commons.rng.sampling.distribution;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;
import org.apache.commons.rng.UniformRandomProvider;

/** Independent caller: all distribution algorithms are untouched upstream code. */
public final class SamplingDistributionsOracle {
    // This test-only entropy source is deliberately independent of translated
    // RNG implementations. Only calls at the UniformRandomProvider boundary
    // are logged; no distribution/rejection algorithm is reproduced here.
    static final class TraceRng implements UniformRandomProvider {
        long state = 0x123456789abcdef0L;
        int calls;
        final List<String> trace = new ArrayList<>();
        long word() {
            if (++calls > 100000) throw new AssertionError("draw budget exceeded");
            state += 0x9e3779b97f4a7c15L;
            long z = state;
            z = (z ^ (z >>> 30)) * 0xbf58476d1ce4e5b9L;
            z = (z ^ (z >>> 27)) * 0x94d049bb133111ebL;
            return z ^ (z >>> 31);
        }
        public double nextDouble() {
            double value = (word() >>> 11) * 0x1.0p-53;
            trace.add("d:" + Long.toUnsignedString(Double.doubleToRawLongBits(value)));
            return value;
        }
        public int nextInt() {
            int value = (int) word();
            trace.add("i:" + value);
            return value;
        }
        public int nextInt(int bound) {
            if (bound <= 0) throw new IllegalArgumentException("bound");
            int value = (int) Long.remainderUnsigned(word(), bound);
            trace.add("b:" + bound + ":" + value);
            return value;
        }
        public long nextLong() { throw new AssertionError("unexpected nextLong"); }
        public long nextLong(long bound) { throw new AssertionError("unexpected boundedLong"); }
        public boolean nextBoolean() { throw new AssertionError("unexpected boolean"); }
        public float nextFloat() { throw new AssertionError("unexpected float"); }
        public void nextBytes(byte[] bytes) { throw new AssertionError("unexpected bytes"); }
        public void nextBytes(byte[] bytes, int start, int length) { throw new AssertionError("unexpected bytes"); }
        public String toString() { return "trace"; }
        String drain() {
            String result = trace.toString();
            trace.clear();
            return result;
        }
    }
    static void continuous(String label, Function<TraceRng, ContinuousSampler> make) {
        TraceRng rng = new TraceRng();
        ContinuousSampler sampler = make.apply(rng);
        System.out.println(label + ".constructor.trace=" + rng.drain());
        for (int i = 0; i < 24; ++i) {
            System.out.println(label + ".double." + i + "=" + sampler.sample());
            System.out.println(label + ".trace." + i + "=" + rng.drain());
        }
        display(label, sampler.toString());
    }
    static void discrete(String label, Function<TraceRng, DiscreteSampler> make) {
        TraceRng rng = new TraceRng();
        DiscreteSampler sampler = make.apply(rng);
        System.out.println(label + ".constructor.trace=" + rng.drain());
        for (int i = 0; i < 24; ++i) {
            System.out.println(label + ".int." + i + "=" + sampler.sample());
            System.out.println(label + ".trace." + i + "=" + rng.drain());
        }
        display(label, sampler.toString());
    }
    static void display(String label, String text) {
        // Object identity text is deliberately not compared across runtimes.
        if (!text.contains("rng=") || !text.endsWith("]")) throw new AssertionError(text);
        System.out.println(label + ".display=ok");
    }
    static void exception(String label, String expected, Runnable run) {
        try { run.run(); }
        catch (RuntimeException ex) {
            if (!ex.getClass().getSimpleName().equals(expected)) throw ex;
            System.out.println(label + "=" + expected);
            return;
        }
        throw new AssertionError(label + " did not throw");
    }
    static void helpers() {
        for (int n = 0; n <= 20; ++n)
            System.out.println("factorial.int." + n + "=" + InternalUtils.factorial(n));
        exception("factorial.negative", "ArrayIndexOutOfBoundsException", () -> InternalUtils.factorial(-1));
        for (int size : new int[] {0, 2, 5, 32, 80}) {
            InternalUtils.FactorialLog cache = InternalUtils.FactorialLog.create().withCache(size);
            for (int n : new int[] {0, 1, 2, 3, 4, 10, 19, 20, 21, 40, 80, 120})
                System.out.println("factorialLog." + size + ".double." + n + "=" + cache.value(n));
            InternalUtils.FactorialLog smaller = cache.withCache(3);
            System.out.println("factorialLog." + size + ".resized.double.2=" + smaller.value(2));
        }
        exception("factorialLog.negativeCache", "NegativeArraySizeException",
                  () -> InternalUtils.FactorialLog.create().withCache(-1));
        exception("factorialLog.negativeValue", "ArrayIndexOutOfBoundsException",
                  () -> InternalUtils.FactorialLog.create().withCache(3).value(-1));
        double[] xs = {-1, 0, 1e-8, 0.1, 0.5, 0.75, 1, 1.5, 2, 2.5, 3, 5, 7.5, 8, 15, 16, 100, 172, 1e6, Double.NaN, Double.POSITIVE_INFINITY};
        for (int i = 0; i < xs.length; ++i)
            System.out.println("logGamma.double." + i + "=" + InternalGamma.logGamma(xs[i]));
    }
    static final class ContinuousFunction implements ContinuousInverseCumulativeProbabilityFunction {
        int calls;
        public double inverseCumulativeProbability(double p) { return p * 3 - 1 + (++calls) * 0.125; }
    }
    static final class DiscreteFunction implements DiscreteInverseCumulativeProbabilityFunction {
        int calls;
        public int inverseCumulativeProbability(double p) { return (int) Math.floor(p * 7) + (++calls) * 100; }
    }
    public static void main(String[] ignored) {
        continuous("uniform", r -> new ContinuousUniformSampler(r, -2, 5));
        continuous("uniform.equal", r -> new ContinuousUniformSampler(r, 7, 7));
        continuous("uniform.reversed", r -> new ContinuousUniformSampler(r, 5, -2));
        continuous("uniform.zero", r -> new ContinuousUniformSampler(r, -0.0, 0.0));
        continuous("exponential", r -> new AhrensDieterExponentialSampler(r, 2.5));
        continuous("exponential.zero", r -> new AhrensDieterExponentialSampler(r, 0));
        for (double theta : new double[] {0.25, 1, 2, 10})
            continuous("gamma." + theta, r -> new AhrensDieterMarsagliaTsangGammaSampler(r, 2.5, theta));
        continuous("gaussian", r -> new BoxMullerGaussianSampler(r, -1, 2));
        continuous("gaussian.zero", r -> new BoxMullerGaussianSampler(r, 3, 0));
        continuous("gaussian.negativeSigma", r -> new BoxMullerGaussianSampler(r, 1, -2));
        continuous("logNormal", r -> new BoxMullerLogNormalSampler(r, 0.25, 0.5));
        continuous("logNormal.zero", r -> new BoxMullerLogNormalSampler(r, 1, 0));
        for (double[] shapes : new double[][] {{1, 1}, {0.2, 5}, {5, 0.2}, {2, 3}, {3, 2}})
            continuous("beta." + shapes[0] + "." + shapes[1], r -> new ChengBetaSampler(r, shapes[0], shapes[1]));
        ContinuousFunction continuousFunction = new ContinuousFunction();
        continuous("inverseContinuous", r -> new InverseTransformContinuousSampler(r, continuousFunction));
        System.out.println("inverseContinuous.callback.calls=" + continuousFunction.calls);
        continuous("inverseContinuous.shared", r -> new InverseTransformContinuousSampler(r, continuousFunction));
        System.out.println("inverseContinuous.shared.callback.calls=" + continuousFunction.calls);
        DiscreteFunction discreteFunction = new DiscreteFunction();
        discrete("inverseDiscrete", r -> new InverseTransformDiscreteSampler(r, discreteFunction));
        System.out.println("inverseDiscrete.callback.calls=" + discreteFunction.calls);
        discrete("inverseDiscrete.shared", r -> new InverseTransformDiscreteSampler(r, discreteFunction));
        System.out.println("inverseDiscrete.shared.callback.calls=" + discreteFunction.calls);
        continuous("pareto", r -> new InverseTransformParetoSampler(r, 2, 3));
        continuous("pareto.unit", r -> new InverseTransformParetoSampler(r, 1, 1));
        discrete("discrete", r -> new DiscreteUniformSampler(r, -7, 13));
        discrete("discrete.equal", r -> new DiscreteUniformSampler(r, 4, 4));
        discrete("discrete.wide", r -> new DiscreteUniformSampler(r, Integer.MIN_VALUE, 17));
        discrete("discrete.full", r -> new DiscreteUniformSampler(r, Integer.MIN_VALUE, Integer.MAX_VALUE));
        for (double mean : new double[] {0.1, 1, 39.5, 40, 80, 1000})
            discrete("poisson." + mean, r -> new PoissonSampler(r, mean));
        for (double exponent : new double[] {0.5, 1, 1.000000000001, 2})
            discrete("zipf." + exponent, r -> new RejectionInversionZipfSampler(r, 100, exponent));
        discrete("zipf.one", r -> new RejectionInversionZipfSampler(r, 1, 1));
        exception("discrete.invalid", "IllegalArgumentException", () -> new DiscreteUniformSampler(new TraceRng(), 3, 2));
        exception("poisson.zero", "IllegalArgumentException", () -> new PoissonSampler(new TraceRng(), 0));
        exception("poisson.negative", "IllegalArgumentException", () -> new PoissonSampler(new TraceRng(), -1));
        exception("zipf.zeroElements", "IllegalArgumentException", () -> new RejectionInversionZipfSampler(new TraceRng(), 0, 1));
        exception("zipf.negativeElements", "IllegalArgumentException", () -> new RejectionInversionZipfSampler(new TraceRng(), -1, 1));
        exception("zipf.zeroExponent", "IllegalArgumentException", () -> new RejectionInversionZipfSampler(new TraceRng(), 100, 0));
        exception("zipf.negativeExponent", "IllegalArgumentException", () -> new RejectionInversionZipfSampler(new TraceRng(), 100, -1));
        helpers();
        System.out.println("oracle=ok");
    }
}
