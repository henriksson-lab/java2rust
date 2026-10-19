import java.util.HashSet;
import java.util.Set;
import java.util.function.IntPredicate;

class SourceCallbackForwarding {
    static boolean check(IntPredicate predicate, int value) {
        return predicate.test(value);
    }
    static boolean forward(IntPredicate predicate, int value) {
        return check(predicate, value);
    }
    static boolean twice(IntPredicate predicate, int value) {
        boolean first = check(predicate, value);
        return first && check(predicate, value);
    }
    static boolean plain(int value) {
        return forward(item -> item > 0, value);
    }
    static boolean positive(int value) { return value > 0; }
    static boolean reference(int value) {
        return forward(SourceCallbackForwarding::positive, value);
    }
    static boolean mutable(int value) {
        Set<Integer> seen = new HashSet<Integer>();
        return twice(item -> seen.add(item), value);
    }
}
