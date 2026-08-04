public class Q2 {
    private int i = 0;
    int bump() { i += 1; return i; }
    public String order() { return "" + bump() + bump() + bump(); }
    public int arr(int[] a) { int k = 0; a[k++] = k; return a[0]; }
}
