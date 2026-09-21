import java.util.Map;

public class NullableMapEntryStringSwitchCatch {
    static class Entry implements Map.Entry<String, String> {
        String key;
        String value;
        Entry(String key, String value) { this.key = key; this.value = value; }
        public String getKey() { return key; }
        public String getValue() { return value; }
        public String setValue(String value) {
            String old = this.value;
            this.value = value;
            return old;
        }
    }

    static String read(Map.Entry<String, String> entry) {
        try {
            switch (entry.getValue()) {
                case "ok": return "matched";
                default: return "other";
            }
        } catch (NullPointerException ex) {
            return "caught";
        }
    }

    public static String run() {
        return read(new Entry("a", null)) + ":" + read(new Entry("b", "ok"))
            + ":" + read(new Entry("c", "else"));
    }

    public static void main(String[] args) { System.out.println(run()); }
}
