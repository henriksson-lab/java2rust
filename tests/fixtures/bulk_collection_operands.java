import java.util.ArrayList;
import java.util.List;

class BulkCollectionOperands {
    private List<Integer> stored = new ArrayList<Integer>();
    private static int trace;
    static void reset() { trace = 0; }
    static int trace() { return trace; }

    private static List<Integer> receiver(int mode) {
        trace = trace * 10 + 1;
        if (mode == 1) throw new IllegalArgumentException("receiver");
        if (mode == 2) return null;
        List<Integer> values = new ArrayList<Integer>();
        values.add(1);
        values.add(2);
        return values;
    }

    private static List<Integer> argument(int mode) {
        trace = trace * 10 + 2;
        if (mode == 1) throw new IllegalStateException("argument");
        if (mode == 2) return null;
        List<Integer> values = new ArrayList<Integer>();
        if (mode != 3) {
            values.add(2);
            values.add(3);
        }
        return values;
    }

    static boolean contains(int first, int second) {
        return receiver(first).containsAll(argument(second));
    }
    static boolean adds(int first, int second) {
        return receiver(first).addAll(argument(second));
    }
    static boolean removes(int first, int second) {
        return receiver(first).removeAll(argument(second));
    }
    static boolean retains(int first, int second) {
        return receiver(first).retainAll(argument(second));
    }
    static boolean nameCollision(List<Integer> value, List<Integer> expected) {
        return value.containsAll(expected);
    }
    static boolean stagingNames(List<Integer> __java_call_receiver, List<Integer> __java_call_argument_0) {
        return __java_call_receiver.containsAll(__java_call_argument_0);
    }
    static boolean sameCallContains() { return receiver(0).containsAll(receiver(0)); }
    static boolean sameCallRemoves() { return receiver(0).removeAll(receiver(0)); }
    static boolean sameCallRetains() { return receiver(0).retainAll(receiver(0)); }
    static boolean selfContains(List<Integer> values) { return values.containsAll(values); }
    static boolean selfAdds(List<Integer> values) { return values.addAll(values); }
    static boolean selfRemoves(List<Integer> values) { return values.removeAll(values); }
    static boolean selfRetains(List<Integer> values) { return values.retainAll(values); }
    static void addStatement(int first, int second) { receiver(first).addAll(argument(second)); }
    static void retainStatement(int first, int second) { receiver(first).retainAll(argument(second)); }
    static int crossKinds() {
        List<Integer> values = new ArrayList<Integer>();
        values.add(1);
        java.util.Set<Integer> source = new java.util.HashSet<Integer>();
        source.add(2);
        source.add(3);
        values.addAll(source);
        return values.size() * 100 + source.size() * 10 + (values.containsAll(source) ? 1 : 0);
    }
    int fieldSelf(int mode) {
        if (mode < 0) throw new IllegalArgumentException();
        this.stored.add(1);
        this.stored.add(2);
        if (mode == 1) this.stored = null;
        if (!this.stored.containsAll(this.stored) || this.stored.retainAll(this.stored)) return -1;
        this.stored.addAll(this.stored);
        int size = this.stored.size();
        if (!this.stored.removeAll(this.stored)) return -2;
        return size * 10 + this.stored.size();
    }
}
