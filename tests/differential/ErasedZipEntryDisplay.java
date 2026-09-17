import java.util.zip.ZipEntry;

public class ErasedZipEntryDisplay {
    public static String run() {
        Object value = new ZipEntry("folder/file.txt");
        return "x" + value + "|" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
