/**
 * Promoted regression for an enum array element read into an erased `Object`.
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
