import java.util.ArrayList;
import java.util.Optional;

public class ErasedOptionalCollectionDisplay {
    public static String run() {
        ArrayList<String> values = new ArrayList<String>();
        values.add("a");
        values.add(null);
        values.add("c");
        Object present = Optional.of(values);
        Object empty = Optional.empty();
        return "p=" + present + ",e=" + empty + ",direct=" + present.toString();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
