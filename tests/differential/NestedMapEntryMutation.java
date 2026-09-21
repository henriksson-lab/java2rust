import java.util.HashMap;
import java.util.Map;

public class NestedMapEntryMutation {
    public static String run() {
        HashMap<String, Integer> values = new HashMap<>();
        values.put("a", 1);
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();

        int iterations = 0;
        while (iterations++ == 0) {
            entry.setValue(2);
        }
        switch (values.get("a")) {
            case 2:
                entry.setValue(3);
                break;
            default:
                entry.setValue(-1);
        }
        try {
            entry.setValue(4);
        } finally {
            entry.setValue(5);
        }
        return values.get("a") + ":" + entry.getValue();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
