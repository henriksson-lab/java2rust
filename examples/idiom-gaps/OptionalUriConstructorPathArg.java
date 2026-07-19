import java.net.URI;

public class OptionalUriConstructorPathArg {
    public URI parse(String path) {
        if (path == null) {
            throw new IllegalArgumentException("missing");
        }
        URI uri = new URI(path);
        return uri;
    }
}
