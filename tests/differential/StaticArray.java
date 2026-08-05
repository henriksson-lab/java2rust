/**
 * A `static` array field: its declared value, and writes to its elements.
 *
 * Two independent defects lived on this one construct, and only one of them was
 * loud.
 *
 * **The initializer was dropped.** `static int[] counter = new int[3]` had no
 * arm in the static-storage initial-value lowering at all, so it fell to the
 * fallback `Vec::new()` — the right type and the wrong length, which is the
 * shape claude_audits/done/0034 fixed for *local* allocations. The first index
 * panicked.
 *
 * **Writes were silently discarded.** The accessor returns the value *by value*
 * — it clones out of the mutex — so `Self::__static_counter()[0] = v` assigned
 * into a temporary and vanished. Rust accepts that: the call result is a place
 * expression, so indexing and assigning to it is legal and does nothing. It
 * compiled, ran, warned about nothing, and the array never changed. Fixing only
 * the initializer would have turned the panic into a quiet wrong answer, which
 * is why the write half had to land first.
 *
 * `bump()` is called twice on purpose: a single write can be masked by a
 * coincidence, two cannot. `swap` covers a write whose index and value both
 * read the same static — the case that deadlocks if the guard is taken before
 * the operands are evaluated, since a `Mutex` is not reentrant.
 *
 * Java prints "2,3,8,6,2,150".
 *
 * See claude_audits/done/0039-static-array-fields-lose-their-value-and-their-writes.md.
 */
public class StaticArray {
    static int[] counter = new int[3];
    static int[] named = {7, 8};
    static String[] words = {"a", "b"};

    static void bump() {
        counter[0] = counter[0] + 1;
    }

    /** Index and value both read the static being written. */
    static void swap() {
        counter[counter[0]] = counter[0] + 4;
    }

    public static String run() {
        bump();
        bump();
        swap();

        words[1] = words[0] + "z";

        int total = 0;
        for (int i = 0; i < named.length; i++) {
            total = total + named[i] * 10;
        }

        return counter[0]
                + "," + counter.length
                + "," + named[1]
                + "," + counter[2]
                + "," + words[1].length()
                + "," + total;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
