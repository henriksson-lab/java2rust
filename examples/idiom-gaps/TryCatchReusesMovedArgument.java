import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

class TryCatchReusesMovedArgument {
    int count(Path path) {
        try {
            List<String> lines = Files.readAllLines(path);
            return lines.size();
        } catch (IOException e) {
            throw new RuntimeException(path.toString(), e);
        }
    }
}
