public class ErasedStringBuilderDisplay {
    public static String run() {
        Object builder = new StringBuilder("ab").append(3);
        Object buffer = new StringBuffer("xy").append(false);
        return "builder=" + builder + "/" + builder.toString()
            + ",buffer=" + buffer + "/" + buffer.toString();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
