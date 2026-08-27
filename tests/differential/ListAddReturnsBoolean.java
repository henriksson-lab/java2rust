import java.util.ArrayList;
import java.util.List;

public class ListAddReturnsBoolean {
    public static String run() {
        List<String> values = new ArrayList<>();
        boolean added = values.add("a");
        return added + ":" + values.size();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
