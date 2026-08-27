public class EnhancedForArray {
    public static String run() {
        int[] arr = {5, 6, 7};
        int sum = 0;
        for (int x : arr) { sum += x; }
        StringBuilder e = new StringBuilder();
        for (int x : arr) { if (x == 6) continue; if (x == 7) break; e.append(x); }
        String[] names = {"a", "b"};
        StringBuilder n = new StringBuilder();
        for (String s : names) { n.append(s).append("."); }
        return sum + "|" + e + "|" + n;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
