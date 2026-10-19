import java.util.*;
import java.util.stream.Collectors;

class CheckedStreamJoining {
    private static int order;
    static String mark(int digit, String value) { order = order * 10 + digit; return value; }
    static String label(String value, boolean fail) {
        order = order * 10 + 4;
        if (fail && value.equals("b")) throw new IllegalArgumentException();
        return value;
    }
    static String badDelimiter() { order = order * 10 + 5; throw new IllegalArgumentException(); }
    static String extendingDelimiter(List<String> values) { order = order * 10 + 1; values.add("b"); return ","; }
    static String argumentFailure() {
        order = 0;
        try {
            return Arrays.asList("a").stream().map(value -> label(value, false))
                .collect(Collectors.joining(badDelimiter(), "[", "]"));
        } catch (IllegalArgumentException ex) { return "argument:" + order; }
    }
    static String argumentMutation() {
        order = 0;
        List<String> values = new ArrayList<>(); values.add("a");
        Iterable<String> retained = values;
        return values.stream().map(value -> label(value, false))
            .collect(Collectors.joining(extendingDelimiter(values), "[", "]")) + ":" + order;
    }
    static String run(boolean fail) {
        order = 0;
        List<String> values = new ArrayList<>();
        values.add("a"); values.add("b");
        Iterable<String> retained = values;
        try {
            String joined = values.stream().map(value -> label(value, fail))
                .collect(Collectors.joining(mark(1, ","), mark(2, "["), mark(3, "]")));
            return joined + ":" + order;
        } catch (IllegalArgumentException ex) { return "caught:" + order; }
    }
    static String empty() {
        order = 0;
        List<String> values = new ArrayList<>();
        Iterable<String> retained = values;
        return values.stream().map(value -> label(value, false))
            .collect(Collectors.joining(mark(1, ","), mark(2, "["), mark(3, "]"))) + ":" + order;
    }
    static String simple() {
        order = 0;
        return Arrays.asList("a", "b").stream().map(value -> label(value, false))
            .collect(Collectors.joining(",")) + ":" + order;
    }
    static String nested() {
        order = 0;
        List<String> values = Arrays.asList("a", "b");
        Iterable<String> retained = values;
        Map<String, List<String>> groups = new LinkedHashMap<>();
        groups.put("g", values);
        StringBuilder text = new StringBuilder();
        for (Map.Entry<String, List<String>> entry : groups.entrySet()) {
            text.append(entry.getKey()).append(entry.getValue().stream()
                .map(value -> label(entry.getKey() + value, false))
                .collect(Collectors.joining(mark(1, ","), mark(2, "["), mark(3, "]"))));
        }
        return text.toString() + ":" + order;
    }
    static String fixedListPolicy() {
        List<String> values = Arrays.asList("x", "y");
        Iterable<String> retained = values;
        values.set(0, "z");
        try { values.add("bad"); return "bad"; }
        catch (UnsupportedOperationException ex) { return values.get(0) + ":" + values.size(); }
    }
    static String plainNested() {
        order = 0;
        Map<String, List<String>> groups = new LinkedHashMap<>();
        groups.put("g", Arrays.asList("a", "b"));
        StringBuilder text = new StringBuilder();
        for (Map.Entry<String, List<String>> entry : groups.entrySet()) {
            text.append(entry.getKey()).append(entry.getValue().stream()
                .map(value -> label(entry.getKey() + value, false))
                .collect(Collectors.joining(mark(1, ","), mark(2, "["), mark(3, "]"))));
        }
        return text.toString() + ":" + order;
    }
    static String nullNested() {
        order = 0;
        List<String> values = new ArrayList<>();
        Iterable<String> retained = values;
        Map<String, List<String>> groups = new LinkedHashMap<>();
        groups.put("missing", null);
        groups.put("present", values);
        try {
            for (Map.Entry<String, List<String>> entry : groups.entrySet()) {
                entry.getValue().stream().map(value -> label(value, false))
                    .collect(Collectors.joining(mark(1, ","), "[", "]"));
            }
            return "bad";
        } catch (NullPointerException ex) { return "null:" + order; }
    }
    static String aliasNestedList() {
        Map<String, List<String>> groups = new LinkedHashMap<>();
        groups.put("g", Arrays.asList("a", "b"));
        for (Map.Entry<String, List<String>> entry : groups.entrySet()) {
            List<String> alias = entry.getValue();
            Iterable<String> retained = alias;
            alias.set(0, "z");
            return entry.getValue().get(0);
        }
        return "bad";
    }
}
