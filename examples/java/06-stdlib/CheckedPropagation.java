package examples.stdlib;

import java.io.BufferedReader;
import java.io.IOException;

public class CheckedPropagation {
    public int lineLength(BufferedReader reader) throws IOException {
        return reader.readLine().length();
    }
}

