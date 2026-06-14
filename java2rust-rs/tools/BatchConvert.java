import de.aschoerk.java2rust.JavaConverter;

import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.stream.Stream;

/**
 * Walk a directory of .java files; for each, write convert2Rust(content) to
 * <outDir>/<relpath>.rs (mirroring the tree). Used for htsjdk parity testing.
 *
 * Usage: java -cp java2rust.jar:tools BatchConvert <inDir> <outDir>
 */
public class BatchConvert {
    public static void main(String[] args) throws Exception {
        Path inDir = Paths.get(args[0]);
        Path outDir = Paths.get(args[1]);

        PrintStream realOut = System.out;
        PrintStream nullOut = new PrintStream(new OutputStream() {
            public void write(int b) {}
        });

        int[] count = {0};
        try (Stream<Path> paths = Files.walk(inDir)) {
            paths.filter(p -> p.toString().endsWith(".java")).sorted().forEach(p -> {
                try {
                    String text = new String(Files.readAllBytes(p), StandardCharsets.UTF_8);
                    String result;
                    System.setOut(nullOut);
                    try {
                        result = JavaConverter.convert2Rust(text);
                    } catch (Throwable t) {
                        result = "<<JAR_THROW: " + t + ">>";
                    } finally {
                        System.setOut(realOut);
                    }
                    Path rel = inDir.relativize(p);
                    Path out = outDir.resolve(rel.toString() + ".rs");
                    Files.createDirectories(out.getParent());
                    Files.write(out, result.getBytes(StandardCharsets.UTF_8));
                    count[0]++;
                } catch (Exception e) {
                    realOut.println("IO error on " + p + ": " + e);
                }
            });
        }
        realOut.println("converted " + count[0] + " files");
    }
}
