import java.util.ArrayList;

public class ErasedNullableFloatArrayListDisplay {
    public static String run() {
        ArrayList<Float> values = new ArrayList<Float>();
        values.add(7.5f);
        values.add(null);
        Object value = values;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
