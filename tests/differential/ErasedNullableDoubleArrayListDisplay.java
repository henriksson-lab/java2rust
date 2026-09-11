import java.util.ArrayList;

public class ErasedNullableDoubleArrayListDisplay {
    public static String run() {
        ArrayList<Double> values = new ArrayList<Double>();
        values.add(7.5);
        values.add(null);
        Object value = values;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
