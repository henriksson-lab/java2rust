public class K {
    static class AB { int id() { return 1; } }
    static class A { static class B { int id() { return 2; } } }
    public static void main(String[] args) {
        AB x = new AB();
        A.B y = new A.B();
        System.out.println(x.id() + "," + y.id());
    }
}
