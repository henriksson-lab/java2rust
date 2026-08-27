import java.util.ArrayList;
import java.util.List;

public class ListDisplay {
    public static String run() {
        List<String> xs = new ArrayList<>();
        xs.add("a");
        xs.add("b");
        return "xs=" + xs;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
