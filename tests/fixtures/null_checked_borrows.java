import java.util.Objects;
import java.util.Optional;
interface CheckedCounter {
    int size();
    void bump();
}
class CheckedCounterImpl implements CheckedCounter {
    int count;
    public int size() { return count; }
    public void bump() { count++; }
}
interface NonNullCheckedCounter { int size(); void bump(); }
class NonNullCheckedCounterImpl implements NonNullCheckedCounter {
    int count;
    public int size() { return count; }
    public void bump() { count++; }
}
class NullCheckedBorrows {
    static int trace;
    static String message() { trace = trace * 10 + 2; return "missing"; }
    static int discarded(CheckedCounter counter) {
        Objects.requireNonNull(counter, message());
        counter.bump();
        Objects.requireNonNull(counter);
        counter.bump();
        return counter.size();
    }
    static int nonNullLoan(NonNullCheckedCounter counter) {
        Objects.requireNonNull(counter, message());
        counter.bump();
        Objects.requireNonNull(counter);
        counter.bump();
        Objects.requireNonNull(counter, message()).bump();
        return counter.size();
    }
    static String counterMessage(NonNullCheckedCounter counter) {
        counter.bump();
        return "changed";
    }
    static int mutableMessage(NonNullCheckedCounter counter) {
        Objects.requireNonNull(counter, counterMessage(counter)).bump();
        counter.bump();
        return counter.size();
    }
    static int chained(CheckedCounter counter) {
        Objects.requireNonNull(counter, message()).bump();
        counter.bump();
        return counter.size();
    }
    static int nullable(CheckedCounter counter) {
        try {
            Objects.requireNonNull(counter, message()).bump();
            counter.bump();
            return counter.size();
        } catch (NullPointerException expected) {
            return -7;
        }
    }
    static CheckedCounter checkedCounter(CheckedCounter counter) {
        return Objects.requireNonNull(counter, message());
    }
    static int[] checkedArray(int[] values) {
        return Objects.requireNonNull(values, message());
    }
    static int arrayAlias(int[] values) {
        int[] checked = Objects.requireNonNull(values, message());
        checked[0] += 3;
        return values[0];
    }
    static String mutateMessage(int[] values) {
        values[0]++;
        trace = trace * 10 + 3;
        return "changed";
    }
    static int aliasDuringMessage(int[] values) {
        Objects.requireNonNull(values, mutateMessage(values))[0] += 2;
        return values[0];
    }
    static int nullableArray(int[] values) {
        try {
            Objects.requireNonNull(values, message())[0]++;
            return values[0];
        } catch (NullPointerException expected) { return -9; }
    }
    static int[] arrayValue(int[] values) { trace = trace * 10 + 1; return values; }
    static String throwingMessage() {
        trace = trace * 10 + 2;
        throw new IllegalStateException("message");
    }
    static int effectOrder(int[] values) {
        trace = 0;
        int[] checked = Objects.requireNonNull(arrayValue(values), message());
        if (trace != 12) return -1;
        checked[0]++;
        return values[0];
    }
    static int throwingOrder() {
        trace = 0;
        try {
            Objects.requireNonNull(arrayValue(null), throwingMessage());
            return -1;
        } catch (IllegalStateException expected) {
            return trace == 12 ? 0 : -2;
        }
    }
    static int run() {
        trace = 0;
        CheckedCounter counter = new CheckedCounterImpl();
        if (discarded(counter) != 2 || chained(counter) != 4) return 1;
        if (nullable(counter) != 6 || nullable(null) != -7 || trace != 2222) return 2;
        int[] values = new int[] { 1 };
        int[] alias = checkedArray(values);
        alias[0] = 5;
        if (values[0] != 5 || arrayAlias(values) != 8) return 3;
        Optional<String> empty = Optional.empty();
        if (Objects.requireNonNull(empty).isPresent()) return 4;
        if (Objects.requireNonNull(empty, message()).isPresent()) return 5;
        if (nonNullLoan(new NonNullCheckedCounterImpl()) != 3) return 6;
        if (aliasDuringMessage(values) != 11 || values[0] != 11) return 7;
        if (nullableArray(values) != 12 || nullableArray(null) != -9) return 8;
        if (mutableMessage(new NonNullCheckedCounterImpl()) != 3) return 9;
        if (effectOrder(values) != 13) return 11;
        if (throwingOrder() != 0) return 12;
        CheckedCounter returned = checkedCounter(counter);
        returned.bump();
        if (counter.size() != 7) return 13;
        CheckedCounter localAlias = Objects.requireNonNull(counter, message());
        localAlias.bump();
        if (counter.size() != 8) return 14;
        try {
            checkedArray(null);
            return 10;
        } catch (NullPointerException expected) {}
        return 0;
    }
}
