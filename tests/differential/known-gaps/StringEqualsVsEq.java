public class StringEqualsVsEq {
    public static String run() {
        String a = "hello";
        String b = "hel" + "lo";
        String c = new String("hello");
        String d = a;
        return (a == b) + "," + (a == c) + "," + (a == d) + "," + a.equals(c) + "," + a.equals("nope");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
