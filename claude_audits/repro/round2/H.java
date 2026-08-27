public class H {
    static class Base { int f(int a) { return a + 1; } int g() { return f(5); } }
    static class Child extends Base {
        @Override int f(int a) { return a + 1000; }
        int f(String s) { return s.length() + 100; }
    }
    public static void main(String[] args) {
        Child c = new Child();
        System.out.println(c.f(1) + "," + c.f("ab") + "," + c.g());
    }
}
