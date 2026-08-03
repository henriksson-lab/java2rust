public class Overloads {
    static class Base {
        double f(int a, int b) { return f(a) - f(b); }
        double f(int a) { return a; }
    }
    static class Child extends Base {
        @Override double f(int a) { return a * 2; }
    }
    public static String run() {
        Base base = new Base();
        Child child = new Child();
        return base.f(3) + "," + base.f(3, 1) + "," + child.f(3) + "," + child.f(3, 1);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
