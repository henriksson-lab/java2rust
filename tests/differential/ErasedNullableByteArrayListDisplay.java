import java.util.ArrayList;

public class ErasedNullableByteArrayListDisplay {
    public static String run() {
        ArrayList<Byte> values = new ArrayList<Byte>();
        values.add((byte) 7);
        values.add(null);
        Object value = values;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
