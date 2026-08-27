import java.util.ArrayList;
import java.util.List;

public class ListRemoveObject {
    public static String run() {
        List<String> xs = new ArrayList<String>();
        xs.add("a");
        xs.add("b");
        xs.add("b");
        boolean removed = xs.remove("b");
        boolean missing = xs.remove("z");
        return removed + "," + missing + "," + xs.size() + "," + xs.get(0) + "," + xs.get(1);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
