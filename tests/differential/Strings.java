public class Strings {
    public static String run() {
        int a = 1, b = 2;
        String concat = "total = " + (a + b);
        String chained = "total = " + a + b;
        StringBuilder sb = new StringBuilder();
        sb.append("x").append(3).append('y');
        return concat + "|" + chained + "|" + sb.toString() + "|" + sb.length();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
