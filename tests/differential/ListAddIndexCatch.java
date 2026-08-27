import java.util.ArrayList;
import java.util.List;

public class ListAddIndexCatch {
    public static String run() {
        List<String> values = new ArrayList<String>();
        values.add("a");
        values.add("c");
        values.add(1, "b");
        String normal = values.get(0) + values.get(1) + values.get(2) + ":" + values.size();
        try {
            values.add(4, "x");
            normal += ":miss-high";
        } catch (IndexOutOfBoundsException ex) {
            normal += ":high";
        }
        try {
            values.add(-1, "x");
            normal += ":miss-negative";
        } catch (IndexOutOfBoundsException ex) {
            normal += ":negative";
        }
        return normal;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
