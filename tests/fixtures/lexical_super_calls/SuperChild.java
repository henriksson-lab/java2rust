package neutral.supercalls;

public class SuperChild<B> extends SuperBase<B> {
    @Override public B echo(B value) { return super.echo(value); }
    public int inheritedMark(B value) { return super.mark(value); }
    public int viaSuperCallback(java.util.function.IntUnaryOperator action, int number) {
        return super.apply(action, number);
    }
    public SuperChild<B> self() { return this; }
    @Override public int checked(boolean fail) { return super.checked(fail); }
}
