public class W {
    static int total = 0;
    static class P { static class Counter { void bump() { total += 10; } } }
    public static void main(String[] args) {
        P.Counter c = new P.Counter();
        c.bump();
        c.bump();
        System.out.println(total);
    }
}
