import java.util.List;
import java.util.Set;
import java.util.Map;
import java.util.ArrayList;
import java.util.LinkedHashMap;

class GenericCollectionDisplay {
    static <T> String wrapped(List<T> values) { return "(" + values + ")"; }
    static <U> String forwarded(List<U> values) { return wrapped(values); }
    static <T> String nested(List<Set<T>> values) { return "nested=" + values; }
    static <T> String map(Map<String, T> values) { return "map=" + values; }
    static <T> String explicit(List<T> values) { return values.toString(); }
    static <T> String formatted(List<T> values) { return String.format("<%s>", values); }
    static <T> String unused(List<T> values) { return "unused"; }
    static String probe() {
        List<String> values = new ArrayList<>();
        values.add("alpha"); values.add("beta");
        Map<String, String> map = new LinkedHashMap<>(); map.put("k", "v");
        GenericDisplayHolder<String> holder = new GenericDisplayHolder<>();
        return forwarded(values) + ";" + map(map) + ";" + explicit(values)
            + ";" + formatted(values) + ";" + holder.render(values);
    }
    static String nullProbe() { return GenericCollectionDisplay.<String>wrapped(null); }
}

class GenericDisplayHolder<T> {
    String render(List<T> values) { return GenericCollectionDisplay.forwarded(values); }
    int unrelated(List<T> values) { return 7; }
}
