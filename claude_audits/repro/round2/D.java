public class D {
    interface I {
        default int f(int a) { return a + 1; }
        default int f(String s) { return s.length() + 100; }
    }
    static class Impl implements I {
        @Override public int f(int a) { return a + 1000; }
    }
    public static void main(String[] args) {
        I i = new Impl();
        System.out.println(i.f(1) + "," + i.f("ab"));
    }
}
