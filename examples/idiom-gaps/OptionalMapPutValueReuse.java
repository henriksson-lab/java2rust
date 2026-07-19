import java.net.URI;
import java.util.HashMap;
import java.util.Map;

class OptionalMapPutValueReuse {
    private final Map<String, URI> paths = new HashMap<>();

    void add(String sample, URI path) {
        if (path == null) {
            throw new IllegalArgumentException("missing " + sample);
        }
        URI previous = paths.put(sample, path);
        if (previous != null) {
            throw new IllegalArgumentException("duplicate " + path);
        }
    }
}
