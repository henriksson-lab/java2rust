public class NarrowParseNull {
    static String parseByteValue(String s) {
        try {
            byte value = Byte.parseByte(s);
            return "ok:" + value;
        } catch (NullPointerException e) {
            return "npe";
        } catch (NumberFormatException e) {
            return "nfe";
        }
    }

    static String parseShortValue(String s) {
        try {
            short value = Short.parseShort(s);
            return "ok:" + value;
        } catch (NullPointerException e) {
            return "npe";
        } catch (NumberFormatException e) {
            return "nfe";
        }
    }

    public static String run() {
        return parseByteValue(null) + "," + parseByteValue("bad") + "," + parseByteValue("127")
            + ";" + parseShortValue(null) + "," + parseShortValue("bad") + "," + parseShortValue("32767");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
