/** Diagnostic conformance probe; inputs model indexed multilingual text. */
public class UnicodeIndexingProbe {
    static String length() { return Integer.toString("Aé😀Z".length()); }
    static String units() {
        String value = "Aé😀Z";
        return Integer.toHexString(value.charAt(0)) + "," + Integer.toHexString(value.charAt(1)) + ","
            + Integer.toHexString(value.charAt(2)) + "," + Integer.toHexString(value.charAt(3)) + ","
            + Integer.toHexString(value.charAt(4));
    }
    static String positions() {
        String value = "Aé😀Z";
        return value.indexOf("Z") + "," + value.indexOf("😀") + "," + value.lastIndexOf("é") + ","
            + value.indexOf("Z",2) + "," + value.lastIndexOf("Z",4);
    }
    static String wholeSlice() {
        String value = "Aé😀Z";
        return value.substring(2,4).equals("😀") ? "true" : "false";
    }
    static String splitPair() {
        String value = "😀";
        String high = value.substring(0,1);
        String low = value.substring(1,2);
        return high.length() + "," + low.length() + "," + Integer.toHexString(high.charAt(0)) + ","
            + Integer.toHexString(low.charAt(0)) + "," + (high + low).equals(value);
    }
    static String numericChar() {
        char high = (char)0xd83d;
        return Integer.toHexString(high) + "," + Integer.toHexString((int)high);
    }
    public static void main(String[] args) {
        System.out.println("length="+length()); System.out.println("units="+units());
        System.out.println("positions="+positions()); System.out.println("wholeSlice="+wholeSlice());
        System.out.println("splitPair="+splitPair()); System.out.println("numericChar="+numericChar());
    }
}
