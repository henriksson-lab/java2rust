/** Identity must survive ordinary borrowed/owned String API boundaries. */
public class StringReferenceViewProbe {
    static String relay(String value) { return value; }
    static String select(boolean first, String left, String right) { return first ? left : right; }
    static String nullable(String value) { return value == null ? null : relay(value); }
    static String twice(String value) { relay(value); return relay(value); }
    static int inspect(String value) { return value.length(); }
    static int inspectNullable(String value) { return value == null ? -1 : inspect(value); }
    static boolean read(String value) {
        return value.indexOf("a") == 1 && value.startsWith("s") && value.endsWith("e")
            && value.contains("am") && value.equals("same");
    }
    static double parse(String value) { return Double.parseDouble(value); }
    interface Reader { int read(String value); }
    static Reader reader() { return value -> inspect(value); }
    static Reader stored() { Reader value = item -> inspect(item); return value; }
    static Reader assigned() { Reader value = null; value = StringReferenceViewProbe::inspect; return value; }
    static Reader branch(boolean first) { return first ? item -> inspect(item) : StringReferenceViewProbe::inspect; }
    static int reboundRead(String value) { value = new String("same"); return inspect(value); }
    static Reader captured(String value) { return ignored -> inspect(value); }
    static boolean callbackNull() {
        Reader value = reader();
        try { value.read(null); return false; }
        catch (NullPointerException expected) { return true; }
    }
    static int temporary() { String value = relay(new String("same")); return inspect(value); }
    static String returnedTemporary() { return relay(new String("same")); }
    static String mixed(boolean first, String value) { if (first) return value; return new String("other"); }
    static Reader temporaryCapture() {
        String value = relay(new String("same"));
        return ignored -> inspect(value);
    }
    static Reader capturedAndReused() {
        String value = new String("same");
        Reader result = ignored -> inspect(value);
        if (inspect(value) != 4) throw new IllegalStateException();
        return result;
    }
    static boolean direct() {
        String value = new String("Aé😀Z");
        return relay(value) == value && twice(value) == value;
    }
    static boolean selected() {
        String left = new String("same");
        String right = new String("same");
        return select(true,left,right) == left && select(false,left,right) == right && left != right && inspect(left) == 4;
    }
    static boolean nulls() {
        String value = new String("x");
        return nullable(null) == null && nullable(value) == value && inspectNullable(null) == -1 && inspectNullable(value) == 1;
    }
    static boolean array() {
        String value = new String("x");
        String[] values = new String[] {value,null};
        return relay(values[0]) == value && nullable(values[1]) == null;
    }
    public static int run() {
        if (!direct()) return 1;
        if (!selected()) return 2;
        if (!nulls()) return 3;
        if (!array()) return 4;
        if (!read(new String("same"))) return 5;
        if (parse(new String("2.5")) != 2.5) return 6;
        if (reader().read(new String("same")) != 4) return 7;
        if (stored().read(new String("same")) != 4 || assigned().read(new String("same")) != 4) return 8;
        if (reboundRead(new String("old")) != 4 || captured(new String("same")).read(null) != 4) return 9;
        if (!callbackNull()) return 10;
        if (temporary() != 4 || !returnedTemporary().equals("same") || temporaryCapture().read(null) != 4) return 11;
        if (branch(true).read(new String("same")) != 4 || branch(false).read(new String("same")) != 4) return 12;
        String value = new String("same");
        if (mixed(true,value) != value || !mixed(false,value).equals("other")) return 13;
        if (capturedAndReused().read(null) != 4) return 14;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
