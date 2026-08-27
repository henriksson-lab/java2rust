public class IntegerParseNull {
    static String parse(String s) {
        try {
            Integer.parseInt(s);
            return "ok";
        } catch (NullPointerException e) {
            return "npe";
        } catch (NumberFormatException e) {
            return "nfe";
        }
    }

    public static String run() {
        return parse(null) + "," + parse("bad") + "," + parse("7");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
