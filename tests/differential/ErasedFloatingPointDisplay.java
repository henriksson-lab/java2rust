import java.util.ArrayList;
import java.util.Optional;

public class ErasedFloatingPointDisplay {
    public static String run() {
        Object direct = 1.0;
        Object optional = Optional.of(1.0);
        ArrayList<Double> values = new ArrayList<Double>();
        values.add(1.0);
        values.add(2.5);
        Object list = values;
        return "d=" + direct + ",o=" + optional + ",l=" + list;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
