public class SwitchArms {
    static int arrowStmt(int x) {
        int r = -1;
        switch (x) { case 1 -> r = 11; default -> r = 99; }
        return r;
    }

    static int commaExpr(int x) {
        return switch (x) { case 1, 2 -> 22; default -> 99; };
    }

    static int commaStmt(int x) {
        int r = -1;
        switch (x) { case 1, 2: r = 22; break; default: r = 99; }
        return r;
    }

    static int sharedExpr(int x) {
        return switch (x) { case 1: case 2: yield 22; default: yield 99; };
    }

    public static String run() {
        return arrowStmt(1) + "," + arrowStmt(2)
            + "," + commaExpr(1) + "," + commaExpr(2)
            + "," + commaStmt(1) + "," + commaStmt(2)
            + "," + sharedExpr(1) + "," + sharedExpr(2);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
