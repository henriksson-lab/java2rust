import java.net.URI;
import java.util.HashMap;
import java.util.Map;

class FieldMapPutKeyReuse {
    private final Map<String, URI> paths = new HashMap<>();

    void add(String sample, String path) {
        URI previous = paths.put(sample, new URI(path));
        if (previous != null) {
            throw new IllegalArgumentException("duplicate " + sample);
        }
    }
}
