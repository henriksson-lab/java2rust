import java.util.ArrayList;

public class ErasedArrayListDisplay {
    public static String run() {
        ArrayList<String> xs = new ArrayList<>();
        xs.add("a");
        xs.add("b");
        Object value = xs;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
