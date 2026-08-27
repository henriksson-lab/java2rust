public class N2 {
    static class P { static class Inner { int id() { return 1; } } }
    public static void main(String[] args) {
        P.Inner a = new P.Inner();
        N2.P.Inner b = new N2.P.Inner();
        System.out.println(a.id() + "," + b.id());
    }
}
