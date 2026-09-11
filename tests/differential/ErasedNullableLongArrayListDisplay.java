import java.util.ArrayList;

public class ErasedNullableLongArrayListDisplay {
    public static String run() {
        ArrayList<Long> values = new ArrayList<Long>();
        values.add(7L);
        values.add(null);
        Object value = values;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
