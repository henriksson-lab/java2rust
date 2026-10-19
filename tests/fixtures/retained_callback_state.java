import java.util.function.IntSupplier;
import java.util.function.Supplier;

class RetainedCallbackState {
    private IntSupplier stored;
    RetainedCallbackState(IntSupplier stored) { this.stored = stored; }
    int readStored() { return this.stored.getAsInt(); }
    static int held(int value) {
        RetainedCallbackState holder = new RetainedCallbackState(plain(value));
        return holder.readStored();
    }
    static Supplier<Integer> counter(int start) {
        final int[] state = new int[] { start };
        return () -> state[0]++;
    }

    static IntSupplier indexed(int[] values, int index) {
        return () -> values[index];
    }

    static IntSupplier plain(int value) {
        return () -> value;
    }

    static int sharedCounter(int start) {
        Supplier<Integer> first = counter(start);
        Supplier<Integer> second = first;
        return first.get() * 10 + second.get();
    }

    static int localCounter() {
        final int[] state = new int[] { 3 };
        Supplier<Integer> first = () -> state[0]++;
        Supplier<Integer> second = first;
        return first.get() * 10 + second.get();
    }

    static int seven() { return 7; }
    static IntSupplier reference() { return RetainedCallbackState::seven; }

    static int invokeReturned(int[] values, int index) {
        return indexed(values, index).getAsInt();
    }

    static int caughtReturned(int[] values, int index) {
        try {
            return invokeReturned(values, index);
        } catch (ArrayIndexOutOfBoundsException error) {
            return -1;
        }
    }

    static int caughtIndex(int[] values, int index) {
        IntSupplier deferred = indexed(values, index);
        try {
            return deferred.getAsInt();
        } catch (ArrayIndexOutOfBoundsException error) {
            return -1;
        }
    }
}
