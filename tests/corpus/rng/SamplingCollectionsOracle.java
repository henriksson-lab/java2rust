import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.apache.commons.rng.UniformRandomProvider;
import org.apache.commons.rng.sampling.CollectionSampler;
import org.apache.commons.rng.sampling.ListSampler;
import org.apache.commons.rng.sampling.PermutationSampler;

/** Caller-only oracle; all sampling algorithms come from untouched upstream. */
public final class SamplingCollectionsOracle {
    static final class Scripted implements UniformRandomProvider {
        private static final int[] VALUES = {0, 7, 1, Integer.MAX_VALUE, 3, 0, 11, 2, 19};
        private int position;
        final List<Integer> bounds = new ArrayList<Integer>();
        public int nextInt(int n) {
            if (n <= 0) throw new IllegalArgumentException("bound");
            bounds.add(n);
            return VALUES[position++ % VALUES.length] % n;
        }
        public int nextInt() { throw new AssertionError("unexpected nextInt"); }
        public long nextLong() { throw new AssertionError("unexpected nextLong"); }
        public long nextLong(long n) { throw new AssertionError("unexpected boundedLong"); }
        public boolean nextBoolean() { throw new AssertionError("unexpected boolean"); }
        public float nextFloat() { throw new AssertionError("unexpected float"); }
        public double nextDouble() { throw new AssertionError("unexpected double"); }
        public void nextBytes(byte[] bytes) { throw new AssertionError("unexpected bytes"); }
        public void nextBytes(byte[] bytes, int start, int len) { throw new AssertionError("unexpected bytes"); }
    }
    static final class Tag {
        final int identity;
        int value;
        Tag(int identity, int value) { this.identity = identity; this.value = value; }
        public String toString() { return identity + ":" + value; }
    }
    interface Action { void run(); }
    static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
    static void exception(String label, String expected, Action action) {
        try { action.run(); }
        catch (RuntimeException error) {
            require(error.getClass().getSimpleName().equals(expected), label + " wrong exception " + error);
            System.out.println(label + "=" + expected);
            return;
        }
        throw new AssertionError(label + " did not throw");
    }
    static void permutations() {
        for (int n : new int[] {1, 2, 5, 17}) {
            for (int k : new int[] {1, n}) {
                Scripted rng = new Scripted();
                Scripted controlRng = new Scripted();
                PermutationSampler sampler = new PermutationSampler(rng, n, k);
                PermutationSampler control = new PermutationSampler(controlRng, n, k);
                for (int repeat = 0; repeat < 5; repeat++) {
                    int[] sample = sampler.sample();
                    require(Arrays.equals(sample, control.sample()), "returned sample aliases internal domain");
                    boolean[] seen = new boolean[n];
                    require(sample.length == k, "sample size");
                    for (int value : sample) {
                        require(value >= 0 && value < n && !seen[value], "invalid permutation");
                        seen[value] = true;
                    }
                    System.out.println("permutation." + n + "." + k + "." + repeat + "=" + Arrays.toString(sample));
                    sample[0] = -999; // Must not corrupt subsequent sampling.
                }
                System.out.println("permutation." + n + "." + k + ".bounds=" + rng.bounds);
            }
        }
        for (int n : new int[] {0, 1, 5})
            System.out.println("natural." + n + "=" + Arrays.toString(PermutationSampler.natural(n)));
        exception("natural.negative", "NegativeArraySizeException", () -> PermutationSampler.natural(-1));
        for (final int[] nk : new int[][] {{-1, 1}, {0, 1}, {3, 0}, {3, -1}, {3, 4}})
            exception("permutation.invalid." + nk[0] + "." + nk[1], "IllegalArgumentException",
                      () -> new PermutationSampler(new Scripted(), nk[0], nk[1]));
    }
    static void shuffles() {
        for (int length : new int[] {0, 1, 2, 7}) {
            for (int start : new int[] {0, Math.max(0, length / 2), length}) {
                for (boolean head : new boolean[] {false, true}) {
                    // The implementation intentionally performs no bounds precheck.
                    // Test defined no-op endpoints here; actual invalid reads below.
                    if (head && start >= length) continue;
                    Scripted arrayRng = new Scripted();
                    Scripted listRng = new Scripted();
                    int[] array = PermutationSampler.natural(length);
                    int[] alias = array;
                    List<String> list = new ArrayList<String>();
                    for (int i = 0; i < length; i++) list.add("v" + i);
                    List<String> listAlias = list;
                    PermutationSampler.shuffle(arrayRng, array, start, head);
                    ListSampler.shuffle(listRng, list, start, head);
                    require(alias == array && listAlias == list, "shuffle replaced input identity");
                    for (int i = 0; i < length; i++) {
                        require(listAlias.get(i).equals("v" + alias[i]), "array/list shuffle disagreement");
                        if ((head && i > start) || (!head && i < start))
                            require(alias[i] == i, "partial shuffle changed sentinel");
                    }
                    require(arrayRng.bounds.equals(listRng.bounds), "shuffle RNG consumption");
                    String label = "shuffle." + length + "." + start + "." + head;
                    System.out.println(label + "=" + Arrays.toString(alias));
                    System.out.println(label + ".list=" + listAlias);
                    System.out.println(label + ".bounds=" + arrayRng.bounds);
                }
            }
            Scripted rng = new Scripted();
            int[] full = PermutationSampler.natural(length);
            PermutationSampler.shuffle(rng, full);
            System.out.println("shuffle.full." + length + "=" + Arrays.toString(full));
            Scripted listRng = new Scripted();
            List<Integer> fullList = new ArrayList<Integer>();
            for (int i = 0; i < length; i++) fullList.add(i);
            ListSampler.shuffle(listRng, fullList);
            System.out.println("shuffle.fullList." + length + "=" + fullList);
            require(rng.bounds.equals(listRng.bounds), "full shuffle consumption");
        }
        exception("shuffle.array.invalid", "ArrayIndexOutOfBoundsException",
                  () -> PermutationSampler.shuffle(new Scripted(), new int[0], 0, true));
        exception("shuffle.list.invalid", "ArrayIndexOutOfBoundsException",
                  () -> ListSampler.shuffle(new Scripted(), new ArrayList<String>(), 0, true));
    }
    static void collections() {
        Tag a = new Tag(1, 7);
        Tag b = new Tag(2, 7); // Equal payload, different identity.
        List<Tag> original = new ArrayList<Tag>(Arrays.asList(a, b, a, null));
        for (int k : new int[] {1, 2, 4}) {
            Scripted rng = new Scripted();
            for (int repeat = 0; repeat < 3; repeat++) {
                List<Tag> sample = ListSampler.sample(rng, original, k);
                require(original.size() == 4 && original.get(0) == a && original.get(1) == b
                        && original.get(2) == a && original.get(3) == null, "sample mutated source");
                for (Tag value : sample) require(value == null || value == a || value == b, "sample copied element");
                System.out.println("list.sample." + k + "." + repeat + "=" + sample);
                sample.clear();
                require(original.size() == 4, "result aliases source container");
            }
            System.out.println("list.sample." + k + ".bounds=" + rng.bounds);
        }
        Scripted rng = new Scripted();
        CollectionSampler<Tag> sampler = new CollectionSampler<Tag>(rng, original);
        original.clear(); // Constructor must shallow-copy the container.
        original.add(new Tag(99, 99));
        a.value = 42; // But element identities must remain shared.
        for (int i = 0; i < 20; i++) {
            Tag sampled = sampler.sample();
            require(sampled == null || sampled == a || sampled == b, "collection identity/isolation");
            System.out.println("collection.sample." + i + "=" + sampled);
        }
        System.out.println("collection.bounds=" + rng.bounds);
        // Two stateful samplers must retain the same supplied RNG identity.
        PermutationSampler other = new PermutationSampler(rng, 5, 3);
        System.out.println("sharedRng.permutation=" + Arrays.toString(other.sample()));
        System.out.println("sharedRng.collection=" + sampler.sample());
        System.out.println("sharedRng.bounds=" + rng.bounds);
        exception("collection.empty", "IllegalArgumentException",
                  () -> new CollectionSampler<String>(new Scripted(), new ArrayList<String>()));
        for (final int k : new int[] {-1, 0, 2})
            exception("list.sample.invalid." + k, "IllegalArgumentException",
                      () -> ListSampler.sample(new Scripted(), Arrays.asList("one"), k));
    }
    public static void main(String[] args) {
        permutations();
        shuffles();
        collections();
        System.out.println("oracle=ok");
    }
}
