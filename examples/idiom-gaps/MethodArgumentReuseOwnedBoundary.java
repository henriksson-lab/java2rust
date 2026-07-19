package idiom.gaps;

public class MethodArgumentReuseOwnedBoundary {
    public String keep(String value) {
        return value;
    }

    public String useTwice(String name) {
        String kept = keep(name);
        if (name.isEmpty()) {
            return "empty";
        }
        return kept;
    }
}
