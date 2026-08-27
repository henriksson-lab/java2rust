class IntArray { int n = 9; }
public class Z2 {
    static int g(int[] a) { return a.length; }
    static int g(IntArray a) { return a.n; }
    public static void main(String[] args) {
        System.out.println(g(new int[3]) + "," + g(new IntArray()));
    }
}
