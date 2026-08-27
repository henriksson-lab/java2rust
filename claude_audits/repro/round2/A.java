public class A {
    public int xY(int a) { return a + 1; }
    public int x_Y(int a) { return a + 2; }
    public static void main(String[] args) {
        A a = new A();
        System.out.println(a.xY(1) + "," + a.x_Y(1));
    }
}
