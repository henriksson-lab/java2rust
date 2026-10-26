import java.util.SplittableRandom;
import java.util.function.IntUnaryOperator;

public final class SplittableRandomProbe {
    static final long[] SEEDS = {Long.MIN_VALUE, -1L, 0L, 1L, 42L, -987654321098765L, Long.MAX_VALUE};
    // Independent OpenJDK 19.0.2 traces; see the recorded oracle command.
    static final long[] EXPECTED = {4280137590629728248L, 3065465027046608222L, 7422911040298624290L, 2388647289574123983L, -619341445683199731L, -4369896466068835716L, -1666874438537432104L};
    static long mix(long h, long value) { return h * 6364136223846793005L + value; }
    static long trace(long seed) {
        SplittableRandom rng = new SplittableRandom(seed);
        int[] bounds = {1, 2, 3, 17, 65536, 1073741825, Integer.MAX_VALUE};
        long h = 1;
        for (int i = 0; i < 256; i++) {
            h = mix(h, rng.nextInt());
            h = mix(h, rng.nextInt(bounds[i % bounds.length]));
            int mode = i % 4;
            if (mode == 0) h = mix(h, rng.nextInt(-50, 93));
            else if (mode == 1) h = mix(h, rng.nextInt(Integer.MIN_VALUE, Integer.MAX_VALUE));
            else if (mode == 2) h = mix(h, rng.nextInt(-2000000000, 2000000000));
            else h = mix(h, rng.nextInt(0, 1));
            h = mix(h, rng.nextLong());
            if (i % 17 == 0) {
                SplittableRandom child = rng.split();
                h = mix(h, child.nextInt());
                h = mix(h, child.nextInt(1073741825));
                h = mix(h, child.nextLong());
                h = mix(h, rng.nextLong());
            }
        }
        return h;
    }
    static int seedFromInt(int seed) { return new SplittableRandom(seed).nextInt(17); }
    static int narrow(byte bound) { return new SplittableRandom(42L).nextInt(bound); }
    public static int run() {
        for (int i = 0; i < SEEDS.length; i++) if (trace(SEEDS[i]) != EXPECTED[i]) return i + 1;
        SplittableRandom shared = new SplittableRandom(42L);
        SplittableRandom alias = shared;
        if (shared != alias) return 21;
        SplittableRandom reference = new SplittableRandom(42L);
        IntUnaryOperator callback = shared::nextInt;
        for (int i = 0; i < 32; i++) {
            if (callback.applyAsInt(1000003) != reference.nextInt(1000003)) return 8;
            if (alias.nextInt() != reference.nextInt()) return 9;
            if (shared.nextLong() != reference.nextLong()) return 10;
        }
        try { shared.nextInt(0); return 11; }
        catch (IllegalArgumentException e) { if (!"bound must be positive".equals(e.getMessage())) return 12; }
        try { callback.applyAsInt(-1); return 13; }
        catch (IllegalArgumentException e) { }
        try { shared.nextInt(8, 8); return 14; }
        catch (IllegalArgumentException e) { if (!"bound must be greater than origin".equals(e.getMessage())) return 15; }
        if (shared.nextLong() != reference.nextLong()) return 16;
        if (seedFromInt(42) != new java.util.SplittableRandom(42L).nextInt(17)) return 17;
        if (narrow((byte) 17) != seedFromInt(42)) return 22;
        IntUnaryOperator escaped = new SplittableRandom(42L)::nextInt;
        reference = new SplittableRandom(42L);
        if (escaped.applyAsInt(17) != reference.nextInt(17) || escaped.applyAsInt(17) != reference.nextInt(17)) return 18;
        SplittableRandom unseeded = new SplittableRandom();
        boolean varied = false;
        int first = unseeded.nextInt(1000003);
        for (int i = 0; i < 32; i++) {
            int draw = unseeded.nextInt(1000003);
            if (draw < 0 || draw >= 1000003) return 19;
            if (draw != first) varied = true;
        }
        if (!varied) return 20;
        return 0;
    }
    public static void main(String[] args) {
        if (args.length != 0) {
            for (long seed : SEEDS) System.out.println(trace(seed));
        } else System.out.println(run());
    }
}
