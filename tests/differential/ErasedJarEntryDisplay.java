import java.util.jar.JarEntry;

public class ErasedJarEntryDisplay {
    public static String run() {
        JarEntry file = new JarEntry("folder/file.txt");
        JarEntry dir = new JarEntry("folder/");
        String name = file.getName();
        boolean fileDirectory = file.isDirectory();
        boolean dirDirectory = dir.isDirectory();
        Object value = file;
        return name + "|" + fileDirectory + "|" + dirDirectory + "|x" + value + "|" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
