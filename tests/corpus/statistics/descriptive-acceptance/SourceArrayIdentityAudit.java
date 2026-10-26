public class SourceArrayIdentityAudit {
    interface Transform<T, R> { R apply(T value); }
    interface Alias<A> extends Transform<A, A> { }
    static class Keep implements Alias<int[]> {
        int[] last;
        public int[] apply(int[] values) {
            last = values;
            values[0]++;
            return values;
        }
    }
    static Transform<int[], int[]> reference(Keep keep) { return keep::apply; }
    public static int run() {
        Keep keep = new Keep();
        Transform<int[], int[]> function = reference(keep);
        int[] values = {4, 8};
        int[] result = function.apply(values);
        if (result != values || keep.last != values || values[0] != 5) return 1;
        values[1] = 19;
        if (result[1] != 19 || function.apply(result) != values || values[0] != 6) return 2;
        Transform<int[], int[]> fresh = data -> data.clone();
        if (fresh.apply(values) == values) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
