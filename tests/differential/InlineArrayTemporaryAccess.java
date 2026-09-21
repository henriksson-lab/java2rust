public class InlineArrayTemporaryAccess {
    static int calls;

    static class Box {
        int value;

        Box(int value) {
            this.value = value;
        }
    }

    static String stringElement() {
        return new String[] { "abc" }[0].substring(1);
    }

    static String nullableStringElement() {
        try {
            return new String[] { null }[0].substring(1);
        } catch (NullPointerException expected) {
            return "npe";
        }
    }

    static int objectElement() {
        return new Box[] { new Box(7) }[0].value;
    }

    static String referenceReturn() {
        return new String[] { "owned" }[0];
    }

    static String positiveBounds() {
        try {
            return new String[] { "a" }[2];
        } catch (ArrayIndexOutOfBoundsException expected) {
            return "positive-bounds";
        }
    }

    static String negativeBounds() {
        try {
            return new String[] { "a" }[-1];
        } catch (ArrayIndexOutOfBoundsException expected) {
            return "negative-bounds";
        }
    }

    static String singleEvaluation() {
        calls = 0;
        String value = new String[] { "abc" + calls++ }[0].substring(1);
        return value + ":" + calls;
    }

    public static String run() {
        return stringElement() + "," + nullableStringElement() + "," + objectElement()
            + "," + referenceReturn() + "," + positiveBounds() + "," + negativeBounds()
            + "," + singleEvaluation();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
