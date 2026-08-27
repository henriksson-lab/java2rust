public class StringBuilderCharAtCatch {
    public static String run() {
        StringBuilder builder = new StringBuilder("ab");
        char first = builder.charAt(0);
        try {
            builder.charAt(3);
            return "miss";
        } catch (StringIndexOutOfBoundsException e) {
            return first + ":caught";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
