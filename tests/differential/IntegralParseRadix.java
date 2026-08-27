public class IntegralParseRadix {
    static String parse(String value, int radix) {
        try {
            byte b = Byte.parseByte(value, radix);
            short s = Short.parseShort(value, radix);
            int i = Integer.parseInt(value, radix);
            long l = Long.parseLong(value, radix);
            return "ok:" + b + "/" + s + "/" + i + "/" + l;
        } catch (NumberFormatException e) {
            return "nfe";
        }
    }

    public static String run() {
        return parse("7f", 16) + ";"
            + parse("1111111", 2) + ";"
            + parse("177", 8) + ";"
            + parse("80", 16) + ";"
            + parse(null, 16) + ";"
            + parse("10", 1);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
