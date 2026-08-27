/**
 * `new HashSet<>()` -- the diamond form -- is lowered as `Vec::new()` while the
 * slot is declared `std::collections::HashSet<String>`, so nothing compiles.
 * The explicit form `new HashSet<String>()` is lowered correctly, which is what
 * makes this a diamond defect rather than a set defect. `new TreeSet<>()` and
 * `new LinkedHashSet<>()` behave the same way; `new HashMap<>()` and
 * `new ArrayList<>()` do not.
 *
 * Java prints
 *
 *     1,1,1
 *
 * The translation does not compile:
 *
 *     error[E0308]: mismatched types
 *       let mut b: std::collections::HashSet<String> = Vec::new();
 */
import java.util.HashSet;
import java.util.Set;

public class SetConstruction {
    public static String run() {
        Set<String> a = new HashSet<String>();
        HashSet<String> b = new HashSet<>();
        Set<String> c = new HashSet<>();
        a.add("1");
        b.add("2");
        c.add("3");
        return a.size() + "," + b.size() + "," + c.size();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
