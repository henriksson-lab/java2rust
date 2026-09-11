import java.util.BitSet;

public class BitSetClearFlipCatch {
    public static String run() {
        BitSet bits = new BitSet();
        bits.set(2);
        bits.clear(2);
        bits.flip(3);

        String clearStatus;
        try {
            bits.clear(-1);
            clearStatus = "miss";
        } catch (IndexOutOfBoundsException e) {
            clearStatus = "clear";
        }

        String flipStatus;
        try {
            bits.flip(-1);
            flipStatus = "miss";
        } catch (IndexOutOfBoundsException e) {
            flipStatus = "flip";
        }

        return (!bits.get(2)) + "," + bits.get(3) + ":" + clearStatus + "," + flipStatus;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
