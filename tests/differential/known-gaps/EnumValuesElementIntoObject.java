/**
 * Known gap: an element of `Col.values()` read into an `Object`.
 *
 * `Col.BLUE` into an `Object` works (`EnumConstantAsObject.java` gates it). The
 * same value reached through `values()[i]` does not: the array read produces an
 * optional element and the erasure then calls `.unwrap()` on the enum *after* the
 * element has already been unwrapped, so it lowers to
 *
 *     Box::new(Col::values()[1].unwrap().clone())
 *
 * which is `E0599` -- `values()` returns `Vec<Self>`, whose elements are not
 * optional. The element shape used for the erased slot claims an optionality the
 * sequence does not have; this is the sequence-side twin of the doubled receiver
 * unwrap in 0056, and wants the same treatment -- one answer to "has this already
 * been discharged" rather than a per-site decision.
 *
 * Java prints "BLUE,BLUE".
 *
 * See claude_audits/0059-concatenation-erases-reference-values-and-collections-lose-their-contract.md.
 */
public class EnumValuesElementIntoObject {
    enum Col {
        RED,
        BLUE
    }

    public static String run() {
        Object viaValues = Col.values()[1];
        Col[] all = Col.values();
        return viaValues.toString() + "," + all[1].toString();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
