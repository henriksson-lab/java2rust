/**
 * `Map.put` returns the *previous* value for the key, or `null` when the key
 * was absent. The translation lowers `put` as `HashMap::insert(..).unwrap()`,
 * so the first put -- which returns `None` -- panics.
 *
 * Java prints
 *
 *     null,1
 *
 * The translation panics:
 *
 *     called `Option::unwrap()` on a `None` value
 *
 * from `let p1: Box<dyn std::any::Any> =
 *          Box::new(m.insert("a".to_string(), 1).unwrap()) as Box<dyn std::any::Any>;`
 */
import java.util.HashMap;
import java.util.Map;

public class MapPutPrevious {
    public static String run() {
        Map<String, Integer> m = new HashMap<>();
        Integer p1 = m.put("a", 1);
        Integer p2 = m.put("a", 2);
        return p1 + "," + p2;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
