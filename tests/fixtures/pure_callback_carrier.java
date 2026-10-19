import java.util.function.IntSupplier;

class PureCallbackCarrier {
    static IntSupplier value(int expected) { return () -> expected; }
    static int local() {
        IntSupplier first = () -> 7;
        IntSupplier alias = first;
        return first.getAsInt() + alias.getAsInt();
    }
}
