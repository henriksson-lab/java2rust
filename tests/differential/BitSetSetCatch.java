import java.util.BitSet;
import java.lang.IndexOutOfBoundsException;

public class BitSetSetCatch {
    public static String run() {
        BitSet bits = new BitSet();
        bits.set(2);
        try {
            bits.set(-1);
            return "miss";
        } catch (IndexOutOfBoundsException e) {
            return bits.get(2) ? "set:caught" : "lost:caught";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
