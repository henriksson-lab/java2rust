public class FloatTextOracle {
    static String units(String value) {
        StringBuilder result = new StringBuilder();
        for (int i=0;i<value.length();i++) {
            if (i != 0) result.append(',');
            result.append(Integer.toHexString(value.charAt(i)));
        }
        return result.toString();
    }
    public static void main(String[] args) {
        String[] inputs = {"", " ", "\t\ud800 ", "\ud800", "\udc00", "\ud800\udc00",
            "1..\ud800", "\ud800..", "0x1..\ud800p0", "NaN\ud800", "\u0000 1.25 \u0000",
            "１２", "é", "😀", "3.25", "0x1.8p2", "-0.0", "\u00a0"};
        for (String value : inputs) {
            String key = units(value);
            try { System.out.println("D|"+key+"|"+Long.toHexString(Double.doubleToRawLongBits(FloatParseProbe.parseDouble(value)))); }
            catch (Exception error) { System.out.println("D|"+key+"|ERR:"+error.getClass().getSimpleName()+":"+units(error.getMessage())); }
            try { System.out.println("F|"+key+"|"+Integer.toHexString(Float.floatToRawIntBits(FloatParseProbe.parseFloat(value)))); }
            catch (Exception error) { System.out.println("F|"+key+"|ERR:"+error.getClass().getSimpleName()+":"+units(error.getMessage())); }
        }
    }
}
