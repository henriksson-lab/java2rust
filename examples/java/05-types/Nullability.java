package examples.types;

public class Nullability {
    private String cached;

    public String getOrDefault(String fallback) {
        if (cached == null) {
            return fallback;
        }
        return cached;
    }

    public void set(String value) {
        cached = value;
    }
}

