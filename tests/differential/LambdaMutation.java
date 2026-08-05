/**
 * A method that mutates the receiver only *inside a lambda* still needs
 * `&mut self`.
 *
 * `forEach` is lowered to a real loop with the lambda body inlined, so by
 * emission time the call is a plain `self.take(...)`. The usage walk did record
 * that call, but it was unresolved and had no receiver type, so the mutability
 * fixpoint did not recognize it as a self call. The method came out `&self` and
 * the output failed to compile with `E0596: cannot borrow *self as mutable`.
 *
 * The narrower cause: `resolve_expr_calls` does not descend into lambda bodies,
 * so the call inside one carries no resolved callee, and the self-mutability
 * fixpoint had no way to identify it. An *unqualified* call needs no resolution
 * to be identified as a self call — the enclosing type is already known.
 *
 * Java prints "3,3,2".
 *
 * See claude_audits/done/0033-receiver-mutability-not-propagated-through-lambdas.md.
 */
import java.util.ArrayList;
import java.util.List;

public class LambdaMutation {
    private List<String> items = new ArrayList<>();
    private int calls = 0;

    /** Mutates through a field. */
    void take(String s) {
        items.add(s);
    }

    /** Mutates a scalar field, reached only through a second hop. */
    void bump() {
        calls = calls + 1;
    }

    void bumpTwice() {
        bump();
        bump();
    }

    /** The shape from the audit: the only mutation is inside the lambda. */
    void collectAll(List<String> xs) {
        xs.forEach(x -> take(x));
    }

    /** Two hops from the lambda, so the fixpoint has to iterate. */
    void countAll(List<String> xs) {
        xs.forEach(x -> bumpTwice());
    }

    /**
     * Deliberately avoids `items.get(i)` inside a `StringBuilder.append`: that
     * shape trips an unrelated optional-carrier defect, and a fixture that
     * cannot pass for a second reason cannot pin the first one.
     */
    int total() {
        int n = 0;
        for (String item : items) {
            n = n + item.length();
        }
        return n;
    }

    public static String run() {
        List<String> source = new ArrayList<>();
        source.add("a");
        source.add("b");
        source.add("c");

        LambdaMutation sink = new LambdaMutation();
        sink.collectAll(source);

        LambdaMutation counter = new LambdaMutation();
        List<String> pair = new ArrayList<>();
        pair.add("x");
        pair.add("y");
        counter.countAll(pair);

        return sink.total() + "," + sink.items.size() + "," + (counter.calls / 2);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
