public class SwitchArms {
    // 1. arrow-form switch STATEMENT
    static int arrowStmt(int x) {
        int r = -1;
        switch (x) { case 1 -> r = 11; default -> r = 99; }
        return r;
    }
    // 2. comma-separated labels, arrow switch EXPRESSION
    static int commaExpr(int x) {
        return switch (x) { case 1, 2 -> 22; default -> 99; };
    }
    // 3. comma-separated labels, colon switch STATEMENT
    static int commaStmt(int x) {
        int r = -1;
        switch (x) { case 1, 2: r = 22; break; default: r = 99; }
        return r;
    }
    // 4. shared labels in a colon switch EXPRESSION
    static int sharedExpr(int x) {
        return switch (x) { case 1: case 2: yield 22; default: yield 99; };
    }
    // 5. colon switch EXPRESSION arm whose yield is inside a block
    static int blockExpr(int x) {
        return switch (x) { case 1: { int t = 4; yield t * 2; } default: yield 99; };
    }
    public static void main(String[] a) {
        System.out.println("arrowStmt  1=" + arrowStmt(1)  + " 2=" + arrowStmt(2));
        System.out.println("commaExpr  1=" + commaExpr(1)  + " 2=" + commaExpr(2));
        System.out.println("commaStmt  1=" + commaStmt(1)  + " 2=" + commaStmt(2));
        System.out.println("sharedExpr 1=" + sharedExpr(1) + " 2=" + sharedExpr(2));
        System.out.println("blockExpr  1=" + blockExpr(1)  + " 2=" + blockExpr(2));
    }
}
