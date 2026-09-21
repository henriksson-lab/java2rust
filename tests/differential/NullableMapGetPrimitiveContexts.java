import java.util.HashMap;

public class NullableMapGetPrimitiveContexts {
    static int identity(int value) { return value; }

    static String condition(boolean present) {
        HashMap<String, Boolean> values = new HashMap<>();
        if (present) values.put("k", true);
        try {
            return values.get("k") ? "true" : "false";
        } catch (NullPointerException expected) {
            return "caught-condition";
        }
    }

    static int returned(HashMap<String, Integer> values) {
        return values.get("k");
    }

    static String primitiveReturn(boolean present) {
        HashMap<String, Integer> values = new HashMap<>();
        if (present) values.put("k", 3);
        try {
            return "return-" + returned(values);
        } catch (NullPointerException expected) {
            return "caught-return";
        }
    }

    static String argument(boolean present) {
        HashMap<String, Integer> values = new HashMap<>();
        if (present) values.put("k", 4);
        try {
            return "argument-" + identity(values.get("k"));
        } catch (NullPointerException expected) {
            return "caught-argument";
        }
    }

    static String assignment(boolean present) {
        HashMap<String, Integer> values = new HashMap<>();
        if (present) values.put("k", 5);
        try {
            int value = values.get("k");
            return "assignment-" + value;
        } catch (NullPointerException expected) {
            return "caught-assignment";
        }
    }

    static String arrayIndex(boolean present) {
        HashMap<String, Integer> values = new HashMap<>();
        if (present) values.put("k", 1);
        try {
            return "index-" + new int[] { 6, 7 }[values.get("k")];
        } catch (NullPointerException expected) {
            return "caught-index";
        }
    }

    static String stringReceiver(boolean present) {
        HashMap<String, String> values = new HashMap<>();
        if (present) values.put("k", "abc");
        try {
            return "length-" + values.get("k").length();
        } catch (NullPointerException expected) {
            return "caught-receiver";
        }
    }

    public static String run() {
        return condition(false) + ":" + condition(true) + ","
            + primitiveReturn(false) + ":" + primitiveReturn(true) + ","
            + argument(false) + ":" + argument(true) + ","
            + assignment(false) + ":" + assignment(true) + ","
            + arrayIndex(false) + ":" + arrayIndex(true) + ","
            + stringReceiver(false) + ":" + stringReceiver(true);
    }

    public static void main(String[] args) { System.out.println(run()); }
}
