public class LongParseNull {
    static String parse(String s) {
        try {
            long value = Long.parseLong(s);
            return "ok:" + value;
        } catch (NullPointerException e) {
            return "npe";
        } catch (NumberFormatException e) {
            return "nfe";
        }
    }

    public static String run() {
        return parse(null) + "," + parse("bad") + "," + parse("9223372036854775807");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
