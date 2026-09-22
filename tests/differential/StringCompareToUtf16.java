public class StringCompareToUtf16 {
    public static String run() {
        int asciiMagnitude = "a".compareTo("c");
        int prefixMagnitude = "abc".compareTo("a");
        int supplementaryBeforeBmp = "𐀀".compareTo("");
        int bmpAfterSupplementary = "".compareTo("𐀀");
        return asciiMagnitude + ":" + prefixMagnitude + ":"
            + supplementaryBeforeBmp + ":" + bmpAfterSupplementary;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
