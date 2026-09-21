import java.util.Map;

public class NullableMapEntryPrimitiveReturnCatch {
    static class Entry implements Map.Entry<String, Integer> {
        String key;
        Integer value;
        Entry(String key, Integer value) { this.key = key; this.value = value; }
        public String getKey() { return key; }
        public Integer getValue() { return value; }
        public Integer setValue(Integer value) {
            Integer old = this.value;
            this.value = value;
            return old;
        }
    }

    static int read(Map.Entry<String, Integer> entry) {
        return entry.getValue();
    }

    static int take(int value) { return value; }

    static String checkReturn() {
        Map.Entry<String, Integer> entry = new Entry("a", null);
        try {
            return "wrong:" + read(entry);
        } catch (NullPointerException ex) {
            return "caught";
        }
    }

    static String checkArgument() {
        Map.Entry<String, Integer> entry = new Entry("b", null);
        try {
            return "wrong:" + take(entry.getValue());
        } catch (NullPointerException ex) {
            return "caught";
        }
    }

    static String checkAssignment() {
        Map.Entry<String, Integer> entry = new Entry("c", null);
        int value = 0;
        try {
            value = entry.getValue();
            return "wrong:" + value;
        } catch (NullPointerException ex) {
            return "caught";
        }
    }

    static String checkArithmetic() {
        Map.Entry<String, Integer> entry = new Entry("d", null);
        try {
            return "wrong:" + (entry.getValue() + 1);
        } catch (NullPointerException ex) {
            return "caught";
        }
    }

    static String checkComparison() {
        Map.Entry<String, Integer> entry = new Entry("f", null);
        try {
            return entry.getValue() > 0 ? "wrong" : "wrong";
        } catch (NullPointerException ex) {
            return "caught";
        }
    }

    static String checkSwitch() {
        Map.Entry<String, Integer> entry = new Entry("g", null);
        try {
            switch (entry.getValue()) {
                case 1: return "wrong-one";
                default: return "wrong-other";
            }
        } catch (NullPointerException ex) {
            return "caught";
        }
    }

    static String checkSwitchPresent() {
        Map.Entry<String, Integer> entry = new Entry("h", 4);
        switch (entry.getValue()) {
            case 4: return "four";
            default: return "other";
        }
    }

    static String checkPresent() {
        Map.Entry<String, Integer> entry = new Entry("e", 4);
        int value = 0;
        value = entry.getValue();
        return read(entry) + ":" + take(entry.getValue()) + ":" + value
            + ":" + (entry.getValue() + 1) + ":" + (entry.getValue() > 3);
    }

    public static String run() {
        return checkReturn() + ":" + checkArgument() + ":" + checkAssignment()
            + ":" + checkArithmetic() + ":" + checkComparison() + ":"
            + checkSwitch() + ":" + checkSwitchPresent() + ":" + checkPresent();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
