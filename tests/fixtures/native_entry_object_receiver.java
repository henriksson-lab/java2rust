import java.util.*;

class EntryCounter {
    private int value;
    EntryCounter(int initial) { value = initial; }
    int read() { return value; }
    void bump() { value++; }
}

class NativeEntryObjectReceiver {
    static int run() {
        EntryCounter original = new EntryCounter(4);
        Map<String, EntryCounter> counters = new LinkedHashMap<>();
        counters.put("one", original);
        int result = 0;
        for (Map.Entry<String, EntryCounter> entry : counters.entrySet()) {
            entry.getValue().bump();
            result += entry.getValue().read();
        }
        return result + original.read();
    }

    static int nullReceiver() {
        Map<String, EntryCounter> counters = new LinkedHashMap<>();
        counters.put("null", null);
        try {
            for (Map.Entry<String, EntryCounter> entry : counters.entrySet()) {
                return entry.getValue().read();
            }
        } catch (NullPointerException ex) { return 7; }
        return -1;
    }

    static int nullMutation() {
        Map<String, EntryCounter> counters = new LinkedHashMap<>();
        counters.put("null", null);
        try {
            for (Map.Entry<String, EntryCounter> entry : counters.entrySet()) {
                entry.getValue().bump();
            }
        } catch (NullPointerException ex) { return 9; }
        return -1;
    }

    static int keyReceiver() {
        EntryCounter original = new EntryCounter(6);
        Map<EntryCounter, String> counters = new LinkedHashMap<>();
        counters.put(original, "one");
        int result = 0;
        for (Map.Entry<EntryCounter, String> entry : counters.entrySet()) {
            entry.getKey().bump();
            result += entry.getKey().read();
        }
        return result + original.read();
    }

    static int nullKey() {
        Map<EntryCounter, String> counters = new LinkedHashMap<>();
        counters.put(null, "null");
        try {
            for (Map.Entry<EntryCounter, String> entry : counters.entrySet()) {
                return entry.getKey().read();
            }
        } catch (NullPointerException ex) { return 11; }
        return -1;
    }
}
