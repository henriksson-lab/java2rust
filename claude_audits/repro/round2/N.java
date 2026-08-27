public class N {
    static class P { static class Inner { int id() { return 1; } } }
    static class Q { static class Inner { int id() { return 2; } } }
    static class Inner { int id() { return 3; } }
    public static void main(String[] args) {
        P.Inner a = new P.Inner();
        Q.Inner b = new Q.Inner();
        Inner c = new Inner();
        System.out.println(a.id() + "," + b.id() + "," + c.id());
    }
}
