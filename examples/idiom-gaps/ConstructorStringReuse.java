import java.net.URI;

class ConstructorStringReuse {
    URI choose(String path) {
        URI uri = new URI(path);
        if (path.isEmpty()) {
            return new URI("fallback");
        }
        return uri;
    }
}
