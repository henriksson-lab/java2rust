import java.util.Map;
import java.util.Set;
import java.util.List;

public class NullableReceiverMatrix {
    static class Node {
        String text() { return "ok"; }
    }

    private static String source() {
        Node value = null;
        try { return value.text(); }
        catch (NullPointerException ex) { return "npe"; }
    }

    private static Node missing() { return null; }

    private static String sourceCall() {
        try { return missing().text(); }
        catch (NullPointerException ex) { return "npe"; }
    }

    private static String string() {
        String value = null;
        try { return value.substring(0); }
        catch (NullPointerException ex) { return "npe"; }
    }

    private static String stringEquals() {
        String value = null;
        try { return "" + value.equals("x"); }
        catch (NullPointerException ex) { return "npe"; }
    }

    private static String stringEqualsNull() {
        String value = null;
        try { return "" + value.equals(null); }
        catch (NullPointerException ex) { return "npe"; }
    }

    private static String objectEquals() {
        Object value = null;
        try { return "" + value.equals("x"); }
        catch (NullPointerException ex) { return "npe"; }
    }

    private static String objectEqualsNull() {
        Object value = null;
        try { return "" + value.equals(null); }
        catch (NullPointerException ex) { return "npe"; }
    }

    private static String objectStringEquals() {
        Object value = "x";
        return "" + value.equals("x");
    }

    private static String objectStringEqualsNull() {
        Object value = "x";
        return "" + value.equals(null);
    }

    private static String objectIntegerEquals() {
        Object value = 2;
        return "" + value.equals(2);
    }

    private static String objectIntegerDifferentType() {
        Object value = 2;
        return "" + value.equals(2L);
    }

    private static String builder() {
        StringBuilder value = null;
        try { return "" + value.length(); }
        catch (NullPointerException ex) { return "npe"; }
    }

    private static String list() {
        List<String> value = null;
        try { return "" + value.size(); }
        catch (NullPointerException ex) { return "npe"; }
    }

    private static String listGet() {
        List<String> value = null;
        try { return value.get(0); }
        catch (NullPointerException ex) { return "npe"; }
    }

    private static String mapGet() {
        Map<String, String> value = null;
        try { return value.get("x"); }
        catch (NullPointerException ex) { return "npe"; }
    }

    private static String setContains() {
        Set<String> value = null;
        try { return "" + value.contains("x"); }
        catch (NullPointerException ex) { return "npe"; }
    }

    public static String run() {
        return source() + "," + sourceCall() + "," + string() + ","
            + stringEquals() + "," + stringEqualsNull() + "," + objectEquals() + ","
            + objectEqualsNull() + ","
            + objectStringEquals() + "," + objectStringEqualsNull() + ","
            + objectIntegerEquals() + "," + objectIntegerDifferentType() + ","
            + builder() + "," + list() + ","
            + listGet() + "," + mapGet() + "," + setContains();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
