public class B {
    static class Base { int f(int a) { return a + 1; } int f(String s) { return s.length() + 100; } }
    static class Mid extends Base { }
    static class Child extends Mid { @Override int f(int a) { return a + 1000; } }
    public static void main(String[] args) {
        Base b = new Child();
        System.out.println(b.f(1) + "," + b.f("ab"));
    }
}
