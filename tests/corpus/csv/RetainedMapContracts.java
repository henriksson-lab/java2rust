import java.util.Map;
import java.util.LinkedHashMap;

public class RetainedMapContracts {
    private Map<String, Integer> values;
    private int total;
    private Map<String, String> text = new LinkedHashMap<>();
    private static Map<String, Integer> shared = new LinkedHashMap<>();
    private static Map<String, Integer> sharedCopy = new LinkedHashMap<>();
    private static int sharedTotal;
    private static int order;
    private Map<String, Integer> replacement = new LinkedHashMap<>();
    RetainedMapContracts(Map<String, Integer> values) { this.values = values; }
    Map<String, Integer> values() { return values; }
    Map<String, Integer> copy() {
        Map<String, Integer> target = new LinkedHashMap<>();
        target.putAll(values);
        return target;
    }
    void visit() {
        values.forEach((key, value) -> {
            if (key.equals("a")) values.put("b", 9);
            total += value;
        });
    }
    void selfCopy() { values.putAll(values); }
    void addDuringVisit() { values.forEach((key, value) -> values.put("new", 7)); }
    Map<String, Integer> target(boolean missing) { order = order * 10 + 1; return missing ? null : values; }
    Map<String, Integer> origin(boolean missing) { order = order * 10 + 2; return missing ? null : values; }
    void checkedTransfer(boolean targetMissing, boolean sourceMissing) { target(targetMissing).putAll(origin(sourceMissing)); }
    Map<String, Integer> rebind() { values = replacement; return replacement; }
    void rebindCopy() { values.putAll(rebind()); }
    Map<String, String> copyText() {
        Map<String, String> target = new LinkedHashMap<>();
        target.putAll(text);
        return target;
    }
    static void visitShared() {
        shared.forEach((key, value) -> {
            if (key.equals("a")) shared.put("b", 9);
            sharedTotal += value;
        });
        sharedCopy.putAll(shared);
    }
    public static int run() {
        Map<String, Integer> source = new LinkedHashMap<>();
        source.put("a", 1);
        source.put("b", 2);
        RetainedMapContracts owner = new RetainedMapContracts(source);
        owner.visit();
        if (owner.total != 10 || source.get("b") != 9) return 1;
        Map<String, Integer> copy = owner.copy();
        if (copy == source || copy.size() != 2 || copy.get("b") != 9) return 2;
        source.put("b", 11);
        if (copy.get("b") != 9) return 3;
        owner.selfCopy();
        if (owner.values() != source || source.size() != 2) return 4;
        try { owner.addDuringVisit(); return 5; }
        catch (java.util.ConcurrentModificationException expected) { }
        owner.text.put("present", null);
        owner.text.put(null, "null-key");
        Map<String, String> textCopy = owner.copyText();
        if (textCopy == owner.text || textCopy.size() != 2) return 6;
        if (!textCopy.containsKey("present") || textCopy.get("present") != null) return 7;
        if (!"null-key".equals(textCopy.get(null))) return 8;
        owner.text.put("present", "later");
        if (textCopy.get("present") != null) return 9;
        shared.put("a", 1);
        shared.put("b", 2);
        visitShared();
        if (sharedTotal != 10 || sharedCopy.get("b") != 9 || sharedCopy == shared) return 10;
        order = 0;
        try { owner.checkedTransfer(true, true); return 11; }
        catch (NullPointerException expected) { if (order != 12) return 12; }
        order = 0;
        try { owner.checkedTransfer(false, true); return 13; }
        catch (NullPointerException expected) { if (order != 12) return 14; }
        owner.replacement.put("captured", 77);
        owner.rebindCopy();
        if (owner.values() == source || source.get("captured") != 77) return 15;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
