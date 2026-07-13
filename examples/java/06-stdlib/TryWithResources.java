package examples.stdlib;

import java.io.BufferedReader;
import java.io.IOException;

public class TryWithResources {
    public String readFirst(Path path) {
        try (BufferedReader reader = Files.newBufferedReader(path);
             Closeable audit = Audit.open(path)) {
            return reader.readLine();
        } catch (IOException ex) {
            return "error";
        }
    }
}

