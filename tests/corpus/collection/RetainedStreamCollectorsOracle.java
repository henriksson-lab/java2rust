public class RetainedStreamCollectorsOracle {
    static final java.util.List<String> events = new java.util.ArrayList<>();
    static final class Key {
        final int value;
        Key(int value) { this.value = value; }
        public int hashCode() { return 1; }
        public boolean equals(Object other) {
            events.add("e");
            return other instanceof Key && value == ((Key) other).value;
        }
    }
    public static void main(String[] args) {
        java.util.Set<Key> result = RetainedStreamCollectors.mapped(
            java.util.Arrays.asList(new Key(1), new Key(2), new Key(1)), key -> {
                events.add("m" + key.value);
                return key;
            });
        if (result.size() != 2 || !events.equals(java.util.Arrays.asList("m1", "m2", "e", "m1", "e"))) {
            throw new AssertionError("collector effect order: " + events);
        }
        int ordered = RetainedStreamCollectors.run();
        int nullable = RetainedStreamCollectors.nullable();
        if (ordered != 3134 || nullable != 21 || RetainedStreamCollectors.checked() != 7
            || RetainedStreamCollectors.listPolicies() != 11) {
            throw new AssertionError("retained collector semantics: " + ordered + ", " + nullable);
        }
        System.out.println(ordered + "\t" + nullable);
    }
}
