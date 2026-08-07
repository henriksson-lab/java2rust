/**
 * The half of identity that must *not* become a pointer comparison.
 *
 * `Identity.java` covers a class that inherits `Object.equals`, where `.equals()`
 * is identity. This one overrides it, and the two operators then disagree in a
 * way Java is very specific about:
 *
 *   - `==` is reference identity **always**, whatever `equals` says;
 *   - `.equals()` is whatever the override says.
 *
 * So `a == c` is false while `a.equals(c)` is true, for the same pair. A
 * lowering that routed both through the same comparison would have to get one of
 * them wrong, which is why this is a fixture and not a unit test: only running
 * both languages shows the divergence.
 *
 * `Tag` is aliased into `holder` so it takes the shared representation
 * (claude_audits/0002b) — the case where `==` lowers to `Rc::ptr_eq` and the
 * overridden `.equals()` has to keep calling the method. Before 0028 both were
 * `==` on `Rc<Tag>`, which is structural, so `a == c` answered true.
 *
 * `hashCode` is deliberately absent: Java only requires it for hashing, this
 * fixture does not hash, and adding it would test a different thing.
 *
 * Java prints "true,false,true,false,true".
 *
 * See claude_audits/done/0028-reference-equality-lowered-as-structural-equality.md.
 */
public class IdentityCustomEquals {
    static final class Tag {
        final int v;

        Tag(int v) {
            this.v = v;
        }

        @Override
        public boolean equals(Object other) {
            if (!(other instanceof Tag)) {
                return false;
            }
            return ((Tag) other).v == v;
        }
    }

    static final class Holder {
        Tag kept;
    }

    public static String run() {
        Tag a = new Tag(5);
        Tag b = a; // an alias: the same object
        Tag c = new Tag(5); // a distinct object with equal contents

        // Aliasing `a` into a field is what makes `Tag` shared.
        Holder holder = new Holder();
        holder.kept = a;

        return (a == b) // true: same object
                + "," + (a == c) // false: `==` ignores the override
                + "," + a.equals(b) // true: the override says equal
                + "," + (holder.kept == c) // false: still a reference check
                + "," + a.equals(c); // true: the override, again
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
