public class ErasedStringAliasDisplay {
    public static String run() {
        String text = new String("x");
        Object erased = text;
        return text + "," + erased;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
