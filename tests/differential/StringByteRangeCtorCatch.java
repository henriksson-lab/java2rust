public class StringByteRangeCtorCatch {
    public static String run() {
        byte[] bytes = new byte[] { 65, 66, 67 };
        String ok = new String(bytes, 1, 2);
        String high = "";
        try {
            new String(bytes, 2, 3);
            high = "miss";
        } catch (IndexOutOfBoundsException e) {
            high = "high";
        }
        String negative = "";
        try {
            new String(bytes, -1, 1);
            negative = "miss";
        } catch (IndexOutOfBoundsException e) {
            negative = "negative";
        }
        return ok + ":" + high + ":" + negative;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
