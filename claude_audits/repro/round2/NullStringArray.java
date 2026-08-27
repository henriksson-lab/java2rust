public class NullStringArray {
    public static String run() {
        String[] a = new String[3];
        a[0] = "x";
        int nulls = 0;
        for (String s : a) { if (s == null) nulls++; }
        return (a[1] == null) + "," + a[0] + "," + a[1] + "," + nulls;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
