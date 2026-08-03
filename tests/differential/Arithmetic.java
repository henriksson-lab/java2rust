public class Arithmetic {
    public static String run() {
        int a = 2000000000;
        int overflow = a * 2 + a;
        int shifted = 1 << 33;
        int logical = -1 >>> 8;
        int arith = -1 >> 8;
        int rem = -7 % 3;
        byte narrowed = (byte) 200;
        return overflow + "," + shifted + "," + logical + "," + arith + "," + rem + "," + narrowed;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
