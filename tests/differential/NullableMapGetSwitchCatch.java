import java.util.HashMap;

public class NullableMapGetSwitchCatch {
    static String readNumber(boolean present) {
        HashMap<String, Integer> values = new HashMap<>();
        if (present) values.put("k", 2);
        try {
            switch (values.get("k")) {
                case 2: return "two";
                default: return "other";
            }
        } catch (NullPointerException expected) {
            return "caught-number";
        }
    }

    static String readString(boolean present) {
        HashMap<String, String> values = new HashMap<>();
        if (present) values.put("k", "ok");
        try {
            switch (values.get("k")) {
                case "ok": return "ok";
                default: return "other";
            }
        } catch (NullPointerException expected) {
            return "caught-string";
        }
    }

    public static String run() {
        return readNumber(false) + ":" + readNumber(true) + ":"
            + readString(false) + ":" + readString(true);
    }

    public static void main(String[] args) { System.out.println(run()); }
}
