public class ErasedStringIdentityEquals {
    public static String run() {
        String fresh = new String("x");
        String literal = "x";
        Object erased = fresh;
        return erased.equals(literal) + "," + (fresh == literal);
    }

    public static void main(String[] args) { System.out.println(run()); }
}
