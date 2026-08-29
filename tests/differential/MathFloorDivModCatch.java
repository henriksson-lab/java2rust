public class MathFloorDivModCatch {
    public static void main(String[] args) {
        System.out.print(run());
    }

    public static String run() {
        return divInt() + "," + modInt() + "," + divLong() + "," + modLong();
    }

    static String divInt() {
        try {
            return "v:" + Math.floorDiv(7, 0);
        } catch (ArithmeticException ex) {
            return "caught-div-int";
        }
    }

    static String modInt() {
        try {
            return "v:" + Math.floorMod(7, 0);
        } catch (ArithmeticException ex) {
            return "caught-mod-int";
        }
    }

    static String divLong() {
        try {
            return "v:" + Math.floorDiv(7L, 0L);
        } catch (ArithmeticException ex) {
            return "caught-div-long";
        }
    }

    static String modLong() {
        try {
            return "v:" + Math.floorMod(7L, 0);
        } catch (ArithmeticException ex) {
            return "caught-mod-long";
        }
    }
}
