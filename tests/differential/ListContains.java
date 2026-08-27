import java.util.ArrayList;
import java.util.List;

public class ListContains {
    public static String run() {
        List<String> xs = new ArrayList<String>();
        xs.add("a");
        xs.add("b");
        return xs.contains("b") + "," + xs.contains("z");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
