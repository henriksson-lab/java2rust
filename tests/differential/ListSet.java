import java.util.ArrayList;
import java.util.List;

public class ListSet {
    public static String run() {
        List<String> xs = new ArrayList<String>();
        xs.add("a");
        xs.add("b");
        String old = xs.set(1, "c");
        return old + "," + xs.get(0) + "," + xs.get(1);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
