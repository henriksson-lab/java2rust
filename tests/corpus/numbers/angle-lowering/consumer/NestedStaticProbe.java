package consumer;
import producer.Units;

public class NestedStaticProbe {
    public static int run() {
        Units.Value a = Units.Value.ZERO;
        if (a != Units.Value.ZERO || a.read() != 0) return 1;
        if (Units.Value.ZERO.read() != 0 || Units.Value.of(7).read() != 7) return 2;
        if (Units.Value.OFFSET != Units.Value.OFFSET) return 3;
        if (Units.Value.OFFSET.applyAsDouble(4) != 5 || Units.Value.OFFSET.applyAsDouble(4) != 5) return 4;
        if (Units.Value.SCALE != 360 || Units.Base.SCALE != 360) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
