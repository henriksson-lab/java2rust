package idiom.gaps;

import java.io.IOException;
import java.net.URI;
import java.net.URISyntaxException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.SortedMap;
import java.util.TreeMap;

final class FileLoadConstructorDelegation {
    private SortedMap<String, URI> paths;

    FileLoadConstructorDelegation() {
        paths = new TreeMap<>();
    }

    FileLoadConstructorDelegation(final Path source) {
        this();
        load(source);
    }

    private void load(final Path source) {
        try {
            final List<String> lines = Files.readAllLines(source);
            for (final String line : lines) {
                final String[] parts = line.split("\\t", -1);
                if (parts.length != 2 || parts[0].trim().isEmpty()) {
                    throw new IllegalArgumentException("bad line");
                }
                try {
                    final URI previous = paths.put(parts[0], new URI(parts[1].trim()));
                    if (previous != null) {
                        throw new IllegalArgumentException("duplicate");
                    }
                } catch (final URISyntaxException e) {
                    throw new IllegalArgumentException("malformed");
                }
            }
        } catch (final IOException e) {
            throw new IllegalArgumentException("read failed");
        }
    }
}
