import org.apache.commons.rng.RestorableUniformRandomProvider;
import org.apache.commons.rng.core.source32.*;

/** Independent caller of all ten untouched native source32 implementations. */
public final class NativeGeneratorOracle {
    private static RestorableUniformRandomProvider generator(int kind, int[] seed) {
        switch (kind) {
        case 0: return new ISAACRandom(seed);
        case 1: return new KISSRandom(seed);
        case 2: return new MersenneTwister(seed);
        case 3: return new MultiplyWithCarry256(seed);
        case 4: return new Well512a(seed);
        case 5: return new Well1024a(seed);
        case 6: return new Well19937a(seed);
        case 7: return new Well19937c(seed);
        case 8: return new Well44497a(seed);
        case 9: return new Well44497b(seed);
        default: throw new AssertionError("generator kind " + kind);
        }
    }

    public static void main(String[] args) {
        String[] names = { "isaac", "kiss", "mt32", "mwc", "well512a", "well1024a",
                           "well19937a", "well19937c", "well44497a", "well44497b" };
        int[] large = new int[1536];
        for (int i = 0; i < large.length; i++) large[i] = 0x9e3779b9 * (i + 1);
        int[][] seeds = { {}, {0}, {1}, {-1}, {Integer.MIN_VALUE, Integer.MAX_VALUE},
                          {1, 17, -31, Integer.MIN_VALUE, Integer.MAX_VALUE}, large };
        for (int kind = 0; kind < names.length; kind++) {
            for (int s = 0; s < seeds.length; s++) {
                String label = names[kind] + "." + s;
                int[] seed = seeds[s].clone();
                RestorableUniformRandomProvider rng = generator(kind, seed);
                RestorableUniformRandomProvider control = generator(kind, seed.clone());
                if (seed.length > 0) seed[0] ^= 0x55aa55aa;
                for (int i = 0; i < 16; i++) {
                    if (rng.nextInt() != control.nextInt()) {
                        throw new AssertionError(label + " retained seed input");
                    }
                }
                // Cross complete state-buffer cycles, including the largest WELL
                // pool, without obscuring the following readable per-draw output.
                long hash = 0x0123456789abcdefL;
                for (int i = 0; i < 2048; i++) {
                    hash = Long.rotateLeft(hash, 7) ^ rng.nextInt();
                }
                System.out.println(label + ".warmupChecksum=" + hash);
                GeneratorOracle.exercise(label, rng);
            }
        }
        System.out.println("oracle=ok");
    }
}
