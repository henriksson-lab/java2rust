package examples.types;

public class NullableClearAssignment {
    private String cached = "ready";

    public String clearAndReturn(String fallback) {
        String previous = cached;
        cached = null;
        if (previous == null) {
            return fallback;
        }
        return previous;
    }
}
