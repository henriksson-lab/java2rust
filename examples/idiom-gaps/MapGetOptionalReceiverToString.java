import java.net.URI;
import java.util.Map;

class MapGetOptionalReceiverToString {
    String render(Map<String, URI> paths, String sample) {
        return paths.get(sample).toString();
    }
}
