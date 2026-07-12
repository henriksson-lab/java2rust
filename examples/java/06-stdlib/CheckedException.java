package examples.stdlib;

import java.io.BufferedReader;
import java.io.IOException;

public class CheckedException {
    public String firstLine(BufferedReader reader) throws IOException {
        String line = reader.readLine();
        if (line == null) {
            throw new IOException("empty");
        }
        return line;
    }
}

