public class FloatingParseNull {
    static String parseFloatValue(String s) {
        try {
            Float.parseFloat(s);
            return "ok";
        } catch (NullPointerException e) {
            return "npe";
        } catch (NumberFormatException e) {
            return "nfe";
        }
    }

    static String parseDoubleValue(String s) {
        try {
            Double.parseDouble(s);
            return "ok";
        } catch (NullPointerException e) {
            return "npe";
        } catch (NumberFormatException e) {
            return "nfe";
        }
    }

    public static String run() {
        return parseFloatValue(null) + "," + parseFloatValue("bad") + "," + parseFloatValue("1.5")
            + ";" + parseDoubleValue(null) + "," + parseDoubleValue("bad") + "," + parseDoubleValue("2.5");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
