/**
 * Functional interfaces passed as parameters and called through.
 *
 * The lowering turns such a parameter into a Rust closure, and the shape of that
 * closure — how many inputs, and what it returns — has to come from the
 * interface's name. It used to come from two hand-written tables that disagreed:
 * `Supplier` had an arity of zero in one and no entry at all in the other, so
 * `Supplier<String>` became `impl FnMut()` and its `-> String` was silently
 * dropped. That is `error[E0308]`, and nothing in the suite compiled the output,
 * so nothing saw it.
 *
 * The cases below are one per corner of the naming grammar:
 *
 *   - `Supplier<T>`         zero inputs, object result   (the dropped-return case)
 *   - `Function<A, B>`      object in, object out
 *   - `Predicate<T>`        object in, `boolean` out
 *   - `IntSupplier`         zero inputs, primitive result read from the *name*
 *   - `ToIntFunction<T>`    object in, primitive out
 *   - `Comparator<T>`       two inputs of the *same* argument, `int` out
 *
 * `Comparator` is the one that cannot be folded into the `Bi*` roots: both its
 * parameters read type argument 0, and it returns an ordering rather than a
 * boolean.
 *
 * The bodies call methods on their own parameters on purpose. A lambda written
 * directly at a functional-interface parameter used to reach its body untyped —
 * the parameter became `JavaType::Unknown("lambda_param:v")` — so `v.length()`
 * never mapped to `len` (`E0599`) and `String.valueOf(v)` treated `v` as an
 * erased object and returned the **empty string**, silently, because it
 * compiled. The interface supplies those types now, and these bodies are what
 * would notice if it stopped.
 *
 * A comparator handed to `Collections.sort` is covered in the slot-shape suite:
 * the escape check recognises that `Collections.sort(values, comparator)`
 * consumes the callback synchronously and lowers it as a closure call.
 *
 * Java prints "hi,ab,true,7,9,-1,4".
 */
import java.util.Comparator;
import java.util.function.Function;
import java.util.function.IntSupplier;
import java.util.function.Predicate;
import java.util.function.Supplier;
import java.util.function.ToIntFunction;

public class FunctionalParams {
    static String supply(Supplier<String> supplier) {
        return supplier.get();
    }

    static String apply(Function<String, String> function) {
        return function.apply("ab");
    }

    static int length(ToIntFunction<String> function) {
        return function.applyAsInt("abcd");
    }

    static boolean test(Predicate<Integer> predicate) {
        return predicate.test(5);
    }

    static int supplyInt(IntSupplier supplier) {
        return supplier.getAsInt();
    }

    static int measure(ToIntFunction<Integer> function) {
        return function.applyAsInt(8);
    }

    static int order(Comparator<Integer> comparator) {
        return comparator.compare(1, 2);
    }

    public static String run() {
        return supply(() -> "hi")
                + "," + apply(value -> String.valueOf(value))
                + "," + test(value -> value > 2)
                + "," + supplyInt(() -> 7)
                + "," + measure(value -> value + 1)
                + "," + order((left, right) -> left - right)
                + "," + length(value -> value.length());
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
