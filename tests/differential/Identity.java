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
 * This fixture is the acceptance check for the `SharedImmutable` handle plus
 * identity lowering. Java prints
 *
 *     5,true,false,false
 *
 * and the translation now prints the same. The alias read (`5`) checks that the
 * shared handle still exposes the field value, `a == b` checks identity for the
 * same object, and the two false cases check identity for distinct objects with
 * equal fields. The `.equals(c)` case matters because Java's inherited
 * `Object.equals` is identity too.
 *
 * The consequence for sequencing: `SharedImmutable` cannot be validated by a
 * differential fixture on its own — the gate would report "nothing changed",
 * which is also what a no-op reports. It had to land together with identity
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
