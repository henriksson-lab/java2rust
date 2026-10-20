package neutral.supercalls;

public class SuperBase<A> {
    public int calls;
    public A echo(A value) { calls++; return value; }
    public int mark(A value) { calls += 2; return calls; }
    public int apply(java.util.function.IntUnaryOperator action, int number) {
        return action.applyAsInt(number);
    }
    public int checked(boolean fail) {
        calls++;
        if (fail) throw new IllegalStateException("lexical super");
        return calls;
    }
}
