import java.util.BitSet;
import java.lang.IndexOutOfBoundsException;

public class BitSetGetCatch {
    public static String run() {
        BitSet bits = new BitSet();
        bits.set(2);
        boolean missing = bits.get(3);
        try {
            bits.get(-1);
            return "miss";
        } catch (IndexOutOfBoundsException e) {
            return bits.get(2) + "," + missing + ":caught";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
