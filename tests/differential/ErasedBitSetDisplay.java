import java.util.BitSet;

public class ErasedBitSetDisplay {
    public static String run() {
        BitSet bits = new BitSet();
        bits.set(2);
        bits.set(5);
        Object value = bits;
        return "x" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
