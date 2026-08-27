import java.util.ArrayList;
import java.util.List;

public class ListAddIndex {
    public static String run() {
        List<String> values = new ArrayList<>();
        values.add("a");
        values.add("c");
        values.add(1, "b");
        return values.get(0) + values.get(1) + values.get(2) + ":" + values.size();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
