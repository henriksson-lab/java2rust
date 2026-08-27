public class NewStringCtor {
    public static String run() {
        String a = new String("hello");
        return a + "," + a.length();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
