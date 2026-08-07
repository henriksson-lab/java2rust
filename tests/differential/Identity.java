/**
 * Object *identity*, which is the only thing a shared representation buys for a
 * type that is never mutated.
 *
 * This fixture exists to make a sequencing claim checkable. `0002b` recommends
 * implementing the `SharedImmutable` tier (`Rc<T>`) first because it has no
 * `borrow()` and no panic risk — good engineering advice, but it hides that the
 * tier has **no observable behaviour of its own**: if a type is never mutated
 * after being aliased, a copy and a shared handle are indistinguishable.
 *
 * Everything except identity already agrees. Java prints
 *
 *     5,true,false,false
 *
 * and the translation prints
 *
 *     5,true,true,true
 *
 * so the alias read (`5`) and the true-identity case (`a == b`) match, and the
 * two divergences are both identity comparisons on *distinct* objects:
 *
 *   - `a == c` is structural, so two distinct objects with equal fields compare
 *     equal — claude_audits/0028;
 *   - `a.equals(c)` is the same defect through the other operator. Java's
 *     `Object.equals` **is** identity unless overridden, and this class does not
 *     override it. 0028 names this as its "other half" and does not design it;
 *     this is the reproducer for it.
 *
 * The consequence for sequencing: `SharedImmutable` cannot be validated by a
 * differential fixture on its own — the gate would report "nothing changed",
 * which is also what a no-op reports. It has to land together with identity
 * lowering (`Rc::ptr_eq`), which is what makes this file pass.
 *
 * See claude_audits/0002b-shared-representation-design.md and
 * claude_audits/done/0028-reference-equality-lowered-as-structural-equality.md.
 */
public class Identity {
    static final class Key {
        final int v;

        Key(int v) {
            this.v = v;
        }

        int get() {
            return v;
        }
    }

    public static String run() {
        Key a = new Key(5);
        Key b = a; // an alias that is never mutated
        Key c = new Key(5); // a distinct object with equal contents

        return b.get()
                + "," + (a == b) // true: same object
                + "," + (a == c) // false: distinct objects
                + "," + a.equals(c); // false: Object.equals is identity
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
