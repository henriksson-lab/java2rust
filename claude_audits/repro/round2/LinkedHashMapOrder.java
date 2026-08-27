/**
 * `LinkedHashMap` guarantees insertion order and `TreeMap` guarantees key
 * order; both are part of the specified contract, unlike `HashMap`. The
 * translation maps every Java map onto `std::collections::HashMap`, so the
 * order is not merely different, it is *randomised per process* by Rust's
 * `RandomState` -- the same binary prints a different answer on consecutive
 * runs.
 *
 * Java prints
 *
 *     z=1;a=2;m=3;
 *
 * The translation compiles cleanly and prints, across five runs of one binary:
 *
 *     z=1;m=3;a=2;  /  z=1;a=2;m=3;  /  a=2;m=3;z=1;  /  a=2;z=1;m=3;  /  ...
 */
import java.util.LinkedHashMap;
import java.util.Map;

public class LinkedHashMapOrder {
    public static String run() {
        Map<String, String> m = new LinkedHashMap<>();
        m.put("z", "1");
        m.put("a", "2");
        m.put("m", "3");
        String out = "";
        for (String k : m.keySet()) {
            out = out + k + "=" + m.get(k) + ";";
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
