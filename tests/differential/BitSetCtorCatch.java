import java.util.BitSet;

public class BitSetCtorCatch {
    public static String run() {
        BitSet bits = new BitSet(3);
        bits.set(2);
        String negative = "";
        try {
            new BitSet(-1);
            negative = "miss";
        } catch (NegativeArraySizeException e) {
            negative = "negative";
        }
        return (bits.get(2) ? "set" : "lost") + ":" + negative;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
