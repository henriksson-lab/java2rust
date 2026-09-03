/**
 * An enum constant read into an `Object`, and printed through the erased path.
 *
 * `Col.BLUE` is written like a static field read and is not one -- it is a
 * variant. The enum lowering that knows this was a *sibling* of the static-storage
 * lowering rather than a step inside it, so which of the two claimed the access
 * depended on the surrounding context: assigning to a `Col` local reached the enum
 * path, while assigning to `Object` reached the static-storage path and emitted
 * `Col::__static_BLUE()`, an accessor that is never generated for a variant
 * (`E0599`). The static-storage resolver now declines a field that is a variant of
 * its owner, so the enum path is the only lowering that can claim one.
 *
 * Fixing that alone turned the compile error into a *silent* wrong answer, which
 * 0018 ranks lower still: `Enum.toString` returns the constant's name, but enums
 * were left out of the erased-object downcast table, so `b.toString()` fell
 * through to the empty-string default and printed "" instead of "BLUE". The
 * generated enum already carried a `Display` writing `self.name()`; it is now
 * reachable through the erased path.
 *
 * The `Col` local covers the context that was always correct -- if a change makes
 * the two contexts disagree again, this fixture says so. The same read through
 * `Col.values()[i]` is a *different* gap and is parked in
 * `known-gaps/EnumValuesElementIntoObject.java`.
 *
 * Java prints "BLUE,RED,BLUE".
 *
 * See claude_audits/0059-concatenation-erases-reference-values-and-collections-lose-their-contract.md.
 */
public class EnumConstantAsObject {
    enum Col {
        RED,
        BLUE
    }

    public static String run() {
        Object boxed = Col.BLUE; // the erased context
        Col direct = Col.RED; // the context that already worked

        return boxed.toString() + "," + direct.toString() + "," + Col.BLUE;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
