import java.util.HashMap;
import java.util.Map;

public class OptionalArgumentWrapReuse {
    private final Map<String, String> seen = new HashMap<>();

    boolean valid(String name) {
        return name != null && !name.trim().isEmpty();
    }

    public void check(String name) {
        seen.put(name, "seen");
        if (!valid(name)) {
            throw new IllegalArgumentException("bad " + name);
        }
    }
}
