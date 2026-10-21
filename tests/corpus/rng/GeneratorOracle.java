import java.util.Arrays;
import org.apache.commons.rng.RandomProviderState;
import org.apache.commons.rng.RestorableUniformRandomProvider;
import org.apache.commons.rng.core.RandomProviderDefaultState;
import org.apache.commons.rng.core.source64.MersenneTwister64;
import org.apache.commons.rng.core.source64.SplitMix64;
import org.apache.commons.rng.core.source64.XorShift1024Star;
import org.apache.commons.rng.core.util.NumberFactory;

/** Independent reference output; compile with untouched pinned Java sources. */
public final class GeneratorOracle {
    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static void exercise(String label, RestorableUniformRandomProvider rng) {
        for (int i = 0; i < 64; i++) {
            System.out.println(label + ".long." + i + "=" + rng.nextLong());
        }
        RandomProviderState saved = rng.saveState();
        long[] replay = new long[16];
        for (int i = 0; i < replay.length; i++) replay[i] = rng.nextLong();
        rng.restoreState(saved);
        for (int i = 0; i < replay.length; i++) {
            require(replay[i] == rng.nextLong(), label + " restore replay");
        }
        for (int i = 0; i < 16; i++) {
            System.out.println(label + ".int." + i + "=" + rng.nextInt());
            System.out.println(label + ".boundedInt." + i + "=" + rng.nextInt(i + 1));
            System.out.println(label + ".boundedLong." + i + "=" + rng.nextLong((1L << i) + 1));
            System.out.println(label + ".bool." + i + "=" + rng.nextBoolean());
            System.out.println(label + ".float." + i + "=" + Float.toString(rng.nextFloat()));
            System.out.println(label + ".double." + i + "=" + Double.toString(rng.nextDouble()));
        }
        for (int length = 0; length <= 17; length++) {
            byte[] bytes = new byte[length + 4];
            Arrays.fill(bytes, (byte) 99);
            rng.nextBytes(bytes, 2, length);
            require(bytes[0] == 99 && bytes[1] == 99
                    && bytes[length + 2] == 99 && bytes[length + 3] == 99,
                    label + " byte slice overwrote sentinel");
            System.out.println(label + ".bytes." + length + "=" + Arrays.toString(bytes));
        }
        byte[] whole = new byte[19];
        rng.nextBytes(whole);
        System.out.println(label + ".wholeBytes=" + Arrays.toString(whole));
        boolean caught = false;
        try { rng.nextInt(0); } catch (IllegalArgumentException ex) { caught = true; }
        require(caught, label + " accepted zero bound");
        caught = false;
        try { rng.nextLong(-1); } catch (IllegalArgumentException ex) { caught = true; }
        require(caught, label + " accepted negative bound");
        caught = false;
        try { rng.nextBytes(whole, -1, 1); }
        catch (IndexOutOfBoundsException ex) { caught = true; }
        require(caught, label + " accepted negative byte offset");
        caught = false;
        try { rng.restoreState(new RandomProviderState() {}); }
        catch (IllegalArgumentException ex) { caught = true; }
        require(caught, label + " accepted foreign state");
        caught = false;
        try { rng.restoreState(new RandomProviderDefaultState(new byte[0])); }
        catch (IllegalArgumentException ex) { caught = true; }
        require(caught, label + " accepted malformed state");
    }

    private static void stateCopies() {
        byte[] initial = new byte[] { 1, 2, 3, 4 };
        RandomProviderDefaultState state = new RandomProviderDefaultState(initial);
        initial[0] = 99;
        byte[] copy = state.getState();
        require(copy[0] == 1, "state constructor retained input");
        copy[1] = 99;
        require(state.getState()[1] == 2, "state getter exposed internal array");
        long[] longs = new long[] { 0, -1, Long.MIN_VALUE, Long.MAX_VALUE, 0x0123456789abcdefL };
        require(Arrays.equals(longs, NumberFactory.makeLongArray(NumberFactory.makeByteArray(longs))),
                "long byte conversion round trip");
        int[] ints = new int[] { 0, -1, Integer.MIN_VALUE, Integer.MAX_VALUE, 0x01234567 };
        require(Arrays.equals(ints, NumberFactory.makeIntArray(NumberFactory.makeByteArray(ints))),
                "int byte conversion round trip");
    }

    public static void main(String[] args) {
        stateCopies();
        long[] seeds = new long[] { 0, 1, -1, Long.MIN_VALUE, Long.MAX_VALUE };
        for (int i = 0; i < seeds.length; i++) {
            exercise("split." + i, new SplitMix64(seeds[i]));
            long[] arraySeed = new long[] { seeds[i], 17, -31, Long.MIN_VALUE };
            MersenneTwister64 mt = new MersenneTwister64(arraySeed);
            XorShift1024Star xs = new XorShift1024Star(arraySeed);
            MersenneTwister64 mtControl = new MersenneTwister64(arraySeed.clone());
            XorShift1024Star xsControl = new XorShift1024Star(arraySeed.clone());
            arraySeed[0] ^= 0x55aa55aa55aa55aaL;
            for (int j = 0; j < 16; j++) {
                require(mt.nextLong() == mtControl.nextLong(), "MT retained seed input");
                require(xs.nextLong() == xsControl.nextLong(), "XorShift retained seed input");
            }
            exercise("mt." + i, mt);
            exercise("xor." + i, xs);
        }
        exercise("mt.emptySeed", new MersenneTwister64(new long[0]));
        System.out.println("oracle=ok");
    }
}
