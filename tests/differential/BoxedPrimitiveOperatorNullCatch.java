public class BoxedPrimitiveOperatorNullCatch {
    static Boolean boolValue() {
        return null;
    }

    static Integer intValue() {
        return null;
    }

    static String condition() {
        try {
            if (boolValue()) {
                return "yes";
            }
            return "no";
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    static String binary() {
        try {
            int primitive = 1 + intValue();
            return "ok:" + primitive;
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    static String compare() {
        try {
            return intValue() > 2 ? "big" : "small";
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    static String unaryMinus() {
        try {
            int primitive = -intValue();
            return "ok:" + primitive;
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    static String unaryPlus() {
        try {
            int primitive = +intValue();
            return "ok:" + primitive;
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    public static String run() {
        return condition() + "," + binary() + "," + compare()
            + "," + unaryMinus() + "," + unaryPlus();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
