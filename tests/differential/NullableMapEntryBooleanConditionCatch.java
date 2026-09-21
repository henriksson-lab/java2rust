import java.util.Map;

public class NullableMapEntryBooleanConditionCatch {
    static class Entry implements Map.Entry<String, Boolean> {
        String key;
        Boolean value;
        Entry(String key, Boolean value) { this.key = key; this.value = value; }
        public String getKey() { return key; }
        public Boolean getValue() { return value; }
        public Boolean setValue(Boolean value) {
            Boolean old = this.value;
            this.value = value;
            return old;
        }
    }

    static String read(Map.Entry<String, Boolean> entry) {
        try {
            if (entry.getValue()) return "yes";
            return "no";
        } catch (NullPointerException ex) {
            return "caught";
        }
    }

    static String readNegated(Map.Entry<String, Boolean> entry) {
        try {
            return !entry.getValue() ? "yes" : "no";
        } catch (NullPointerException ex) {
            return "caught";
        }
    }

    static String readAnd(boolean left) {
        Map.Entry<String, Boolean> entry = new Entry("g", null);
        try {
            return left && entry.getValue() ? "yes" : "no";
        } catch (NullPointerException ex) {
            return "caught";
        }
    }

    static String readOr(boolean left) {
        Map.Entry<String, Boolean> entry = new Entry("h", null);
        try {
            return left || entry.getValue() ? "yes" : "no";
        } catch (NullPointerException ex) {
            return "caught";
        }
    }

    public static String run() {
        return read(new Entry("a", null)) + ":" + read(new Entry("b", true))
            + ":" + read(new Entry("c", false)) + ":"
            + readNegated(new Entry("d", null)) + ":"
            + readNegated(new Entry("e", true)) + ":"
            + readNegated(new Entry("f", false)) + ":"
            + readAnd(false) + ":" + readAnd(true) + ":"
            + readOr(true) + ":" + readOr(false);
    }

    public static void main(String[] args) { System.out.println(run()); }
}
