package examples.stdlib;

import java.io.BufferedReader;
import java.io.IOException;

public class TryCatchFinally {
    public String readOrDefault(BufferedReader reader) {
        try {
            String line = reader.readLine();
            return line == null ? "empty" : line;
        } catch (IOException ex) {
            return "error";
        } finally {
            System.out.println("done");
        }
    }
}

